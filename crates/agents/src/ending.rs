use super::*;

pub(super) struct State {
    /// The same Terminals, behind the seam `stop` and `remove` kill through: real in production,
    /// a fake in the one test that proves a failed kill deletes nothing (A16), since `Terminals`
    /// is a concrete type a test cannot otherwise make fail on demand.
    pub(super) kill: Arc<dyn KillsTerminals>,
}

impl Agents {
    /// Daemon shutdown: end every running Agent's Claude Code through its own exit, concurrently,
    /// and return once each has exited or `CLEAN_EXIT_BOUND` has passed. What is left running is
    /// the caller's to kill.
    pub async fn exit_running(&self) -> Result<(), RpcError> {
        let nodes = self.shared.rail().tree()?;
        let mut exits = tokio::task::JoinSet::new();
        for terminal_id in nodes.into_iter().filter_map(|node| node.terminal_id) {
            let terminals = Arc::clone(&self.shared.terminals);
            exits.spawn(async move {
                if let Ok(mut exit) = terminals.subscribe(&terminal_id) {
                    exit_through_claude_code(&terminals, &terminal_id, &mut exit).await;
                }
            });
        }
        while let Some(joined) = exits.join_next().await {
            joined.map_err(RpcError::internal)?;
        }
        Ok(())
    }
}

impl Shared {
    pub(super) fn release_closing(&self, id: &str, ended: bool) {
        let mut runs = self.runs();
        if let Some(Slot::Running(run)) = runs.get_mut(id) {
            run.closing = false;
            run.ended |= ended;
        } else if let Some(Slot::ResumeCleanup {
            old: Some(mut old), ..
        }) = runs.remove(id)
        {
            old.closing = false;
            old.ended |= ended;
            runs.insert(id.to_owned(), Slot::Running(*old));
        }
    }

    /// A21: stop a Terminal in the background, for a failure that must not block the caller
    /// (`observe`'s own `Err` return) and has no actor to attribute a `rail.changed` to.
    pub(super) fn spawn_kill(&self, terminal_id: String) {
        let kill = Arc::clone(&self.ending.kill);
        let terminals = Arc::clone(&self.terminals);
        tokio::spawn(async move {
            if let Err(err) = kill_or_already_gone(&*kill, &terminals, &terminal_id).await {
                eprintln!(
                    "agents: could not stop terminal {terminal_id} after a conversation save \
                     failure: {err}"
                );
            }
        });
    }

    pub(super) async fn stop(&self, actor: Actor, id: &str) -> Result<(), RpcError> {
        self.end_with(actor, id, Ending::Stop, RailPublication::Immediate)
            .await
    }

    /// G5: stop the Agent and delete its node, Worktree directory and branch whatever their
    /// state.
    pub(super) async fn discard(&self, actor: Actor, id: &str) -> Result<(), RpcError> {
        if self.rail().node(id)?.worktree.is_none() {
            return Err(RpcError::new(code::NOT_FOUND, format!("no_worktree: {id}")));
        }
        self.end_with(actor, id, Ending::Discard, RailPublication::Immediate)
            .await
    }

    /// A32: stop the Agent and delete its node, leaving its Worktree directory and branch as they
    /// are, whatever their state.
    pub(super) async fn retire(&self, actor: Actor, id: &str) -> Result<(), RpcError> {
        self.end_with(actor, id, Ending::Retire, RailPublication::Immediate)
            .await
    }

    pub(super) async fn end_with(
        &self,
        actor: Actor,
        id: &str,
        ending: Ending,
        publication: RailPublication,
    ) -> Result<(), RpcError> {
        let remove = !matches!(ending, Ending::Stop | Ending::StopForRemoval);
        let checked_node = if ending == Ending::Remove {
            Some(self.rail().node(id)?)
        } else {
            None
        };
        if let Some(worktree) = checked_node
            .as_ref()
            .and_then(|node| node.worktree.as_ref())
        {
            let git = self.git.bounded();
            let project = self.project_dir.clone();
            let worktree = worktree::Worktree::from(worktree);
            tokio::task::spawn_blocking(move || git.require_landed(&project, &worktree))
                .await
                .map_err(RpcError::internal)??;
        }
        let (terminal_id, worktree, measured_attempt) = {
            let rail = self.rail();
            let mut node = rail.node(id)?;
            if checked_node.as_ref().is_some_and(|checked| {
                checked.attempt != node.attempt || checked.worktree != node.worktree
            }) {
                return Err(RpcError::conflict(format!(
                    "{id} changed during Worktree precheck"
                )));
            }
            let mut runs = self.runs();
            if !remove
                && node.terminal_id.is_none()
                && runs
                    .get(id)
                    .is_none_or(|s| matches!(s, Slot::Running(r) if r.ended))
            {
                return Ok(());
            }
            match runs.get_mut(id) {
                Some(Slot::Starting { .. }) => {
                    return Err(RpcError::conflict(format!("{id} is still starting")));
                }
                // A22: whichever of a resume, a stop or a remove claims the id first wins.
                Some(Slot::Resuming { .. }) => {
                    return Err(RpcError::conflict(format!("{id} is resuming")));
                }
                Some(Slot::Running(run)) => {
                    if run.closing {
                        return Err(RpcError::conflict(format!("{id} is stopping")));
                    }
                    run.closing = true;
                    node.terminal_id = Some(run.terminal_id.clone());
                }
                None => {
                    runs.insert(id.to_owned(), Slot::Closing);
                }
                Some(Slot::ResumeCleanup {
                    terminal_id,
                    pending,
                    error,
                    ..
                }) => {
                    if *pending {
                        return Err(error.clone());
                    }
                    node.terminal_id = Some(terminal_id.clone());
                    *pending = true;
                }
                Some(Slot::Closing) => {
                    return Err(RpcError::conflict(format!("{id} is being removed")));
                }
            }
            (
                node.terminal_id,
                matches!(ending, Ending::Remove | Ending::Discard)
                    .then_some(node.worktree)
                    .flatten(),
                node.attempt,
            )
        };
        if let Some(Slot::Running(run)) = self.runs().get_mut(id) {
            self.apply(None, run, id, &actor, Observation::Stopped, None)?;
        }
        let killed = match terminal_id {
            Some(terminal_id) => {
                let killed =
                    kill_or_already_gone(&*self.ending.kill, &self.terminals, &terminal_id).await;
                if matches!(self.runs().get(id), Some(Slot::ResumeCleanup { .. })) {
                    killed.and_then(|()| self.confirm_terminal_exit(&terminal_id))
                } else {
                    killed
                }
            }
            None => Ok(()),
        };
        if killed.is_ok()
            && let Some(attempt) = measured_attempt
        {
            self.timing
                .lock()
                .expect("measurement timing lock")
                .end_attempt(id, &attempt);
        }
        if killed.is_ok()
            && let Some(worktree) = worktree
        {
            let git = self.git.bounded();
            let project = self.project_dir.clone();
            let worktree = worktree::Worktree::from(&worktree);
            let removed = tokio::task::spawn_blocking(move || {
                if ending == Ending::Discard {
                    git.discard(&project, &worktree)
                } else {
                    git.remove_landed(&project, &worktree)
                }
            })
            .await
            .map_err(RpcError::internal)?;
            if let Err(err) = removed {
                self.rail().detach_terminal(id)?;
                self.release_closing(id, true);
                if publication == RailPublication::Immediate {
                    self.bus.emit(actor, EventData::RailChanged);
                }
                return Err(err);
            }
        }
        let result = {
            let mut rail = self.rail();
            let mut runs = self.runs();
            if let Err(err) = killed {
                if let Some(Slot::Running(run)) = runs.get_mut(id) {
                    run.closing = false;
                } else if let Some(Slot::ResumeCleanup { pending, .. }) = runs.get_mut(id) {
                    *pending = false;
                } else {
                    runs.remove(id);
                }
                return Err(err);
            }
            let result = if remove {
                rail.remove(id)
            } else {
                rail.detach_terminal(id)
            };
            if remove {
                runs.remove(id);
                if result.is_ok() {
                    self.events
                        .tool_calls
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .remove(id);
                }
            } else if let Some(Slot::Running(run)) = runs.get_mut(id) {
                run.closing = false;
                run.ended = true;
            } else if let Some(Slot::ResumeCleanup {
                old: Some(mut old), ..
            }) = runs.remove(id)
            {
                old.closing = false;
                old.ended = true;
                runs.insert(id.to_owned(), Slot::Running(*old));
            }
            result
        };
        result?;
        if publication == RailPublication::Immediate && ending != Ending::StopForRemoval {
            self.bus.emit(actor, EventData::RailChanged);
        }
        if remove {
            // X11, X12: the node's bubbles end with it.
            report_undo(
                "close the node's Terminals",
                self.terminals.close_owned_by(id).await,
            );
        }
        Ok(())
    }
}

/// Kill a Terminal's program and return once it is observably not running (A16) or
/// `KILL_WAIT_BOUND` has passed, whichever comes first: `kill` itself returns as soon as the
/// program is reaped, which can race the Terminal's own reader thread noticing the program's end
/// and flipping `terminal.list`'s `running`, and a descendant left holding the PTY open can keep
/// that race from ever resolving (see `KILL_WAIT_BOUND`). `terminal::Shared::subscribe` checks
/// `handle` under the same lock `finish` clears it under, so subscribing before `kill` runs means
/// the exit event is never missed: either the receiver exists before `finish` publishes it, or
/// `subscribe` already found the Terminal gone and returned `NOT_FOUND`. `terminal_id` may already
/// be gone, from `kill` or from the subscribe, which is not a failure.
pub(super) async fn kill_or_already_gone(
    kill: &dyn KillsTerminals,
    terminals: &Terminals,
    terminal_id: &str,
) -> Result<(), RpcError> {
    let mut exit = terminals.subscribe(terminal_id).ok();
    let ended = if let Some(exit) = exit.as_mut() {
        exit_through_claude_code(terminals, terminal_id, exit).await
    } else {
        false
    };
    if !ended {
        match kill.kill(terminal_id).await {
            Err(err) if err.code != code::NOT_FOUND => return Err(err),
            _ => {}
        }
    }
    // A reaped program can precede the reader's TerminalExited publication.
    // Resume cleanup needs that publication before it can confirm the Terminal stopped.
    if let Some(mut exit) = exit
        && terminals
            .list()
            .iter()
            .any(|t| t.id == terminal_id && t.running)
    {
        let _ = tokio::time::timeout(KILL_WAIT_BOUND, wait_for_exit(&mut exit)).await;
    }
    Ok(())
}

/// How often `program_ended` looks at the program.
const PROGRAM_POLL: Duration = Duration::from_millis(50);

/// Ask a Terminal's Claude Code to end itself, so it counts the end as a clean one and not as a
/// failed start (`claude_code::EXIT_KEY`), and return whether the Terminal exited within
/// `CLEAN_EXIT_BOUND`. `exit` is the Terminal's own event stream, subscribed before the first
/// press. A `false` is reported on stderr, because the caller then kills the program.
pub(super) async fn exit_through_claude_code(
    terminals: &Terminals,
    terminal_id: &str,
    exit: &mut Receiver<Arc<EventData>>,
) -> bool {
    let presses = async {
        for _ in 0..claude_code::EXIT_PRESSES {
            if let Err(err) = terminals.write(terminal_id, claude_code::EXIT_KEY).await {
                return err;
            }
            tokio::time::sleep(claude_code::EXIT_PRESS_GAP).await;
        }
        std::future::pending().await
    };
    let outcome = tokio::time::timeout(CLEAN_EXIT_BOUND, async {
        tokio::select! {
            () = wait_for_exit(exit) => None,
            () = program_ended(terminals, terminal_id) => None,
            err = presses => Some(err),
        }
    })
    .await;
    match outcome {
        Ok(None) => true,
        // It ended between the subscribe and a press.
        Ok(Some(err)) if err.code == code::NOT_FOUND => true,
        Ok(Some(err)) => {
            eprintln!(
                "agents: could not type Claude Code's exit into terminal {terminal_id}: {err}; killing it"
            );
            false
        }
        Err(_) => {
            eprintln!(
                "agents: Claude Code did not exit within {CLEAN_EXIT_BOUND:?} of its exit keys in terminal {terminal_id}; killing it"
            );
            false
        }
    }
}

/// Resolve once the Terminal's program is reaped. A descendant that holds the PTY open keeps the
/// Terminal's exit event back, so the event alone would wait the whole bound for a program that
/// is already gone.
async fn program_ended(terminals: &Terminals, terminal_id: &str) {
    while !terminals.program_ended(terminal_id) {
        tokio::time::sleep(PROGRAM_POLL).await;
    }
}

/// Wait for a Terminal's own exit event, bounded by the caller. A lag never loses it, since it is
/// always the last event a Terminal sends. `Terminals` lives in an `Arc` for as long as the Daemon
/// does, so the channel this reads never closes while this can run; `Closed` is unreachable but
/// still has to be matched to satisfy `RecvError`.
pub(super) async fn wait_for_exit(events: &mut Receiver<Arc<EventData>>) {
    loop {
        match events.recv().await {
            Ok(event) if matches!(event.as_ref(), EventData::TerminalExited(_)) => return,
            Ok(_) | Err(RecvError::Lagged(_)) => {}
            Err(RecvError::Closed) => return,
        }
    }
}

/// Selected lifecycle calls leave Rail publication to their coordinator.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum RailPublication {
    Immediate,
    Coordinator,
}

/// How `Shared::end_with` ends an Agent's node.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Ending {
    /// Stop the program; the node stays on the Rail.
    Stop,
    /// A34: stop without publishing a Rail change before coordinated removal commits.
    StopForRemoval,
    /// A16: delete the node and its Worktree when landed.
    Remove,
    /// G5: delete the node and its Worktree whatever its state.
    Discard,
    /// A32: delete the node, keep its Worktree.
    Retire,
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use contracts::EventData;
    use contracts::terminal::{ExitedEvent, SpawnParams, TitleEvent};
    use rpc::Bus;

    use terminal::Terminals;
    use tokio::sync::broadcast;
    use tokio::sync::broadcast::error::TryRecvError;

    use tokio::time::Instant;

    use crate::tests::*;
    /// A lag skips the events it dropped, but must not end the wait on its own: only
    /// `TerminalExited` may. With capacity 1, every send before a read overwrites the one before
    /// it, so the receiver sees exactly one `Lagged`, then the last message sent.
    #[tokio::test]
    async fn wait_for_exit_keeps_waiting_past_a_lag() {
        let (tx, mut rx) = broadcast::channel(1);
        tx.send(Arc::new(EventData::TerminalTitle(TitleEvent {
            id: "1".into(),
            title: "a".into(),
        })))
        .unwrap();
        tx.send(Arc::new(EventData::TerminalTitle(TitleEvent {
            id: "1".into(),
            title: "b".into(),
        })))
        .unwrap();
        tx.send(Arc::new(EventData::TerminalExited(ExitedEvent {
            id: "1".into(),
            code: Some(0),
        })))
        .unwrap();

        crate::ending::wait_for_exit(&mut rx).await;

        // Had the lag ended the wait instead of being skipped past, `TerminalExited` would still
        // be sitting unread here.
        assert!(matches!(rx.try_recv(), Err(TryRecvError::Empty)));
    }
    /// An ordinary event (a title change, say output) must not end the wait on its own: only
    /// `TerminalExited` may. A large capacity means both sends land without a lag, so a mutant
    /// that returns on any `Ok(_)` would stop at the title and leave `TerminalExited` unread.
    #[tokio::test]
    async fn wait_for_exit_keeps_waiting_past_an_ordinary_event() {
        let (tx, mut rx) = broadcast::channel(8);
        tx.send(Arc::new(EventData::TerminalTitle(TitleEvent {
            id: "1".into(),
            title: "a".into(),
        })))
        .unwrap();
        tx.send(Arc::new(EventData::TerminalExited(ExitedEvent {
            id: "1".into(),
            code: Some(0),
        })))
        .unwrap();

        crate::ending::wait_for_exit(&mut rx).await;

        // Had the title event ended the wait, `TerminalExited` would still be sitting unread here.
        assert!(matches!(rx.try_recv(), Err(TryRecvError::Empty)));
    }
    /// A kill that fails must return its error at once, not wait for an exit event from a program
    /// the kill never touched: a real Terminal that ignores its exit keys proves the wait after the
    /// kill is skipped, since it would otherwise run out `KILL_WAIT_BOUND` before returning. The
    /// program reads raw, so the exit keys are input to it and not a signal.
    #[tokio::test(start_paused = true)]
    async fn kill_or_already_gone_returns_a_failed_kills_error_without_waiting() {
        let dir = tempfile::tempdir().unwrap();
        let terminals = Terminals::open(dir.path(), Bus::new()).unwrap();
        let spawned = spawn_raw(&terminals, dir.path(), "sleep 30").await;
        let began = Instant::now();

        let err = crate::ending::kill_or_already_gone(&FailingKill, &terminals, &spawned.id)
            .await
            .unwrap_err();

        assert_eq!(err.code, rpc::code::INTERNAL);
        assert!(began.elapsed() < crate::CLEAN_EXIT_BOUND + crate::KILL_WAIT_BOUND / 2);
    }

    /// What a kill finds of the program's input when it is asked to kill.
    struct RecordedKill(std::path::PathBuf, std::sync::Mutex<Vec<Vec<u8>>>);

    #[async_trait::async_trait]
    impl crate::KillsTerminals for RecordedKill {
        async fn kill(&self, _id: &str) -> Result<(), rpc::RpcError> {
            // The program's input thread writes after `Terminals::write` returns.
            for _ in 0..100 {
                let seen = std::fs::read(&self.0).unwrap_or_default();
                if seen.len() >= crate::claude_code::EXIT_PRESSES {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            self.1
                .lock()
                .unwrap()
                .push(std::fs::read(&self.0).unwrap_or_default());
            Ok(())
        }
    }

    /// Spawn `script` once its terminal reads raw, so `^C` reaches it as input, not as SIGINT.
    async fn spawn_raw(
        terminals: &Terminals,
        dir: &std::path::Path,
        script: &str,
    ) -> terminal::Spawned {
        let ready = dir.join("ready");
        let spawned = terminals
            .spawn(SpawnParams {
                cwd: dir.to_string_lossy().into_owned(),
                command: Some(vec![
                    "sh".into(),
                    "-c".into(),
                    format!("stty raw -echo; touch {}; {script}", ready.display()),
                ]),
                env: Default::default(),
                cols: 80,
                rows: 24,
            })
            .await
            .unwrap();
        while !ready.exists() {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        spawned
    }

    /// A7: stopping an Agent whose Claude Code exits on its exit keys types them into its
    /// Terminal and sends no signal, so Claude Code counts the end as a clean one.
    #[tokio::test]
    async fn a7_stop_types_claude_codes_exit_keys_and_sends_no_kill() {
        let dir = tempfile::tempdir().unwrap();
        let typed = dir.path().join("typed");
        let (arrived, parked) = tokio::sync::oneshot::channel();
        let arrived = std::sync::Mutex::new(Some(arrived));
        let (release, released) = std::sync::mpsc::channel();
        let released = std::sync::Mutex::new(released);
        let terminals = Arc::new(
            Terminals::open(dir.path(), Bus::new())
                .unwrap()
                .with_exit_hold(move || {
                    arrived.lock().unwrap().take().unwrap().send(()).unwrap();
                    released.lock().unwrap().recv().unwrap();
                }),
        );
        let spawned = spawn_raw(
            &terminals,
            dir.path(),
            &format!("head -c 3 > {}", typed.display()),
        )
        .await;
        let kill = Arc::new(RecordedKill(typed.clone(), Default::default()));
        let mut stopping = {
            let terminals = Arc::clone(&terminals);
            let kill = Arc::clone(&kill);
            let id = spawned.id.clone();
            tokio::spawn(async move {
                crate::ending::kill_or_already_gone(&*kill, &terminals, &id).await
            })
        };
        parked.await.unwrap();
        // Hold Terminal publication after process exit: cleanup must still await its event.
        let premature = tokio::time::timeout(crate::KILL_WAIT_BOUND / 2, &mut stopping).await;
        release.send(()).unwrap();
        assert!(premature.is_err(), "stop returned before Terminal exit");
        stopping.await.unwrap().unwrap();

        assert!(
            !terminals
                .list()
                .iter()
                .any(|t| t.id == spawned.id && t.running)
        );
        assert_eq!(std::fs::read(&typed).unwrap(), b"\x03\x03\x03");
        assert!(kill.1.lock().unwrap().is_empty(), "no kill was needed");
    }

    /// A7: a Claude Code that ignores its exit keys is killed once `CLEAN_EXIT_BOUND` has passed,
    /// after all three keys were typed, and the failure is reported on stderr.
    #[tokio::test(start_paused = true)]
    async fn a7_stop_kills_a_claude_code_that_ignores_its_exit_keys_after_the_bound() {
        let dir = tempfile::tempdir().unwrap();
        let typed = dir.path().join("typed");
        let terminals = Terminals::open(dir.path(), Bus::new()).unwrap();
        let spawned = spawn_raw(
            &terminals,
            dir.path(),
            &format!("cat > {}", typed.display()),
        )
        .await;
        let kill = RecordedKill(typed, Default::default());
        let began = Instant::now();

        crate::ending::kill_or_already_gone(&kill, &terminals, &spawned.id)
            .await
            .unwrap();

        assert!(began.elapsed() >= crate::CLEAN_EXIT_BOUND);
        assert_eq!(*kill.1.lock().unwrap(), vec![b"\x03\x03\x03".to_vec()]);
    }
}
