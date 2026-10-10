//! O2: the Steer an Agent's order becomes at its first `SessionStart`.

use contracts::agent::Order;

/// For a Work order, the ask, then `Must not:` and one line per limit (left out when there are
/// none). For a Clarification order, the question and what to do about it.
pub(crate) fn steer(order: &Order) -> String {
    match order {
        Order::Work { ask, limits } if limits.is_empty() => ask.clone(),
        Order::Work { ask, limits } => {
            let lines: String = limits.iter().map(|limit| format!("\n- {limit}")).collect();
            format!("{ask}\n\nMust not:{lines}")
        }
        Order::Clarification { question } => format!(
            "{question}\n\nFind the intent and elicit what is missing, asking whom \
             `agent_context` names under `ask`. Then record a Work order with `agent_set_order`."
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn o2_a_work_order_steers_the_ask_then_one_line_per_limit() {
        let order = Order::Work {
            ask: "fix the build".into(),
            limits: vec!["touch CI".into(), "push".into()],
        };
        assert_eq!(
            steer(&order),
            "fix the build\n\nMust not:\n- touch CI\n- push"
        );
        assert_eq!(steer(&Order::work("fix the build")), "fix the build");
    }
}
