//! The Decision store (`scenarios/decisions.md` H2 to H7, H10): one open Decision per Agent, in
//! memory only, so none outlives the Daemon (H5). The vendor's payload and reply text are
//! `claude_code`'s; this module carries opaque text.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use contracts::decision::{Answer, ClearedEvent, Decision, Outcome, PermissionOutput};
use contracts::{Actor, EventData};
use rpc::{RpcError, code};
use serde_json::Value;
use tokio::sync::oneshot;

use crate::claude_code;
use crate::{Observation, Shared};

/// What the waiting `agent.permission` call returns: the text `rup permission` prints, or the
/// reason it exits 1.
type Reply = Result<String, RpcError>;

struct Open {
    decision: Decision,
    /// The Attempt the Decision was opened for, so the Observations it causes reach that one.
    attempt: String,
    /// The hook's waiting call; `None` once its connection closed at its `timeout` (H7(c)) and
    /// for a Decision opened without a hook to reply to.
    waiter: Option<oneshot::Sender<Reply>>,
}

pub struct Decisions {
    open: Mutex<HashMap<String, Open>>,
    next: Mutex<u64>,
    /// H4: the secret the App holds; `None` refuses every answer.
    proof: Option<String>,
    /// The `timeout` of the hook's settings (H9); a close at or after it is not a No or an Esc.
    hook_timeout: Duration,
}

impl Decisions {
    pub fn new(proof: Option<String>, hook_timeout: Duration) -> Self {
        Self {
            open: Mutex::new(HashMap::new()),
            next: Mutex::new(0),
            proof,
            hook_timeout,
        }
    }

    pub(crate) fn set_proof(&mut self, proof: Option<String>) {
        self.proof = proof;
    }

    fn open(&self) -> MutexGuard<'_, HashMap<String, Open>> {
        self.open.lock().expect("decisions lock")
    }

    fn next_id(&self) -> String {
        let mut next = self.next.lock().expect("decision id lock");
        *next += 1;
        format!("d{next}")
    }

    /// The open Decisions, oldest first.
    pub fn list(&self) -> Vec<Decision> {
        let mut all: Vec<_> = self
            .open()
            .values()
            .map(|open| open.decision.clone())
            .collect();
        all.sort_by(|a, b| (a.opened_at, &a.id).cmp(&(b.opened_at, &b.id)));
        all
    }

    /// Clear Agent `agent`'s Decision, if it has one, and say so. The waiting hook gets `reason`.
    pub(crate) fn clear(&self, bus: &rpc::Bus, agent: &str, outcome: Outcome, reason: &str) {
        let Some(open) = self.open().remove(agent) else {
            return;
        };
        cleared(bus, &open.decision.id, outcome);
        if let Some(waiter) = open.waiter {
            let _ = waiter.send(Err(RpcError::new(code::CONFLICT, reason)));
        }
    }
}

fn cleared(bus: &rpc::Bus, id: &str, outcome: Outcome) {
    bus.emit(
        Actor::daemon(),
        EventData::DecisionCleared(ClearedEvent {
            id: id.to_owned(),
            outcome,
        }),
    );
}

/// Dropped with the `agent.permission` call it belongs to: if the call is dropped while its
/// Decision is still open, the hook's own side closed the connection (H7(b), H7(c)).
struct HookGuard {
    shared: Arc<Shared>,
    agent: String,
    decision: String,
    started: i64,
    armed: bool,
}

impl Drop for HookGuard {
    fn drop(&mut self) {
        if self.armed {
            self.shared
                .hook_closed(&self.agent, &self.decision, self.started);
        }
    }
}

impl Shared {
    /// H2: open Agent `id`'s Decision for `payload` and, for one with a hook to reply to, wait
    /// until it is answered or cleared.
    pub(crate) async fn permission(
        self: &Arc<Self>,
        actor: Actor,
        id: &str,
        payload: Value,
    ) -> Result<PermissionOutput, RpcError> {
        let request = claude_code::permission_request(&payload);
        let started = (self.clock)();
        let (rx, attempt, decision) = {
            // `runs` before `decisions`, as every other path: an Agent that ends cannot be left
            // with a Decision opened after its clear.
            let runs = self.runs();
            let Some(crate::Slot::Running(run)) = runs.get(id) else {
                return Err(RpcError::not_found(format!("agent {id}")));
            };
            if run.ended || run.closing {
                return Err(RpcError::not_found(format!("agent {id} has ended")));
            }
            let decision = Decision {
                id: self.decisions.next_id(),
                agent: id.to_owned(),
                tool: request.tool,
                args: request.args,
                opened_at: started,
                answerable: request.answerable,
            };
            let (tx, rx) = oneshot::channel();
            let waiter = decision.answerable.then_some(tx);
            let mut open = self.decisions.open();
            let replaced = open.insert(
                id.to_owned(),
                Open {
                    decision: decision.clone(),
                    attempt: run.attempt.clone(),
                    waiter,
                },
            );
            if let Some(old) = replaced {
                cleared(&self.bus, &old.decision.id, Outcome::Replaced);
                if let Some(waiter) = old.waiter {
                    let _ = waiter.send(Err(RpcError::new(code::CONFLICT, "superseded")));
                }
            }
            self.bus
                .emit(actor.clone(), EventData::DecisionOpened(decision.clone()));
            (rx, run.attempt.clone(), decision)
        };
        if let Err(err) = self.observe(
            actor,
            id,
            &attempt,
            Observation::Signal(claude_code::as_permission_signal(payload)),
        ) {
            self.decisions
                .clear(&self.bus, id, Outcome::AgentGone, "the Agent ended");
            return Err(err);
        }
        if !decision.answerable {
            return Ok(PermissionOutput {
                output: String::new(),
            });
        }
        let mut guard = HookGuard {
            shared: Arc::clone(self),
            agent: id.to_owned(),
            decision: decision.id,
            started,
            armed: true,
        };
        let reply = rx.await;
        guard.armed = false;
        match reply {
            Ok(reply) => reply.map(|output| PermissionOutput { output }),
            Err(_) => Err(RpcError::internal("the Decision was dropped")),
        }
    }

    /// H3: the user's `answer` for Decision `id`.
    pub(crate) fn answer(
        &self,
        actor: Actor,
        id: &str,
        answer: Answer,
        proof: Option<&str>,
    ) -> Result<(), RpcError> {
        if self.decisions.proof.is_none() || self.decisions.proof.as_deref() != proof {
            return Err(RpcError::forbidden("decision.answer needs the App's proof"));
        }
        let (agent, attempt) = {
            let mut open = self.decisions.open();
            let Some((agent, entry)) = open.iter_mut().find(|(_, o)| o.decision.id == id) else {
                return Err(RpcError::not_found(format!("decision {id}")));
            };
            if !entry.decision.answerable {
                return Err(RpcError::new(
                    code::INVALID_PARAMS,
                    format!("decision {id} can only be answered in the Terminal"),
                ));
            }
            let waiter = entry.waiter.take().expect("an answerable Decision waits");
            if waiter.send(Ok(claude_code::reply(answer))).is_err() {
                // The hook has gone; its guard is about to clear the Decision as a close.
                return Err(RpcError::not_found(format!("decision {id}")));
            }
            let agent = agent.clone();
            let attempt = entry.attempt.clone();
            open.remove(&agent);
            (agent, attempt)
        };
        let outcome = match answer {
            Answer::Allow => Outcome::Allow,
            Answer::Deny => Outcome::Deny,
        };
        cleared(&self.bus, id, outcome);
        if let Err(err) = self.observe(actor, &agent, &attempt, Observation::Answered) {
            eprintln!("agents: {agent}: answered Decision {id} after the Agent ended: {err}");
        }
        Ok(())
    }

    /// The hook's connection closed while its Decision was open and nothing of the Daemon's made
    /// it close (H7(b), H7(c)).
    fn hook_closed(&self, agent: &str, decision: &str, started: i64) {
        let attempt = {
            let mut open = self.decisions.open();
            let Some(entry) = open.get_mut(agent).filter(|o| o.decision.id == decision) else {
                return;
            };
            let waited = (self.clock)() - started;
            if waited >= i64::try_from(self.decisions.hook_timeout.as_millis()).unwrap_or(i64::MAX)
            {
                entry.decision.answerable = false;
                entry.waiter = None;
                eprintln!("agents: {agent}: the hook timed out; answer in the Terminal");
                return;
            }
            let attempt = entry.attempt.clone();
            open.remove(agent);
            attempt
        };
        cleared(&self.bus, decision, Outcome::Terminal);
        if let Err(err) = self.observe(Actor::daemon(), agent, &attempt, Observation::Dismissed) {
            eprintln!(
                "agents: {agent}: dismissed Decision {decision} after the Agent ended: {err}"
            );
        }
    }
}
