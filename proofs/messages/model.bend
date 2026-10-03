import Base

# Pure model of `step` in crates/messages for ONE receiver (docs/messages.md; B1 to B9).
# A Message is its id. Its status is the list its id sits in. Escalation (B13 to B17) and the
# store (B8's restart) are not modelled yet: M8 and M9 need them.

type Route is Data:
  Auto{}
  AskFirst{}
  RouteDrop{}

type Ev is Data:
  Send{from_user: Bool, route: Route}
  Idle{}
  Busy{}
  Exit{}
  TakeoverBegin{}
  TakeoverEnd{}
  UserDeliver{id: Nat}
  UserDrop{id: Nat}

# queue: `pending` ids, in the order they became deliverable (B7).
# held_af: `held` for ask-first. held_tk: `held` for takeover.
# typed: `delivered` ids, in the order they were written. dropped: `dropped` ids.
# users: ids the user sent. next: the next id. takeover, idle, gone: the receiver's state.
# tk_typed is a proof-only record of the ids typed while a Takeover held; released holds the ids the user delivered
# by hand (B6: the user's call wins even during a Takeover). The Rust derives both by replay.
type World is Data:
  W{queue: +List<Nat>, held_af: +List<Nat>, held_tk: +List<Nat>, typed: +List<Nat>, dropped: +List<Nat>, users: +List<Nat>, tk_typed: +List<Nat>, released: +List<Nat>, next: Nat, takeover: Bool, idle: Bool, gone: Bool}

def init() -> World:
  W{[], [], [], [], [], [], [], [], 1n, False{}, False{}, False{}}

def mem(+x: Nat, xs: +List<Nat>) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case Con{+y, rest}:
      Bool.or(Nat.is_eq(x, y), mem(x, rest))

def remove_step(b: Bool, +y: Nat, tail: +List<Nat>) -> +List<Nat>:
  match b:
    case True{}:
      tail
    case False{}:
      y <> tail

def remove(+x: Nat, xs: +List<Nat>) -> +List<Nat>:
  match xs:
    case Nil{}:
      []
    case Con{+y, rest}:
      remove_step(Nat.is_eq(x, y), y, remove(x, rest))

def snoc(xs: +List<Nat>, +x: Nat) -> +List<Nat>:
  List.append(&2, Nat, xs, [x])

def record(tk: Bool, +x: Nat, tt: +List<Nat>) -> +List<Nat>:
  match tk:
    case True{}:
      snoc(tt, x)
    case False{}:
      tt

# B2, B7: an idle receiver outside a Takeover is typed the head of the queue, once, and is then busy.
def flush_go(q: +List<Nat>, s: World) -> World:
  match q:
    case Nil{}:
      s
    case Con{+x, rest}:
      match s:
        case W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          W{rest, haf, htk, snoc(t, x), d, u, record(tk, x, tt), rl, n, tk, False{}, g}

def flush_b(i: Bool, s: World) -> World:
  match i:
    case False{}:
      s
    case True{}:
      match s:
        case W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          flush_go(q, W{[], haf, htk, t, d, u, tt, rl, n, tk, i, g})

def flush(s: World) -> World:
  match s:
    case W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
      flush_b(i, W{q, haf, htk, t, d, u, tt, rl, n, tk, i, g})

def keep_step(b: Bool, +x: Nat, tail: +List<Nat>) -> +List<Nat>:
  match b:
    case True{}:
      x <> tail
    case False{}:
      tail

def drop_step(b: Bool, +x: Nat, tail: +List<Nat>) -> +List<Nat>:
  match b:
    case True{}:
      tail
    case False{}:
      x <> tail

def keep_users(q: +List<Nat>, +u: +List<Nat>) -> +List<Nat>:
  match q:
    case Nil{}:
      []
    case Con{+x, rest}:
      keep_step(mem(x, u), x, keep_users(rest, u))


def drop_users(q: +List<Nat>, +u: +List<Nat>) -> +List<Nat>:
  match q:
    case Nil{}:
      []
    case Con{+x, rest}:
      drop_step(mem(x, u), x, drop_users(rest, u))

def owned(fu: Bool, +n: Nat, u: +List<Nat>) -> +List<Nat>:
  match fu:
    case True{}:
      snoc(u, n)
    case False{}:
      u

def send_route(fu: Bool, tk: Bool, s: World) -> World:
  match fu:
    case True{}:
      match s:
        case W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          flush(W{snoc(q, n), haf, htk, t, d, snoc(u, n), tt, rl, 1n+n, tk, i, g})
    case False{}:
      match tk:
        case True{}:
          match s:
            case W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
              W{q, haf, snoc(htk, n), t, d, u, tt, rl, 1n+n, tk, i, g}
        case False{}:
          match s:
            case W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
              flush(W{snoc(q, n), haf, htk, t, d, u, tt, rl, 1n+n, tk, i, g})

def send_auto(fu: Bool, s: World) -> World:
  match s:
    case W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
      send_route(fu, tk, W{q, haf, htk, t, d, u, tt, rl, n, tk, i, g})

def send_live(r: Route, fu: Bool, s: World) -> World:
  match r:
    case Auto{}:
      send_auto(fu, s)
    case AskFirst{}:
      match s:
        case W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          W{q, snoc(haf, n), htk, t, d, owned(fu, n, u), tt, rl, 1n+n, tk, i, g}
    case RouteDrop{}:
      match s:
        case W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          W{q, haf, htk, t, snoc(d, n), owned(fu, n, u), tt, rl, 1n+n, tk, i, g}

# B9: a later send to a gone receiver is CONFLICT and changes nothing.
def send(r: Route, fu: Bool, g: Bool, s: World) -> World:
  match g:
    case True{}:
      s
    case False{}:
      send_live(r, fu, s)

# B6: a Takeover begins; pending Messages from others wait for it to end, the user's stay pending.
def takeover_begin(s: World) -> World:
  match s:
    case W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
      W{keep_users(q, u), haf, List.append(&2, Nat, htk, drop_users(q, u)), t, d, u, tt, rl, n, True{}, i, g}

# B6: the Takeover ends; what it held becomes pending, behind what already waits.
def takeover_end(s: World) -> World:
  match s:
    case W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
      flush(W{List.append(&2, Nat, q, htk), haf, [], t, d, u, tt, rl, n, False{}, i, g})

# B9: a receiver that is gone has no pending Message; what a Takeover held is dropped with it.
def exit_(s: World) -> World:
  match s:
    case W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
      W{[], haf, [], t, List.append(&2, Nat, d, List.append(&2, Nat, q, htk)), u, tt, rl, n, tk, False{}, True{}}

def deliver_if(b: Bool, +id: Nat, s: World) -> World:
  match b:
    case False{}:
      s
    case True{}:
      match s:
        case W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          flush(W{snoc(q, id), remove(id, haf), htk, t, d, u, tt, snoc(rl, id), n, tk, i, g})

def drop_if(b: Bool, +id: Nat, s: World) -> World:
  match b:
    case False{}:
      s
    case True{}:
      match s:
        case W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          W{q, remove(id, haf), htk, t, snoc(d, id), u, tt, rl, n, tk, i, g}

def deliver_tk(b: Bool, +id: Nat, s: World) -> World:
  match b:
    case False{}:
      s
    case True{}:
      match s:
        case W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          flush(W{snoc(q, id), haf, remove(id, htk), t, d, u, tt, snoc(rl, id), n, tk, i, g})

def deliver_pick(a: Bool, b: Bool, +id: Nat, s: World) -> World:
  match a:
    case True{}:
      deliver_if(True{}, id, s)
    case False{}:
      deliver_tk(b, id, s)

# B3, B6: the user can deliver a Message held for ask-first or for a Takeover, and not once the receiver is gone (B9).
def user_deliver(+id: Nat, s: World) -> World:
  match s:
    case W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
      deliver_pick(Bool.and(mem(id, haf), Bool.not(g)), Bool.and(mem(id, htk), Bool.not(g)), id, W{q, haf, htk, t, d, u, tt, rl, n, tk, i, g})

def drop_tk(b: Bool, +id: Nat, s: World) -> World:
  match b:
    case False{}:
      s
    case True{}:
      match s:
        case W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          W{q, haf, remove(id, htk), t, snoc(d, id), u, tt, rl, n, tk, i, g}

def drop_pick(a: Bool, b: Bool, +id: Nat, s: World) -> World:
  match a:
    case True{}:
      drop_if(True{}, id, s)
    case False{}:
      drop_tk(b, id, s)

# B3, B6: the user can drop a Message held for ask-first or for a Takeover; the Takeover goes on.
def user_drop(+id: Nat, s: World) -> World:
  match s:
    case W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
      drop_pick(mem(id, haf), mem(id, htk), id, W{q, haf, htk, t, d, u, tt, rl, n, tk, i, g})

def set_idle(b: Bool, s: World) -> World:
  match s:
    case W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
      W{q, haf, htk, t, d, u, tt, rl, n, tk, b, g}

def idle_ev(s: World) -> World:
  match s:
    case W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
      match g:
        case True{}:
          W{q, haf, htk, t, d, u, tt, rl, n, tk, i, g}
        case False{}:
          flush(W{q, haf, htk, t, d, u, tt, rl, n, tk, True{}, g})

def busy_ev(s: World) -> World:
  set_idle(False{}, s)

def send_ev(fu: Bool, r: Route, s: World) -> World:
  match s:
    case W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
      send(r, fu, g, W{q, haf, htk, t, d, u, tt, rl, n, tk, i, g})

# The whole transition table: one pure, total function of the world and one event.
def step(e: Ev, s: World) -> World:
  match e:
    case Send{fu, r}:
      send_ev(fu, r, s)
    case Idle{}:
      idle_ev(s)
    case Busy{}:
      busy_ev(s)
    case Exit{}:
      exit_(s)
    case TakeoverBegin{}:
      takeover_begin(s)
    case TakeoverEnd{}:
      takeover_end(s)
    case UserDeliver{id}:
      user_deliver(id, s)
    case UserDrop{id}:
      user_drop(id, s)

# A schedule is a list of events; every law quantifies over every such list.
def run(evs: +List<Ev>, s: World) -> World:
  match evs:
    case Nil{}:
      s
    case Con{e, rest}:
      run(rest, step(e, s))
