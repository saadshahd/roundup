//! The one pure, total transition function of a receiver's Messages (B1 to B9), named and ordered
//! as `proofs/messages/model.bend` names them. No I/O, clock or random source: the Daemon builds
//! the `Event`, reads the `State` back from the store and applies the `Effect`s.

use contracts::message::Delivery;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct State {
    pub queue: Vec<u32>,
    pub held_ask_first: Vec<u32>,
    pub held_takeover: Vec<u32>,
    pub typed: Vec<u32>,
    pub dropped: Vec<u32>,
    pub users: Vec<u32>,
    pub next: u32,
    pub takeover: bool,
    pub idle: bool,
    pub gone: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            queue: vec![],
            held_ask_first: vec![],
            held_takeover: vec![],
            typed: vec![],
            dropped: vec![],
            users: vec![],
            next: 1,
            takeover: false,
            idle: false,
            gone: false,
        }
    }
}

#[derive(Clone, Debug)]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) enum Event {
    Send { from_user: bool, route: Delivery },
    Idle,
    Busy,
    Exit,
    TakeoverBegin,
    TakeoverEnd,
    UserDeliver(u32),
    UserDrop(u32),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Effect {
    Type(u32),
}

#[cfg(test)]
pub(crate) const TRANSITIONS: [&str; 8] = [
    "send",
    "idle",
    "busy",
    "exit",
    "takeover_begin",
    "takeover_end",
    "user_deliver",
    "user_drop",
];

fn flush(mut s: State, effects: &mut Vec<Effect>) -> State {
    if s.idle && !s.queue.is_empty() {
        let x = s.queue.remove(0);
        s.typed.push(x);
        s.idle = false;
        effects.push(Effect::Type(x));
    }
    s
}

pub(crate) fn step(state: &State, event: Event) -> (State, Vec<Effect>) {
    let mut s = state.clone();
    let mut fx = vec![];
    match event {
        Event::Send { from_user, route } => {
            if s.gone {
                return (s, fx);
            }
            let n = s.next;
            s.next += 1;
            match route {
                Delivery::Auto => {
                    if from_user {
                        s.queue.push(n);
                        s.users.push(n);
                    } else if s.takeover {
                        s.held_takeover.push(n);
                    } else {
                        s.queue.push(n);
                    }
                    s = flush(s, &mut fx);
                }
                Delivery::AskFirst => {
                    s.held_ask_first.push(n);
                    if from_user {
                        s.users.push(n);
                    }
                }
                Delivery::Drop => {
                    s.dropped.push(n);
                    if from_user {
                        s.users.push(n);
                    }
                }
            }
        }
        Event::Idle => {
            if !s.gone {
                s.idle = true;
                s = flush(s, &mut fx);
            }
        }
        Event::Busy => s.idle = false,
        Event::Exit => {
            let mut gone = std::mem::take(&mut s.queue);
            gone.append(&mut s.held_takeover);
            s.dropped.append(&mut gone);
            s.idle = false;
            s.gone = true;
        }
        Event::TakeoverBegin => {
            let (keep, held): (Vec<u32>, Vec<u32>) =
                s.queue.iter().partition(|x| s.users.contains(x));
            s.queue = keep;
            s.held_takeover.extend(held);
            s.takeover = true;
        }
        Event::TakeoverEnd => {
            let held = std::mem::take(&mut s.held_takeover);
            s.queue.extend(held);
            s.takeover = false;
            s = flush(s, &mut fx);
        }
        Event::UserDeliver(id) => {
            if !s.gone {
                if let Some(i) = s.held_ask_first.iter().position(|x| *x == id) {
                    s.held_ask_first.remove(i);
                    s.queue.push(id);
                    s = flush(s, &mut fx);
                } else if let Some(i) = s.held_takeover.iter().position(|x| *x == id) {
                    s.held_takeover.remove(i);
                    s.queue.push(id);
                    s = flush(s, &mut fx);
                }
            }
        }
        Event::UserDrop(id) => {
            if let Some(i) = s.held_ask_first.iter().position(|x| *x == id) {
                s.held_ask_first.remove(i);
                s.dropped.push(id);
            } else if let Some(i) = s.held_takeover.iter().position(|x| *x == id) {
                s.held_takeover.remove(i);
                s.dropped.push(id);
            }
        }
    }
    (s, fx)
}
