//! Decisions: the Card that blocks an Agent on a live hook or tool call (`GLOSSARY.md`). Only the
//! user answers one; see `scenarios/decisions.md`.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "decision/")]
pub struct Decision {
    pub id: String,
    pub agent: String,
    pub tool: String,
    pub args: String,
    /// Milliseconds since the Unix epoch.
    #[ts(type = "number")]
    pub opened_at: i64,
    /// `false` when the Daemon opened it without a live hook to reply to (H2's `AskUserQuestion`,
    /// H7(c), H8's unreadable payload): `decision.answer` is then `INVALID_PARAMS`.
    pub answerable: bool,
}

/// Called by `rup permission`; waits until the Decision it opens is answered or cleared, except
/// when the Decision is not `answerable` (H2's `AskUserQuestion`), which returns at once with an
/// empty [`PermissionOutput`].
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[ts(export, export_to = "decision/")]
pub struct PermissionParams {
    pub id: String,
    #[ts(type = "Record<string, unknown>")]
    pub payload: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[ts(export, export_to = "decision/")]
pub struct PermissionOutput {
    /// What `rup permission` prints on stdout; empty for the `AskUserQuestion` case (H2).
    pub output: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "decision/")]
pub enum Answer {
    Allow,
    Deny,
}

/// `proof` is checked against the value the Daemon holds (H4); a missing or wrong one is
/// `FORBIDDEN`, so the field must deserialize even when the caller sends none.
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[ts(export, export_to = "decision/")]
pub struct AnswerParams {
    pub id: String,
    pub answer: Answer,
    #[ts(optional)]
    pub proof: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "decision/")]
pub enum Outcome {
    Allow,
    Deny,
    Replaced,
    AgentGone,
    Terminal,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[ts(export, export_to = "decision/")]
pub struct ClearedEvent {
    pub id: String,
    pub outcome: Outcome,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample() -> Decision {
        Decision {
            id: "d1".into(),
            agent: "a1".into(),
            tool: "Bash".into(),
            args: "touch spike_out.txt".into(),
            opened_at: 1_700_000_000_000,
            answerable: true,
        }
    }

    #[test]
    fn h2_decision_round_trips_through_json() {
        let decision = sample();

        let wire = serde_json::to_value(&decision).unwrap();
        let back: Decision = serde_json::from_value(wire).unwrap();

        assert_eq!(back, decision);
    }

    #[test]
    fn h2_decision_wire_shape_has_every_field_h2_names() {
        let wire = serde_json::to_value(sample()).unwrap();

        assert_eq!(
            wire,
            json!({
                "id": "d1",
                "agent": "a1",
                "tool": "Bash",
                "args": "touch spike_out.txt",
                "opened_at": 1_700_000_000_000i64,
                "answerable": true,
            })
        );
    }

    #[test]
    fn h2_permission_params_round_trip_the_payload_untouched() {
        let params = PermissionParams {
            id: "a1".into(),
            payload: json!({"hook_event_name": "PermissionRequest", "tool_name": "Bash"}),
        };

        let wire = serde_json::to_value(&params).unwrap();
        let back: PermissionParams = serde_json::from_value(wire).unwrap();

        assert_eq!(back.payload, params.payload);
    }

    #[test]
    fn h2_permission_output_is_empty_for_ask_user_question() {
        let output = PermissionOutput {
            output: String::new(),
        };

        let wire = serde_json::to_value(&output).unwrap();

        assert_eq!(wire, json!({"output": ""}));
    }

    #[test]
    fn h2_answer_is_the_word_allow_or_deny_on_the_wire() {
        assert_eq!(serde_json::to_value(Answer::Allow).unwrap(), json!("allow"));
        assert_eq!(serde_json::to_value(Answer::Deny).unwrap(), json!("deny"));
    }

    #[test]
    fn h2_answer_params_round_trip_with_the_proof() {
        let params = AnswerParams {
            id: "d1".into(),
            answer: Answer::Deny,
            proof: Some("secret".into()),
        };

        let wire = serde_json::to_value(&params).unwrap();
        let back: AnswerParams = serde_json::from_value(wire).unwrap();

        assert_eq!(back.id, params.id);
        assert_eq!(back.answer, params.answer);
        assert_eq!(back.proof, params.proof);
    }

    /// H3: "an `answer` that is neither word is `INVALID_PARAMS`"; `rpc::params` maps any
    /// deserialize failure of this type to that code, so the enum alone is the whole check.
    #[test]
    fn h2_answer_params_reject_a_word_that_is_neither_allow_nor_deny() {
        let bad = json!({"id": "d1", "answer": "ask", "proof": "secret"});

        assert!(serde_json::from_value::<AnswerParams>(bad).is_err());
    }

    /// H4: a missing `proof` must still deserialize, as `None`, so the Daemon can answer
    /// `FORBIDDEN` instead of `rpc::params` turning it into `INVALID_PARAMS` first.
    #[test]
    fn h4_a_missing_proof_deserializes_as_none() {
        let no_proof = json!({"id": "d1", "answer": "allow"});

        let params: AnswerParams = serde_json::from_value(no_proof).unwrap();

        assert_eq!(params.proof, None);
    }

    #[test]
    fn h2_outcome_agent_gone_is_hyphenated_on_the_wire() {
        assert_eq!(
            serde_json::to_value(Outcome::AgentGone).unwrap(),
            json!("agent-gone")
        );
    }

    #[test]
    fn h2_cleared_event_round_trips_through_json() {
        let event = ClearedEvent {
            id: "d1".into(),
            outcome: Outcome::Replaced,
        };

        let wire = serde_json::to_value(&event).unwrap();
        let back: ClearedEvent = serde_json::from_value(wire).unwrap();

        assert_eq!(back.id, event.id);
        assert_eq!(back.outcome, event.outcome);
    }
}
