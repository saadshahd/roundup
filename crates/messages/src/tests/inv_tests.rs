//! V4: `inv_` replays tie `proofs/messages/model.bend` to the Rust. Written by an architect before
//! the Builder; the Builder makes these pass and never edits this file (one `mod` line in `lib.rs`
//! is the Builder's).
//!
//! Pinned seam, `crates/messages/src/step.rs` (new), the one pure, total function the Daemon's
//! transitions run through, named and ordered as `model.bend` names and orders them:
//! ```text
//! pub(crate) struct State { queue, held_ask_first, held_takeover, typed, dropped, users: Vec<u32>,
//!                           next: u32, takeover: bool, idle: bool, gone: bool }   // Clone, Debug, PartialEq
//! impl Default for State   // model.bend's `init`: empty lists, `next` 1, all flags false
//! #[derive(Clone, Debug)]
//! pub(crate) enum Event { Send { from_user: bool, route: Delivery }, Idle, Busy, Exit,
//!                         TakeoverBegin, TakeoverEnd, UserDeliver(u32), UserDrop(u32) }
//! pub(crate) enum Effect { Type(u32) }             // Clone, Debug, PartialEq
//! pub(crate) fn step(state: &State, event: Event) -> (State, Vec<Effect>)
//! pub(crate) const TRANSITIONS: [&str; 8]          // the snake_case names of the Event variants
//! ```
//! `step` has no I/O, clock or random source. Slice 2's Daemon applies its effects: `Type(id)`
//! is the one prompt handed to `Deliver`.

use contracts::message::Delivery;

use crate::step::{Effect, Event, State, TRANSITIONS, step};

/// xorshift64: a fixed seed gives the same schedules on every machine.
struct Rng(u64);

impl Rng {
    fn below(&mut self, n: u64) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0 % n
    }
}

const MESSAGES: u64 = 3;
const EVENTS: u64 = 6;

/// At most 6 events and 3 Messages (V4): a schedule never sends a fourth.
fn schedule(rng: &mut Rng) -> Vec<Event> {
    let mut sends = 0;
    let mut events = vec![];
    for _ in 0..rng.below(EVENTS + 1) {
        let event = match rng.below(8) {
            0 if sends < MESSAGES => {
                sends += 1;
                Event::Send {
                    from_user: rng.below(2) == 0,
                    route: [Delivery::Auto, Delivery::AskFirst, Delivery::Drop]
                        [rng.below(3) as usize],
                }
            }
            1 => Event::Idle,
            2 => Event::Busy,
            3 => Event::Exit,
            4 => Event::TakeoverBegin,
            5 => Event::TakeoverEnd,
            6 => Event::UserDeliver(1 + rng.below(MESSAGES) as u32),
            7 => Event::UserDrop(1 + rng.below(MESSAGES) as u32),
            _ => Event::Idle,
        };
        events.push(event);
    }
    events
}

fn schedules() -> Vec<Vec<Event>> {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    (0..4000).map(|_| schedule(&mut rng)).collect()
}

/// One transition: the state before, the event, the state after and what it typed.
struct Move {
    before: State,
    event: Event,
    after: State,
    effects: Vec<Effect>,
}

fn replay(events: &[Event]) -> Vec<Move> {
    let mut state = State::default();
    events
        .iter()
        .map(|event| {
            let (after, effects) = step(&state, event.clone());
            let before = std::mem::replace(&mut state, after.clone());
            Move {
                before,
                event: event.clone(),
                after,
                effects,
            }
        })
        .collect()
}

fn last(moves: &[Move]) -> State {
    moves
        .last()
        .map_or_else(State::default, |m| m.after.clone())
}

fn is_prefix(short: &[u32], long: &[u32]) -> bool {
    long.starts_with(short)
}

fn disjoint(a: &[u32], b: &[u32]) -> bool {
    a.iter().all(|x| !b.contains(x))
}

fn all_in(a: &[u32], b: &[u32]) -> bool {
    a.iter().all(|x| b.contains(x))
}

fn user_call(event: &Event) -> bool {
    matches!(event, Event::UserDeliver(_) | Event::UserDrop(_))
}

#[test]
fn inv_m1_a_message_is_typed_at_most_once() {
    for events in schedules() {
        let typed = last(&replay(&events)).typed;
        let mut unique = typed.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), typed.len(), "{events:?}");
    }
}

#[test]
fn inv_m2_a_message_held_for_ask_first_leaves_held_only_by_the_users_call() {
    for events in schedules() {
        for m in replay(&events) {
            if !user_call(&m.event) {
                assert!(
                    all_in(&m.before.held_ask_first, &m.after.held_ask_first),
                    "{events:?} at {:?}",
                    m.event
                );
            }
        }
    }
}

#[test]
fn inv_m3_nothing_typed_during_a_takeover_came_from_anyone_but_the_user_unless_delivered_by_hand() {
    for events in schedules() {
        let (mut typed_during, mut released) = (vec![], vec![]);
        for m in replay(&events) {
            if let Event::UserDeliver(id) = m.event
                && !m.before.gone
                && (m.before.held_ask_first.contains(&id) || m.before.held_takeover.contains(&id))
            {
                released.push(id);
            }
            for effect in &m.effects {
                let Effect::Type(id) = effect;
                if m.after.takeover {
                    typed_during.push(*id);
                }
            }
        }
        let users = last(&replay(&events)).users;
        let allowed: Vec<u32> = users.into_iter().chain(released).collect();
        assert!(all_in(&typed_during, &allowed), "{events:?}");
    }
}

#[test]
fn inv_m4_the_typed_list_only_grows_at_its_end() {
    for events in schedules() {
        for m in replay(&events) {
            assert!(
                is_prefix(&m.before.typed, &m.after.typed),
                "{events:?} at {:?}",
                m.event
            );
        }
    }
}

#[test]
fn inv_m5_delivered_and_dropped_are_disjoint_and_a_typed_id_waits_nowhere() {
    for events in schedules() {
        let s = last(&replay(&events));
        assert!(disjoint(&s.typed, &s.dropped), "{events:?}");
        assert!(disjoint(&s.typed, &s.queue), "{events:?}");
        assert!(disjoint(&s.typed, &s.held_ask_first), "{events:?}");
        assert!(disjoint(&s.typed, &s.held_takeover), "{events:?}");
    }
}

#[test]
fn inv_m6_every_id_handed_out_is_in_exactly_one_list() {
    for events in schedules() {
        let s = last(&replay(&events));
        let held = s.queue.len() + s.held_ask_first.len() + s.held_takeover.len();
        assert_eq!(
            1 + held + s.typed.len() + s.dropped.len(),
            s.next as usize,
            "{events:?}"
        );
    }
}

#[test]
fn inv_m7_once_the_receiver_is_gone_nothing_waits_and_a_send_changes_nothing() {
    for events in schedules() {
        let s = last(&replay(&events));
        if s.gone {
            assert!(
                s.queue.is_empty() && s.held_takeover.is_empty(),
                "{events:?}"
            );
            let (after, effects) = step(
                &s,
                Event::Send {
                    from_user: false,
                    route: Delivery::Auto,
                },
            );
            assert_eq!((after, effects), (s, vec![]), "{events:?}");
        }
    }
}

#[test]
fn inv_m10_a_live_receiver_that_turns_idle_again_types_every_pending_message() {
    for events in schedules() {
        let mut s = last(&replay(&events));
        if s.gone {
            continue;
        }
        for _ in 0..s.queue.len() {
            s = step(&s, Event::Busy).0;
            s = step(&s, Event::Idle).0;
        }
        assert!(s.queue.is_empty(), "{events:?}");
    }
}

#[test]
fn inv_the_transition_names_in_step_and_in_model_bend_are_the_same_set() {
    let model = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../proofs/messages/model.bend"
    ))
    .unwrap();
    let ev = model
        .split("type Ev is Data:")
        .nth(1)
        .unwrap()
        .split("\n\n")
        .next()
        .unwrap();
    let mut in_model: Vec<String> = ev
        .lines()
        .filter_map(|line| line.trim().split('{').next())
        .filter(|name| !name.is_empty())
        .map(|name| {
            name.chars()
                .enumerate()
                .flat_map(|(i, c)| {
                    let lower = c.to_ascii_lowercase();
                    if i > 0 && c.is_ascii_uppercase() {
                        vec!['_', lower]
                    } else {
                        vec![lower]
                    }
                })
                .collect()
        })
        .collect();
    in_model.sort();
    let mut in_step: Vec<String> = TRANSITIONS.iter().map(|name| (*name).to_owned()).collect();
    in_step.sort();
    assert_eq!(in_step, in_model);
}

/// Hand-checked against `model.bend`, so a Builder that drifts from the model on a guard or an
/// order fails here and not only in a random schedule.
#[test]
fn inv_golden_traces_follow_model_bend() {
    let send = |from_user, route| Event::Send { from_user, route };
    let run = |events: Vec<Event>| {
        events
            .into_iter()
            .fold((State::default(), vec![]), |(s, mut typed), e| {
                let (next, effects) = step(&s, e);
                typed.extend(effects);
                (next, typed)
            })
    };

    let (s, typed) = run(vec![Event::Idle, send(false, Delivery::Auto)]);
    assert_eq!(
        (s.typed, s.idle, typed),
        (vec![1], false, vec![Effect::Type(1)])
    );

    let (s, typed) = run(vec![
        Event::TakeoverBegin,
        send(false, Delivery::Auto),
        send(true, Delivery::Auto),
        Event::TakeoverEnd,
        Event::Idle,
    ]);
    assert_eq!(
        (s.held_takeover, s.typed, typed),
        (vec![], vec![2], vec![Effect::Type(2)])
    );

    let (s, _) = run(vec![
        Event::TakeoverBegin,
        send(false, Delivery::Auto),
        send(false, Delivery::Auto),
        Event::TakeoverEnd,
        Event::Idle,
        Event::Busy,
        Event::Idle,
    ]);
    assert_eq!(
        s.typed,
        vec![1, 2],
        "a Takeover's holds are released in id order"
    );

    let (s, _) = run(vec![
        send(false, Delivery::AskFirst),
        send(false, Delivery::Drop),
        Event::UserDeliver(1),
        Event::UserDrop(2),
    ]);
    assert_eq!(
        (s.queue, s.held_ask_first, s.dropped),
        (vec![1], vec![], vec![2])
    );

    let (s, typed) = run(vec![
        send(false, Delivery::Auto),
        Event::TakeoverBegin,
        Event::Exit,
        Event::Idle,
    ]);
    assert_eq!((s.dropped, s.gone, typed), (vec![1], true, vec![]));
}

/// The laws above hold of `step`; this holds the Daemon to it: a transition coded beside `step`
/// would leave the laws proving a model nobody runs.
#[test]
fn inv_the_daemons_transitions_run_through_step() {
    let lib = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/lib.rs")).unwrap();
    for event in [
        "Event::Send",
        "Event::Idle",
        "Event::TakeoverBegin",
        "Event::TakeoverEnd",
        "Event::Exit",
        "Event::UserDeliver",
        "Event::UserDrop",
    ] {
        assert!(lib.contains(event), "lib.rs never builds {event}");
    }
}
