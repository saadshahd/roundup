import Base
import ./model.bend as M

# Human-written predicates the laws use. Builders never edit this file or LAWS.bend.

def implies(a: Bool, b: Bool) -> Bool:
  match a:
    case True{}:
      b
    case False{}:
      True{}

def nodup(xs: +List<Nat>) -> Bool:
  match xs:
    case Nil{}:
      True{}
    case Con{+x, +rest}:
      Bool.and(Bool.not(M.mem(x, rest)), nodup(rest))

def all_in(xs: +List<Nat>, +ys: +List<Nat>) -> Bool:
  match xs:
    case Nil{}:
      True{}
    case Con{+x, rest}:
      Bool.and(M.mem(x, ys), all_in(rest, ys))

def disjoint(xs: +List<Nat>, +ys: +List<Nat>) -> Bool:
  match xs:
    case Nil{}:
      True{}
    case Con{+x, rest}:
      Bool.and(Bool.not(M.mem(x, ys)), disjoint(rest, ys))

def is_prefix(xs: +List<Nat>, +ys: +List<Nat>) -> Bool:
  match xs:
    case Nil{}:
      True{}
    case Con{+x, rest}:
      match ys:
        case Nil{}:
          False{}
        case Con{+y, ys2}:
          Bool.and(Nat.is_eq(x, y), is_prefix(rest, ys2))

def len(xs: +List<Nat>) -> Nat:
  List.length(&2, Nat, xs)

def is_nil(xs: +List<Nat>) -> Bool:
  match xs:
    case Nil{}:
      True{}
    case Con{y, rest}:
      False{}

def is_user_call(e: M.Ev) -> Bool:
  match e:
    case M.Send{fu, r}:
      False{}
    case M.Idle{}:
      False{}
    case M.Busy{}:
      False{}
    case M.Exit{}:
      False{}
    case M.TakeoverBegin{}:
      False{}
    case M.TakeoverEnd{}:
      False{}
    case M.UserDeliver{id}:
      True{}
    case M.UserDrop{id}:
      True{}

# The fairness schedule of M10: the receiver turns busy and idle again, n times.
def cycle(n: Nat) -> +List<M.Ev>:
  match n:
    case 0n:
      []
    case 1n+p:
      M.Busy{} <> M.Idle{} <> cycle(p)

# Field readers, so a law reads the world through one name each.
def queue(s: M.World) -> +List<Nat>:
  match s:
    case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
      q

def held_af(s: M.World) -> +List<Nat>:
  match s:
    case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
      haf

def held_tk(s: M.World) -> +List<Nat>:
  match s:
    case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
      htk

def typed(s: M.World) -> +List<Nat>:
  match s:
    case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
      t

def dropped(s: M.World) -> +List<Nat>:
  match s:
    case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
      d

def users(s: M.World) -> +List<Nat>:
  match s:
    case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
      u

def tk_typed(s: M.World) -> +List<Nat>:
  match s:
    case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
      tt

def released(s: M.World) -> +List<Nat>:
  match s:
    case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
      rl

def next_id(s: M.World) -> Nat:
  match s:
    case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
      n

def gone(s: M.World) -> Bool:
  match s:
    case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
      g

def world(evs: +List<M.Ev>) -> M.World:
  M.run(evs, M.init())
