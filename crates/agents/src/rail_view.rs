use super::*;

pub(super) struct State {
    /// Since when an Agent without a Terminal has been `done`.
    pub(super) opened: i64,
}

impl Shared {
    pub(super) fn rail(&self) -> MutexGuard<'_, rail::Rail> {
        agent_lock(&self.rail).expect("rail lock")
    }

    pub(super) fn wait_of(&self, id: &str) -> Option<u32> {
        self.waits.lock().expect("waits lock").get(id).copied()
    }

    /// A33: replace the blockers Agents wait on. A running Agent whose reading changes gets a
    /// new `status_revision` and a Status event.
    pub(super) fn set_waits(&self, next: HashMap<String, u32>) {
        let mut runs = self.runs();
        let mut waits = self.waits.lock().expect("waits lock");
        for (id, slot) in runs.iter_mut() {
            let Slot::Running(run) = slot else { continue };
            let Some(status) = run.adapter.status() else {
                continue;
            };
            let before = waiting_on(status, waits.get(id).copied());
            let after = waiting_on(status, next.get(id).copied());
            if run.ended || (before.kind, &before.label) == (after.kind, &after.label) {
                continue;
            }
            run.status_revision = run
                .status_revision
                .checked_add(1)
                .expect("Status revision exhausted");
            self.bus.emit(
                Actor::daemon(),
                EventData::AgentStatus(Box::new(StatusEvent {
                    id: id.clone(),
                    attempt: run.attempt.clone(),
                    status_revision: run.status_revision.to_string(),
                    status: after,
                })),
            );
        }
        *waits = next;
    }

    pub(super) fn runs(&self) -> MutexGuard<'_, HashMap<String, Slot>> {
        agent_lock(&self.runs).expect("runs lock")
    }

    pub(super) fn mark_starting(&self, id: &str, attempt: String) {
        let since = (self.clock)();
        self.runs().insert(
            id.to_owned(),
            Slot::Starting {
                attempt,
                since,
                named: false,
                held: VecDeque::new(),
            },
        );
    }

    /// Every node leaving the module carries its Attempt's Status. During resume the old
    /// Terminal stays visible until acknowledgement, but its ended Status would close the new
    /// Attempt's Message receiver (B36). A node without a run is done after a Daemon reopen.
    pub(super) fn present(&self, mut node: RailNode, runs: &HashMap<String, Slot>) -> RailNode {
        node.status_revision = match runs.get(&node.id) {
            Some(Slot::Running(run)) => Some(run.status_revision.to_string()),
            // B36: inheriting the ended run's revision closes Messages on the new Attempt.
            Some(Slot::Starting { .. } | Slot::Resuming { .. }) => Some("1".into()),
            _ => None,
        };
        let live = match runs.get(&node.id) {
            Some(Slot::Running(run)) => run
                .adapter
                .status()
                .map(|status| waiting_on(status, self.wait_of(&node.id))),
            Some(Slot::Starting { since, .. } | Slot::Resuming { since, .. }) => Some(Status {
                kind: Kind::Working,
                label: "starting".into(),
                since: *since,
            }),
            Some(Slot::ResumeCleanup {
                terminal_id, error, ..
            }) => {
                node.terminal_id = Some(terminal_id.clone());
                Some(Status {
                    kind: Kind::Error,
                    label: error.message.clone(),
                    since: self.rail_view.opened,
                })
            }
            None | Some(Slot::Closing) => None,
        };
        node.status = Some(live.unwrap_or_else(|| Status {
            kind: Kind::Done,
            label: "terminal gone".into(),
            since: self.rail_view.opened,
        }));
        // A22: exited (no live process, and no start in flight), with A21 data to resume.
        let exited = match runs.get(&node.id) {
            Some(Slot::Running(run)) => run.ended,
            Some(
                Slot::Starting { .. }
                | Slot::Resuming { .. }
                | Slot::ResumeCleanup { .. }
                | Slot::Closing,
            ) => false,
            None => true,
        };
        node.can_resume &= exited;
        let channels = self.channels.lock().expect("channels lock");
        node.channel = channel_in(&channels, &node.id, runs.get(&node.id));
        node.inbox = self.events.inbox.fact(&node.id);
        node.stray = self
            .strays
            .lock()
            .expect("scan found lock")
            .get(&node.id)
            .cloned()
            .unwrap_or_default();
        node
    }

    pub(super) fn node(&self, id: &str) -> Result<RailNode, RpcError> {
        let rail = self.rail();
        let runs = self.runs();
        Ok(self.present(rail.node(id)?, &runs))
    }

    pub(super) fn tree(&self) -> Result<Vec<RailNode>, RpcError> {
        let rail = self.rail();
        let runs = self.runs();
        Ok(rail
            .tree()?
            .into_iter()
            .map(|node| self.present(node, &runs))
            .collect())
    }
}

impl Agents {
    /// B32: run a synchronous input-recording operation against authoritative ownership.
    ///
    /// Daemon-internal only: never exposed through `Module::call`, and grants no Reach or
    /// Steer authority. Locks Rail then Attempts, like lifecycle attachment/removal. The cut
    /// is the targeted read with both locks held; attach, detach, reassignment, removal,
    /// reopen and Status/Attempt transitions cannot cross it until `record` returns.
    /// `None` means no attached receiver (also after reopen), not a cached negative.
    ///
    /// `record` may lock the Message store only after these locks. It must not await, perform
    /// Terminal I/O, emit events, reenter Agents, or take a lock whose holder calls Agents.
    /// Record all ownership-dependent effects inside it; escaped facts are observations,
    /// never authority for a later write. Schedule delivery only after this method returns.
    /// SQL errors or ambiguous ownership return an error without calling `record`; its error
    /// propagates unchanged and does not roll back effects it made: `record` owns its transaction.
    /// Poisoned lifecycle locks panic, as other Agents operations do.
    pub fn with_terminal_owner<R>(
        &self,
        terminal: &str,
        record: impl FnOnce(Option<&TerminalOwner>) -> Result<R, RpcError>,
    ) -> Result<R, RpcError> {
        let mut rail = self.shared.rail();
        let runs = self.shared.runs();
        let owner = rail.terminal_owner(terminal)?.map(|(id, attempt)| {
            let (status_revision, ended) = match runs.get(&id) {
                Some(Slot::Running(run)) => (
                    Some(run.status_revision.to_string()),
                    run.ended
                        || run.closing
                        || run.adapter.is_ended()
                        || run
                            .adapter
                            .status()
                            .is_none_or(|status| status.kind == Kind::Done),
                ),
                Some(Slot::Starting { .. } | Slot::Resuming { .. }) => (Some("1".into()), false),
                None | Some(Slot::Closing | Slot::ResumeCleanup { .. }) => (None, true),
            };
            TerminalOwner {
                id,
                attempt,
                status_revision,
                ended,
            }
        });
        record(owner.as_ref())
    }

    /// H21: the Agent a call from process `pid` is by, found by walking its parents to the
    /// `claude` of a running Agent; `None` for a process that descends from no Agent.
    pub fn agent_of_peer(&self, pid: u32) -> Option<String> {
        let claudes: HashMap<u32, String> = self
            .running_programs()
            .into_iter()
            .map(|(id, pid)| (pid, id))
            .collect();
        peer::agent_above(pid, &claudes, peer::parent_of)
    }

    /// D8: each running Agent with the pid of its program; an Agent that is not running has none.
    pub fn running_programs(&self) -> Vec<(String, u32)> {
        self.shared
            .runs()
            .iter()
            .filter_map(|(id, slot)| match slot {
                Slot::Running(run) => self
                    .shared
                    .terminals
                    .program_pid(&run.terminal_id)
                    .map(|pid| (id.clone(), pid)),
                _ => None,
            })
            .collect()
    }

    /// A33: `waits` maps each Agent to the lowest open blocker of its blocked Todos.
    pub fn set_waits(&self, waits: HashMap<String, u32>) {
        self.shared.set_waits(waits);
    }

    /// O4, O13: replace the order of an Agent or a Door, and Steer it the new one unless the caller is
    /// the Agent itself or a Takeover is active. A child change by an Agent waits for
    /// the physical Steer and submit. Post-storage errors name the stored Order:
    /// A refused Steer reports no bytes; UNKNOWN_OUTCOME forbids blind retry.
    /// Direct user changes retain O4's asynchronous Steer.
    pub(super) async fn set_order(
        &self,
        ctx: &Ctx,
        params: SetOrderParams,
    ) -> Result<RailNode, RpcError> {
        let SetOrderParams { id, order } = params;
        valid_explicit_order(&order)?;
        self.shared.rail().node(&id)?;
        let allowed = match ctx.actor.kind {
            ActorKind::User => true,
            // O13: a Door sets the order only of an Agent it, or an Agent under it, spawned.
            ActorKind::Agent => {
                ctx.actor.id == id || self.shared.rail().spawned_under(&id, &ctx.actor.id)?
            }
            ActorKind::Ext => false,
        };
        if !allowed {
            return Err(RpcError::forbidden(format!(
                "{} may not set the order of agent {id}",
                ctx.actor.id
            )));
        }
        let node = self.shared.rail().set_order(&id, &order)?;
        ctx.touch(contracts::Verb::Wrote, &format!("agent:{id}"))?;
        ctx.emit(EventData::RailChanged);
        let takeover = self
            .shared
            .takeover
            .as_ref()
            .is_some_and(|active| active(&id));
        if ctx.actor.kind == ActorKind::Agent && ctx.actor.id != id {
            if takeover {
                return Err(RpcError::conflict(
                    "Order stored; Takeover refused the Steer; no bytes written",
                ));
            }
            self.shared
                .prompt(&id, &order::steer(&order, None), None, None, false)
                .await
                .map_err(|err| match err {
                    SteerError::Refused(reason) => {
                        let mut error = RpcError::from(reason);
                        error.message = format!(
                            "Order stored; Steer refused; no bytes written: {}",
                            error.message
                        );
                        error
                    }
                    SteerError::Uncertain(reason) => RpcError::unknown_outcome(format!(
                        "Order stored; Steer bytes may have been written; do not retry: {reason}"
                    )),
                })?;
        } else if ctx.actor.id != id && !takeover {
            let shared = Arc::clone(&self.shared);
            let text = order::steer(&order, None);
            tokio::spawn(async move {
                if let Err(err) = shared.prompt(&id, &text, None, None, false).await {
                    eprintln!("agents: the new order of {id} was not steered: {err}");
                }
            });
        }
        self.shared.node(&node.id)
    }
}

/// O7: an explicit order, also held to the ask cap.
pub(super) fn valid_explicit_order(order: &Order) -> Result<(), RpcError> {
    order
        .validate_explicit()
        .map_err(|message| RpcError::new(code::INVALID_PARAMS, message))
}

/// O1: an order of the right shape, or `INVALID_PARAMS`.
pub(super) fn valid_order(order: &Order) -> Result<(), RpcError> {
    order
        .validate()
        .map_err(|message| RpcError::new(code::INVALID_PARAMS, message))
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::Arc;

    use contracts::agent::NodeKind;

    use crate::{Slot, rail};

    use crate::tests::*;
    #[test]
    fn b32_owner_reads_see_external_attachment_while_a_peer_pins_an_older_snapshot() {
        let (dir, _, shared) = shared_over_temp_dir(Arc::new(|| 0));
        let id = shared
            .rail()
            .insert(NodeKind::Agent, "receiver", None, None)
            .unwrap()
            .id;
        let agents = owner_agents(shared, dir.path());
        agents
            .with_terminal_owner("terminal", |owner| {
                assert!(owner.is_none());
                Ok(())
            })
            .unwrap();
        let peer = rusqlite::Connection::open(dir.path().join("agents.db")).unwrap();
        peer.execute_batch("BEGIN; SELECT rootpage FROM sqlite_schema;")
            .unwrap();
        let writer = rusqlite::Connection::open(dir.path().join("agents.db")).unwrap();
        writer
            .execute(
                "UPDATE nodes SET terminal_id = 'terminal', attempt = 7 WHERE id = ?",
                [&id],
            )
            .unwrap();
        agents
            .with_terminal_owner("terminal", |owner| {
                let owner = owner.unwrap();
                assert_eq!(owner.id, id);
                assert_eq!(owner.attempt.as_deref(), Some("7"));
                Ok(())
            })
            .unwrap();
        peer.execute_batch("ROLLBACK").unwrap();
    }

    #[test]
    fn b32_owner_read_leases_expire_and_rearm_independently_of_input() {
        for ongoing in [false, true] {
            let (dir, _, shared) = shared_over_temp_dir(Arc::new(|| 0));
            let id = shared
                .rail()
                .insert(NodeKind::Agent, "receiver", None, None)
                .unwrap()
                .id;
            let agents = owner_agents(shared, dir.path());
            let db = rusqlite::Connection::open(dir.path().join("agents.db")).unwrap();
            // The second burst must acquire a new lease after the first burst's
            // checkpoint completed, even when all intervening wakeups were coalesced.
            for _ in 0..2 {
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
                loop {
                    agents.with_terminal_owner("unowned", |_| Ok(())).unwrap();
                    db.busy_timeout(std::time::Duration::from_millis(500))
                        .unwrap();
                    db.execute("UPDATE nodes SET attempt = attempt + 1 WHERE id = ?", [&id])
                        .unwrap();
                    db.busy_timeout(std::time::Duration::ZERO).unwrap();
                    let (busy, frames, copied): (i64, i64, i64) = db
                        .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| {
                            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
                        })
                        .unwrap();
                    if busy != 0 && frames > copied && copied >= 0 {
                        break;
                    }
                    assert!(
                        std::time::Instant::now() < deadline,
                        "owner reads never shared a bounded WAL read lease"
                    );
                    std::thread::yield_now();
                }
                // Uncopied frames witness an active reader, unlike (-1, -1) for a competing
                // checkpoint. Further input must neither release nor renew that reader.
                db.busy_timeout(std::time::Duration::from_millis(500))
                    .unwrap();
                let stop = std::sync::atomic::AtomicBool::new(false);
                std::thread::scope(|scope| {
                    let input = ongoing.then(|| {
                        scope.spawn(|| {
                            while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                                agents.with_terminal_owner("unowned", |_| Ok(())).unwrap();
                            }
                        })
                    });
                    let checkpoint = db.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| {
                        Ok((
                            row.get::<_, i64>(0)?,
                            row.get::<_, i64>(1)?,
                            row.get::<_, i64>(2)?,
                        ))
                    });
                    stop.store(true, std::sync::atomic::Ordering::Relaxed);
                    if let Some(input) = input {
                        input.join().unwrap();
                    }
                    assert_eq!(checkpoint.unwrap(), (0, 0, 0), "ongoing input: {ongoing}");
                });
            }
        }
    }

    #[test]
    fn b32_agent_and_door_owner_facts_match_receiver_binding() {
        for kind in [NodeKind::Agent, NodeKind::Workstream] {
            let (dir, _, shared) = shared_over_temp_dir(Arc::new(|| 0));
            let id = shared
                .rail()
                .insert(kind, "receiver", None, Some("terminal"))
                .unwrap()
                .id;
            let attempt = shared.rail().allocate_attempt(&id).unwrap();
            shared.mark_starting(&id, attempt);
            let agents = owner_agents(Arc::clone(&shared), dir.path());
            agents
                .with_terminal_owner("terminal", |owner| {
                    let owner = owner.unwrap();
                    assert_eq!(owner.id, id);
                    assert_eq!(owner.attempt.as_deref(), Some("1"));
                    assert_eq!(owner.status_revision.as_deref(), Some("1"));
                    assert!(!owner.ended);
                    Ok(())
                })
                .unwrap();
        }
    }
    #[test]
    fn b32_repeated_owner_reads_bind_each_terminal_afresh() {
        let (dir, _, shared) = shared_over_temp_dir(Arc::new(|| 0));
        for terminal in ["first", "second"] {
            shared
                .rail()
                .insert(NodeKind::Agent, terminal, None, Some(terminal))
                .unwrap();
        }
        let agents = owner_agents(shared, dir.path());
        for (terminal, expected) in [
            ("first", Some("1")),
            ("missing", None),
            ("second", Some("2")),
            ("first", Some("1")),
            ("missing", None),
        ] {
            agents
                .with_terminal_owner(terminal, |owner| {
                    assert_eq!(owner.map(|owner| owner.id.as_str()), expected);
                    Ok(())
                })
                .unwrap();
        }
    }
    #[test]
    fn b32_a_warm_owner_read_sees_an_attempt_committed_by_another_connection() {
        let (dir, _, shared) = shared_over_temp_dir(Arc::new(|| 0));
        let id = shared
            .rail()
            .insert(NodeKind::Agent, "receiver", None, Some("terminal"))
            .unwrap()
            .id;
        let agents = owner_agents(Arc::clone(&shared), dir.path());
        agents
            .with_terminal_owner("terminal", |owner| {
                assert!(owner.unwrap().attempt.is_none());
                Ok(())
            })
            .unwrap();
        let db = rusqlite::Connection::open(dir.path().join("agents.db")).unwrap();
        db.execute("UPDATE nodes SET attempt = 7 WHERE id = ?", [&id])
            .unwrap();
        agents
            .with_terminal_owner("terminal", |owner| {
                let owner = owner.unwrap();
                assert_eq!(owner.id, id);
                assert_eq!(owner.attempt.as_deref(), Some("7"));
                assert!(owner.ended);
                Ok(())
            })
            .unwrap();
    }
    #[test]
    fn b32_a_warm_owner_read_rejects_ambiguity_then_recovers() {
        let (dir, _, shared) = shared_over_temp_dir(Arc::new(|| 0));
        let id = shared
            .rail()
            .insert(NodeKind::Agent, "first", None, Some("terminal"))
            .unwrap()
            .id;
        let agents = owner_agents(Arc::clone(&shared), dir.path());
        agents
            .with_terminal_owner("terminal", |owner| {
                assert_eq!(owner.unwrap().id, id);
                Ok(())
            })
            .unwrap();
        let other = shared
            .rail()
            .insert(NodeKind::Agent, "second", None, Some("terminal"))
            .unwrap()
            .id;
        let error = agents
            .with_terminal_owner("terminal", |_| -> Result<(), rpc::RpcError> {
                panic!("must not record input for an ambiguous receiver")
            })
            .unwrap_err();
        assert_eq!(error.code, rpc::code::CONFLICT);
        shared.rail().detach_terminal(&other).unwrap();
        agents
            .with_terminal_owner("terminal", |owner| {
                assert_eq!(owner.unwrap().id, id);
                Ok(())
            })
            .unwrap();
    }
    #[test]
    fn b32_owner_read_stops_at_the_second_receiver() {
        let (dir, _, shared) = shared_over_temp_dir(Arc::new(|| 0));
        for name in ["first", "second", "third"] {
            shared
                .rail()
                .insert(NodeKind::Agent, name, None, Some("terminal"))
                .unwrap();
        }
        let agents = owner_agents(shared, dir.path());
        let db = rusqlite::Connection::open(dir.path().join("agents.db")).unwrap();
        // A third step must fail, so CONFLICT proves the query stopped at ambiguity
        // rather than scanning the remaining receivers before rejecting the input.
        db.execute_batch(
            "ALTER TABLE nodes RENAME TO receivers;
             CREATE VIEW nodes AS
             SELECT id, terminal_id,
                    CASE WHEN id = 3 THEN json_extract('invalid', '$') ELSE attempt END AS attempt
             FROM receivers;",
        )
        .unwrap();
        let error = agents
            .with_terminal_owner("terminal", |_| -> Result<(), rpc::RpcError> {
                panic!("must not record input for an ambiguous receiver")
            })
            .unwrap_err();
        assert_eq!(error.code, rpc::code::CONFLICT);
    }

    #[test]
    fn b32_a_warm_owner_read_recovers_after_schema_replacement() {
        let (dir, _, shared) = shared_over_temp_dir(Arc::new(|| 0));
        let agents = owner_agents(Arc::clone(&shared), dir.path());
        agents
            .with_terminal_owner("terminal", |owner| {
                assert!(owner.is_none());
                Ok(())
            })
            .unwrap();
        let db = rusqlite::Connection::open(dir.path().join("agents.db")).unwrap();
        db.execute_batch("DROP TABLE nodes").unwrap();
        let error = agents
            .with_terminal_owner("terminal", |_| -> Result<(), rpc::RpcError> {
                panic!("must not record input after an owner query fails")
            })
            .unwrap_err();
        assert_eq!(error.code, rpc::code::INTERNAL);
        assert!(shared.rail.try_lock().is_ok());
        assert!(shared.runs.try_lock().is_ok());
        db.execute_batch(
            "CREATE TABLE nodes (id INTEGER, terminal_id TEXT, attempt INTEGER);
             INSERT INTO nodes VALUES (42, 'terminal', 9);",
        )
        .unwrap();
        agents
            .with_terminal_owner("terminal", |owner| {
                let owner = owner.unwrap();
                assert_eq!(owner.id, "42");
                assert_eq!(owner.attempt.as_deref(), Some("9"));
                Ok(())
            })
            .unwrap();
    }
    #[test]
    fn r16_owner_read_recovers_after_row_conversion_error() {
        let (dir, _, shared) = shared_over_temp_dir(Arc::new(|| 0));
        shared
            .rail()
            .insert(NodeKind::Agent, "receiver", None, Some("terminal"))
            .unwrap();
        let agents = owner_agents(shared, dir.path());
        let db = rusqlite::Connection::open(dir.path().join("agents.db")).unwrap();
        db.execute("UPDATE nodes SET attempt = 'invalid'", [])
            .unwrap();
        let error = agents
            .with_terminal_owner("terminal", |_| -> Result<(), rpc::RpcError> {
                panic!("a failed owner conversion must not record input")
            })
            .unwrap_err();
        assert_eq!(error.code, rpc::code::INTERNAL);
        db.execute("UPDATE nodes SET attempt = 9", []).unwrap();
        agents
            .with_terminal_owner("terminal", |owner| {
                assert_eq!(owner.unwrap().attempt.as_deref(), Some("9"));
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn b32_recording_failure_releases_lifecycle_locks() {
        let (dir, _, shared) = shared_over_temp_dir(Arc::new(|| 0));
        let agents = owner_agents(Arc::clone(&shared), dir.path());
        let error = agents
            .with_terminal_owner("terminal", |_| -> Result<(), rpc::RpcError> {
                Err(rpc::RpcError::conflict("record failed"))
            })
            .unwrap_err();
        assert_eq!(error.message, "record failed");
        assert!(shared.rail.try_lock().is_ok());
        assert!(shared.runs.try_lock().is_ok());
    }
    #[test]
    fn b32_pending_resume_binds_the_new_attempt_not_the_ended_run() {
        let (dir, _, shared) = shared_over_temp_dir(Arc::new(|| 0));
        let id = shared
            .rail()
            .insert(NodeKind::Workstream, "door", None, Some("old-terminal"))
            .unwrap()
            .id;
        shared.rail().allocate_attempt(&id).unwrap();
        let attempt = shared.rail().allocate_attempt(&id).unwrap();
        let (done, _wait) = tokio::sync::oneshot::channel();
        shared.runs().insert(
            id.clone(),
            Slot::Resuming {
                attempt,
                since: 0,
                old: None,
                conversation_id: "conversation".into(),
                cwd: "/".into(),
                prompt: None,
                starred: false,
                terminal_id: Some("unacknowledged-terminal".into()),
                held: VecDeque::new(),
                done,
            },
        );
        let agents = owner_agents(shared, dir.path());
        agents
            .with_terminal_owner("old-terminal", |owner| {
                let owner = owner.unwrap();
                assert_eq!(owner.attempt.as_deref(), Some("2"));
                assert_eq!(owner.status_revision.as_deref(), Some("1"));
                assert!(!owner.ended);
                Ok(())
            })
            .unwrap();
        agents
            .with_terminal_owner("unacknowledged-terminal", |owner| {
                assert!(owner.is_none());
                Ok(())
            })
            .unwrap();
    }
    #[test]
    fn b32_closed_receiver_cannot_be_revived_by_an_attached_terminal() {
        let (dir, _, shared) = shared_over_temp_dir(Arc::new(|| 0));
        let id = shared
            .rail()
            .insert(NodeKind::Agent, "receiver", None, Some("terminal"))
            .unwrap()
            .id;
        shared.rail().allocate_attempt(&id).unwrap();
        shared.runs().insert(id, Slot::Closing);
        let agents = owner_agents(shared, dir.path());
        agents
            .with_terminal_owner("terminal", |owner| {
                let owner = owner.unwrap();
                assert_eq!(owner.attempt.as_deref(), Some("1"));
                assert!(owner.status_revision.is_none());
                assert!(owner.ended);
                Ok(())
            })
            .unwrap();
    }
    #[tokio::test]
    async fn b32_running_and_failed_owner_facts_match_existing_rail_binding() {
        let watched = Watched::start();
        watched
            .shared
            .rail()
            .insert(NodeKind::Agent, "receiver", None, Some("1"))
            .unwrap();
        watched.shared.rail().allocate_attempt("1").unwrap();
        let agents = owner_agents(Arc::clone(&watched.shared), watched._dir.path());
        for failed in [false, true] {
            if failed {
                let mut runs = watched.shared.runs();
                let Some(Slot::Running(run)) = runs.get_mut("1") else {
                    unreachable!()
                };
                run.adapter.fail("failed".into());
                run.status_revision += 1;
            }
            let node = watched.shared.node("1").unwrap();
            agents
                .with_terminal_owner("1", |owner| {
                    let owner = owner.unwrap();
                    assert_eq!(owner.attempt, node.attempt);
                    assert_eq!(owner.status_revision, node.status_revision);
                    assert_eq!(owner.ended, failed);
                    Ok(())
                })
                .unwrap();
        }
        assert!(matches!(
            agents.prompt("1", "after fatal failure").await,
            Err(crate::PromptError::NotFound { .. })
        ));
        watched.watcher.abort();
    }
    #[test]
    fn b32_concurrent_attach_invalidates_an_unowned_read() {
        owner_transition(
            false,
            |shared, id, _| {
                shared.rail().attach_terminal(id, "terminal").unwrap();
            },
            Some(("1", "1")),
        );
    }
    #[test]
    fn b32_concurrent_detach_invalidates_an_owned_read() {
        owner_transition(
            true,
            |shared, id, _| {
                shared.rail().detach_terminal(id).unwrap();
            },
            None,
        );
    }
    #[test]
    fn b32_concurrent_reassignment_returns_the_new_receiver() {
        owner_transition(
            true,
            |shared, id, _| {
                let mut rail = shared.rail();
                rail.detach_terminal(id).unwrap();
                let next = rail
                    .insert(NodeKind::Workstream, "door", None, Some("terminal"))
                    .unwrap()
                    .id;
                rail.allocate_attempt(&next).unwrap();
            },
            Some(("2", "1")),
        );
    }
    #[test]
    fn b32_concurrent_removal_leaves_no_receiver() {
        owner_transition(
            true,
            |shared, id, _| {
                let mut rail = shared.rail();
                let mut runs = shared.runs();
                rail.remove(id).unwrap();
                runs.remove(id);
            },
            None,
        );
    }
    #[test]
    fn b32_concurrent_reopen_unlinks_terminal_ownership() {
        owner_transition(
            true,
            |shared, _, path| {
                let mut rail = shared.rail();
                let mut runs = shared.runs();
                *rail = rail::Rail::open(&path.join("agents.db")).unwrap();
                runs.clear();
            },
            None,
        );
    }
    #[test]
    fn b32_concurrent_attempt_change_fences_the_old_receiver() {
        owner_transition(
            true,
            |shared, id, _| {
                let mut rail = shared.rail();
                let attempt = rail.allocate_attempt(id).unwrap();
                shared.mark_starting(id, attempt);
            },
            Some(("1", "2")),
        );
    }
}
