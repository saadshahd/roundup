import Base
import ./LAWS.bend as Laws
import ./model.bend as M
import ./spec.bend as S

# ---- lib

# T(b): the claim that b is True.
def T(b: Bool) -> Data:
  {b == True{} : Bool}

def disc(b: Bool) -> Type:
  match b:
    case False{}:
      Unit
    case True{}:
      Empty

def ft(e: {False{} == True{} : Bool}) -> Empty:
  %e : disc(_)
  Unit{}

def no(-A: Type, h: T(False{})) -> A:
  Empty.absurd(A, ft(h))

def and_l(a: Bool, b: Bool, +h: T(Bool.and(a, b))) -> T(a):
  match a:
    case False{}:
      no(T(False{}), h)
    case True{}:
      {==}

def and_r(a: Bool, b: Bool, +h: T(Bool.and(a, b))) -> T(b):
  match a:
    case False{}:
      no(T(b), h)
    case True{}:
      h

def and_i(a: Bool, b: Bool, +ha: T(a), +hb: T(b)) -> T(Bool.and(a, b)):
  %Equal.sym(Bool, a, True{}, ha) : {Bool.and(_, b) == True{} : Bool}
  hb

def or_l(a: Bool, b: Bool, +h: T(a)) -> T(Bool.or(a, b)):
  %Equal.sym(Bool, a, True{}, h) : {Bool.or(_, b) == True{} : Bool}
  {==}

def or_r(a: Bool, b: Bool, +h: T(b)) -> T(Bool.or(a, b)):
  match a:
    case False{}:
      h
    case True{}:
      {==}

def or_mono(e: Bool, m: Bool, m2: Bool, f: T(m) -> T(m2), +h: T(Bool.or(e, m))) -> T(Bool.or(e, m2)):
  match e:
    case False{}:
      f(h)
    case True{}:
      {==}

def or_elim(-c: Bool, e: Bool, m: Bool, f1: T(e) -> T(c), f2: T(m) -> T(c), +h: T(Bool.or(e, m))) -> T(c):
  match e:
    case False{}:
      f2(h)
    case True{}:
      f1({==})

def eq_refl(+x: Nat) -> T(Nat.is_eq(x, x)):
  match x:
    case 0n:
      {==}
    case 1n+p:
      eq_refl(p)

def eq_elim(+x: Nat, +y: Nat, +h: T(Nat.is_eq(x, y))) -> {x == y : Nat}:
  match x y:
    case 0n 0n:
      {==}
    case 0n 1n+yp:
      no({0n == 1n+yp : Nat}, h)
    case 1n+xp 0n:
      no({1n+xp == 0n : Nat}, h)
    case 1n+xp 1n+yp:
      Equal.cong(Nat, Nat, a => 1n+a, xp, yp, eq_elim(xp, yp, h))

def mem_snoc_keep(+x: Nat, +l: +List<Nat>, +y: Nat, +h: T(M.mem(x, l))) -> T(M.mem(x, M.snoc(l, y))):
  match l:
    case Nil{}:
      no(T(M.mem(x, M.snoc([], y))), h)
    case Con{+z, +rest}:
      or_mono(Nat.is_eq(x, z), M.mem(x, rest), M.mem(x, M.snoc(rest, y)), hh => mem_snoc_keep(x, rest, y, hh), h)

def mem_snoc_new(+l: +List<Nat>, +y: Nat) -> T(M.mem(y, M.snoc(l, y))):
  match l:
    case Nil{}:
      or_l(Nat.is_eq(y, y), False{}, eq_refl(y))
    case Con{+z, +rest}:
      or_r(Nat.is_eq(y, z), M.mem(y, M.snoc(rest, y)), mem_snoc_new(rest, y))

def mem_cons_keep(+x: Nat, +y: Nat, +ys: +List<Nat>, +h: T(M.mem(x, ys))) -> T(M.mem(x, y <> ys)):
  or_r(Nat.is_eq(x, y), M.mem(x, ys), h)

def mem_app_l(+x: Nat, +a: +List<Nat>, +b: +List<Nat>, +h: T(M.mem(x, a))) -> T(M.mem(x, List.append(&2, Nat, a, b))):
  match a:
    case Nil{}:
      no(T(M.mem(x, List.append(&2, Nat, [], b))), h)
    case Con{+z, +rest}:
      or_mono(Nat.is_eq(x, z), M.mem(x, rest), M.mem(x, List.append(&2, Nat, rest, b)), hh => mem_app_l(x, rest, b, hh), h)

def mem_app_r(+x: Nat, +a: +List<Nat>, +b: +List<Nat>, +h: T(M.mem(x, b))) -> T(M.mem(x, List.append(&2, Nat, a, b))):
  match a:
    case Nil{}:
      h
    case Con{+z, +rest}:
      or_r(Nat.is_eq(x, z), M.mem(x, List.append(&2, Nat, rest, b)), mem_app_r(x, rest, b, h))

def mem_eq(+x: Nat, +z: Nat, +zs: +List<Nat>, +e: {x == z : Nat}, +h: T(M.mem(z, zs))) -> T(M.mem(x, zs)):
  %Equal.sym(Nat, x, z, e) : T(M.mem(_, zs))
  h

def mem_all_in(+x: Nat, +ys: +List<Nat>, +zs: +List<Nat>, +hm: T(M.mem(x, ys)), +ha: T(S.all_in(ys, zs))) -> T(M.mem(x, zs)):
  match ys:
    case Nil{}:
      no(T(M.mem(x, zs)), hm)
    case Con{+z, +rest}:
      or_elim(M.mem(x, zs), Nat.is_eq(x, z), M.mem(x, rest), he => mem_eq(x, z, zs, eq_elim(x, z, he), and_l(M.mem(z, zs), S.all_in(rest, zs), ha)), hr => mem_all_in(x, rest, zs, hr, and_r(M.mem(z, zs), S.all_in(rest, zs), ha)), hm)

def all_in_trans(+xs: +List<Nat>, +ys: +List<Nat>, +zs: +List<Nat>, +h1: T(S.all_in(xs, ys)), +h2: T(S.all_in(ys, zs))) -> T(S.all_in(xs, zs)):
  match xs:
    case Nil{}:
      {==}
    case Con{+z, +rest}:
      and_i(M.mem(z, zs), S.all_in(rest, zs), mem_all_in(z, ys, zs, and_l(M.mem(z, ys), S.all_in(rest, ys), h1), h2), all_in_trans(rest, ys, zs, and_r(M.mem(z, ys), S.all_in(rest, ys), h1), h2))

def all_in_cons_r(+xs: +List<Nat>, +y: Nat, +ys: +List<Nat>, +h: T(S.all_in(xs, ys))) -> T(S.all_in(xs, y <> ys)):
  match xs:
    case Nil{}:
      {==}
    case Con{+z, +rest}:
      and_i(M.mem(z, y <> ys), S.all_in(rest, y <> ys), mem_cons_keep(z, y, ys, and_l(M.mem(z, ys), S.all_in(rest, ys), h)), all_in_cons_r(rest, y, ys, and_r(M.mem(z, ys), S.all_in(rest, ys), h)))

def all_in_snoc_r(+xs: +List<Nat>, +ys: +List<Nat>, +y: Nat, +h: T(S.all_in(xs, ys))) -> T(S.all_in(xs, M.snoc(ys, y))):
  match xs:
    case Nil{}:
      {==}
    case Con{+z, +rest}:
      and_i(M.mem(z, M.snoc(ys, y)), S.all_in(rest, M.snoc(ys, y)), mem_snoc_keep(z, ys, y, and_l(M.mem(z, ys), S.all_in(rest, ys), h)), all_in_snoc_r(rest, ys, y, and_r(M.mem(z, ys), S.all_in(rest, ys), h)))

def all_in_app_l(+xs: +List<Nat>, +a: +List<Nat>, +b: +List<Nat>, +h: T(S.all_in(xs, a))) -> T(S.all_in(xs, List.append(&2, Nat, a, b))):
  match xs:
    case Nil{}:
      {==}
    case Con{+z, +rest}:
      and_i(M.mem(z, List.append(&2, Nat, a, b)), S.all_in(rest, List.append(&2, Nat, a, b)), mem_app_l(z, a, b, and_l(M.mem(z, a), S.all_in(rest, a), h)), all_in_app_l(rest, a, b, and_r(M.mem(z, a), S.all_in(rest, a), h)))

def all_in_app_r(+xs: +List<Nat>, +a: +List<Nat>, +b: +List<Nat>, +h: T(S.all_in(xs, b))) -> T(S.all_in(xs, List.append(&2, Nat, a, b))):
  match xs:
    case Nil{}:
      {==}
    case Con{+z, +rest}:
      and_i(M.mem(z, List.append(&2, Nat, a, b)), S.all_in(rest, List.append(&2, Nat, a, b)), mem_app_r(z, a, b, and_l(M.mem(z, b), S.all_in(rest, b), h)), all_in_app_r(rest, a, b, and_r(M.mem(z, b), S.all_in(rest, b), h)))

def all_in_refl(+l: +List<Nat>) -> T(S.all_in(l, l)):
  match l:
    case Nil{}:
      {==}
    case Con{+z, +rest}:
      and_i(M.mem(z, z <> rest), S.all_in(rest, z <> rest), or_l(Nat.is_eq(z, z), M.mem(z, rest), eq_refl(z)), all_in_cons_r(rest, z, rest, all_in_refl(rest)))

def all_in_snoc_i(+xs: +List<Nat>, +ys: +List<Nat>, +x: Nat, +h: T(S.all_in(xs, ys)), +hx: T(M.mem(x, ys))) -> T(S.all_in(M.snoc(xs, x), ys)):
  match xs:
    case Nil{}:
      and_i(M.mem(x, ys), True{}, hx, {==})
    case Con{+z, +rest}:
      and_i(M.mem(z, ys), S.all_in(M.snoc(rest, x), ys), and_l(M.mem(z, ys), S.all_in(rest, ys), h), all_in_snoc_i(rest, ys, x, and_r(M.mem(z, ys), S.all_in(rest, ys), h), hx))

def all_in_app_i(+a: +List<Nat>, +b: +List<Nat>, +c: +List<Nat>, +ha: T(S.all_in(a, c)), +hb: T(S.all_in(b, c))) -> T(S.all_in(List.append(&2, Nat, a, b), c)):
  match a:
    case Nil{}:
      hb
    case Con{+z, +rest}:
      and_i(M.mem(z, c), S.all_in(List.append(&2, Nat, rest, b), c), and_l(M.mem(z, c), S.all_in(rest, c), ha), all_in_app_i(rest, b, c, and_r(M.mem(z, c), S.all_in(rest, c), ha), hb))

def all_in_snoc_self(+l: +List<Nat>, +x: Nat) -> T(S.all_in(l, M.snoc(l, x))):
  all_in_snoc_r(l, l, x, all_in_refl(l))

def all_in_app_self_l(+a: +List<Nat>, +b: +List<Nat>) -> T(S.all_in(a, List.append(&2, Nat, a, b))):
  all_in_app_l(a, a, b, all_in_refl(a))

def all_in_app_self_r(+a: +List<Nat>, +b: +List<Nat>) -> T(S.all_in(b, List.append(&2, Nat, a, b))):
  all_in_app_r(b, a, b, all_in_refl(b))

def mem_in_hd(+x: Nat, +rest: +List<Nat>, +ys: +List<Nat>, +h: T(S.all_in(x <> rest, ys))) -> T(M.mem(x, ys)):
  and_l(M.mem(x, ys), S.all_in(rest, ys), h)

def all_in_tl(+x: Nat, +rest: +List<Nat>, +ys: +List<Nat>, +h: T(S.all_in(x <> rest, ys))) -> T(S.all_in(rest, ys)):
  and_r(M.mem(x, ys), S.all_in(rest, ys), h)

def imp_t(b: Bool) -> T(S.implies(b, True{})):
  match b:
    case False{}:
      {==}
    case True{}:
      {==}

# ---- M3
def tk_of(+s: M.World) -> Bool:
  match s:
    case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
      tk

def UR(+u: +List<Nat>, +rl: +List<Nat>) -> +List<Nat>:
  List.append(&2, Nat, u, rl)

def I3(+q: +List<Nat>, +tt: +List<Nat>, +u: +List<Nat>, +rl: +List<Nat>, +tk: Bool) -> Bool:
  Bool.and(S.all_in(tt, UR(u, rl)), S.implies(tk, S.all_in(q, UR(u, rl))))

def inv3(+s: M.World) -> Bool:
  I3(S.queue(s), S.tk_typed(s), S.users(s), S.released(s), tk_of(s))

def imp_nil(b: Bool, +ys: +List<Nat>) -> T(S.implies(b, S.all_in([], ys))):
  imp_t(b)

def flush_go3(+q: +List<Nat>, +haf: +List<Nat>, +htk: +List<Nat>, +t: +List<Nat>, +d: +List<Nat>, +u: +List<Nat>, +tt: +List<Nat>, +rl: +List<Nat>, +n: Nat, +tk: Bool, +i: Bool, +g: Bool, +h1: T(S.implies(tk, S.all_in(q, UR(u, rl)))), +h: T(S.all_in(tt, UR(u, rl)))) -> T(inv3(M.flush_go(q, M.W{[], haf, htk, t, d, u, tt, rl, n, tk, i, g}))):
  match q:
    case Nil{}:
      and_i(S.all_in(tt, UR(u, rl)), S.implies(tk, S.all_in([], UR(u, rl))), h, imp_nil(tk, UR(u, rl)))
    case Con{+x, +rest}:
      match tk:
        case True{}:
          and_i(S.all_in(M.snoc(tt, x), UR(u, rl)), S.all_in(rest, UR(u, rl)), all_in_snoc_i(tt, UR(u, rl), x, h, mem_in_hd(x, rest, UR(u, rl), h1)), all_in_tl(x, rest, UR(u, rl), h1))
        case False{}:
          and_i(S.all_in(tt, UR(u, rl)), True{}, h, {==})

def flush_b3(i: Bool, +s: M.World, +h: T(inv3(s))) -> T(inv3(M.flush_b(i, s))):
  match i:
    case False{}:
      h
    case True{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          flush_go3(q, haf, htk, t, d, u, tt, rl, n, tk, i, g, and_r(S.all_in(tt, UR(u, rl)), S.implies(tk, S.all_in(q, UR(u, rl))), h), and_l(S.all_in(tt, UR(u, rl)), S.implies(tk, S.all_in(q, UR(u, rl))), h))

def flush3(+s: M.World, +h: T(inv3(s))) -> T(inv3(M.flush(s))):
  match s:
    case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
      flush_b3(i, M.W{q, haf, htk, t, d, u, tt, rl, n, tk, i, g}, h)

def imp_mono(tk: Bool, A: Bool, B: Bool, f: T(A) -> T(B), +h: T(S.implies(tk, A))) -> T(S.implies(tk, B)):
  match tk:
    case False{}:
      {==}
    case True{}:
      f(h)

def I3_grow(+q: +List<Nat>, +tt: +List<Nat>, +u: +List<Nat>, +rl: +List<Nat>, +u2: +List<Nat>, +rl2: +List<Nat>, +tk: Bool, +h: T(I3(q, tt, u, rl, tk)), +hs: T(S.all_in(UR(u, rl), UR(u2, rl2)))) -> T(I3(q, tt, u2, rl2, tk)):
  and_i(S.all_in(tt, UR(u2, rl2)), S.implies(tk, S.all_in(q, UR(u2, rl2))), all_in_trans(tt, UR(u, rl), UR(u2, rl2), and_l(S.all_in(tt, UR(u, rl)), S.implies(tk, S.all_in(q, UR(u, rl))), h), hs), imp_mono(tk, S.all_in(q, UR(u, rl)), S.all_in(q, UR(u2, rl2)), hq => all_in_trans(q, UR(u, rl), UR(u2, rl2), hq, hs), and_r(S.all_in(tt, UR(u, rl)), S.implies(tk, S.all_in(q, UR(u, rl))), h)))

def I3_snoc(+q: +List<Nat>, +tt: +List<Nat>, +u: +List<Nat>, +rl: +List<Nat>, +u2: +List<Nat>, +rl2: +List<Nat>, +tk: Bool, +n: Nat, +h: T(I3(q, tt, u, rl, tk)), +hs: T(S.all_in(UR(u, rl), UR(u2, rl2))), +hn: T(M.mem(n, UR(u2, rl2)))) -> T(I3(M.snoc(q, n), tt, u2, rl2, tk)):
  and_i(S.all_in(tt, UR(u2, rl2)), S.implies(tk, S.all_in(M.snoc(q, n), UR(u2, rl2))), and_l(S.all_in(tt, UR(u2, rl2)), S.implies(tk, S.all_in(q, UR(u2, rl2))), I3_grow(q, tt, u, rl, u2, rl2, tk, h, hs)), imp_mono(tk, S.all_in(q, UR(u2, rl2)), S.all_in(M.snoc(q, n), UR(u2, rl2)), hq => all_in_snoc_i(q, UR(u2, rl2), n, hq, hn), and_r(S.all_in(tt, UR(u2, rl2)), S.implies(tk, S.all_in(q, UR(u2, rl2))), I3_grow(q, tt, u, rl, u2, rl2, tk, h, hs))))

def ur_mono_u(+u: +List<Nat>, +rl: +List<Nat>, +n: Nat) -> T(S.all_in(UR(u, rl), UR(M.snoc(u, n), rl))):
  all_in_app_i(u, rl, UR(M.snoc(u, n), rl), all_in_app_l(u, M.snoc(u, n), rl, all_in_snoc_self(u, n)), all_in_app_r(rl, M.snoc(u, n), rl, all_in_refl(rl)))

def ur_mono_rl(+u: +List<Nat>, +rl: +List<Nat>, +id: Nat) -> T(S.all_in(UR(u, rl), UR(u, M.snoc(rl, id)))):
  all_in_app_i(u, rl, UR(u, M.snoc(rl, id)), all_in_app_l(u, u, M.snoc(rl, id), all_in_refl(u)), all_in_app_r(rl, u, M.snoc(rl, id), all_in_snoc_self(rl, id)))

def mem_ur_u(+u: +List<Nat>, +rl: +List<Nat>, +n: Nat) -> T(M.mem(n, UR(M.snoc(u, n), rl))):
  mem_app_l(n, M.snoc(u, n), rl, mem_snoc_new(u, n))

def mem_ur_rl(+u: +List<Nat>, +rl: +List<Nat>, +id: Nat) -> T(M.mem(id, UR(u, M.snoc(rl, id)))):
  mem_app_r(id, u, M.snoc(rl, id), mem_snoc_new(rl, id))

def I3_refl_ur(+u: +List<Nat>, +rl: +List<Nat>) -> T(S.all_in(UR(u, rl), UR(u, rl))):
  all_in_refl(UR(u, rl))

# send_ev: B2, B3, B6, B9
def send_auto3(fu: Bool, +s: M.World, +h: T(inv3(s))) -> T(inv3(M.send_auto(fu, s))):
  match fu:
    case True{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          flush3(M.W{M.snoc(q, n), haf, htk, t, d, M.snoc(u, n), tt, rl, 1n+n, tk, i, g}, I3_snoc(q, tt, u, rl, M.snoc(u, n), rl, tk, n, h, ur_mono_u(u, rl, n), mem_ur_u(u, rl, n)))
    case False{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          match tk:
            case True{}:
              h
            case False{}:
              flush3(M.W{M.snoc(q, n), haf, htk, t, d, u, tt, rl, 1n+n, False{}, i, g}, h)

def send_ask3(fu: Bool, +s: M.World, +h: T(inv3(s))) -> T(inv3(M.send_live(M.AskFirst{}, fu, s))):
  match fu:
    case True{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          I3_grow(q, tt, u, rl, M.snoc(u, n), rl, tk, h, ur_mono_u(u, rl, n))
    case False{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          h

def send_drop3(fu: Bool, +s: M.World, +h: T(inv3(s))) -> T(inv3(M.send_live(M.RouteDrop{}, fu, s))):
  match fu:
    case True{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          I3_grow(q, tt, u, rl, M.snoc(u, n), rl, tk, h, ur_mono_u(u, rl, n))
    case False{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          h

def send_ev3(fu: Bool, r: M.Route, +s: M.World, +h: T(inv3(s))) -> T(inv3(M.send_ev(fu, r, s))):
  match r:
    case M.Auto{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          match g:
            case True{}:
              h
            case False{}:
              send_auto3(fu, M.W{q, haf, htk, t, d, u, tt, rl, n, tk, i, False{}}, h)
    case M.AskFirst{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          match g:
            case True{}:
              h
            case False{}:
              send_ask3(fu, M.W{q, haf, htk, t, d, u, tt, rl, n, tk, i, False{}}, h)
    case M.RouteDrop{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          match g:
            case True{}:
              h
            case False{}:
              send_drop3(fu, M.W{q, haf, htk, t, d, u, tt, rl, n, tk, i, False{}}, h)

def idle_ev3(+s: M.World, +h: T(inv3(s))) -> T(inv3(M.idle_ev(s))):
  match s:
    case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
      match g:
        case True{}:
          h
        case False{}:
          flush3(M.W{q, haf, htk, t, d, u, tt, rl, n, tk, True{}, False{}}, h)

def busy_ev3(+s: M.World, +h: T(inv3(s))) -> T(inv3(M.busy_ev(s))):
  match s:
    case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
      h

def exit3(+s: M.World, +h: T(inv3(s))) -> T(inv3(M.exit_(s))):
  match s:
    case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
      and_i(S.all_in(tt, UR(u, rl)), S.implies(tk, S.all_in([], UR(u, rl))), and_l(S.all_in(tt, UR(u, rl)), S.implies(tk, S.all_in(q, UR(u, rl))), h), imp_nil(tk, UR(u, rl)))

def ks_in(b: Bool, +x: Nat, +tail: +List<Nat>, +u: +List<Nat>, hm: T(b) -> T(M.mem(x, u)), +ht: T(S.all_in(tail, u))) -> T(S.all_in(M.keep_step(b, x, tail), u)):
  match b:
    case True{}:
      and_i(M.mem(x, u), S.all_in(tail, u), hm({==}), ht)
    case False{}:
      ht

def keep_users_in(+q: +List<Nat>, +u: +List<Nat>) -> T(S.all_in(M.keep_users(q, u), u)):
  match q:
    case Nil{}:
      {==}
    case Con{+x, +rest}:
      ks_in(M.mem(x, u), x, M.keep_users(rest, u), u, hh => hh, keep_users_in(rest, u))

def takeover_begin3(+s: M.World, +h: T(inv3(s))) -> T(inv3(M.takeover_begin(s))):
  match s:
    case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
      and_i(S.all_in(tt, UR(u, rl)), S.all_in(M.keep_users(q, u), UR(u, rl)), and_l(S.all_in(tt, UR(u, rl)), S.implies(tk, S.all_in(q, UR(u, rl))), h), all_in_trans(M.keep_users(q, u), u, UR(u, rl), keep_users_in(q, u), all_in_app_self_l(u, rl)))

def takeover_end3(+s: M.World, +h: T(inv3(s))) -> T(inv3(M.takeover_end(s))):
  match s:
    case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
      flush3(M.W{List.append(&2, Nat, q, htk), haf, [], t, d, u, tt, rl, n, False{}, i, g}, and_i(S.all_in(tt, UR(u, rl)), True{}, and_l(S.all_in(tt, UR(u, rl)), S.implies(tk, S.all_in(q, UR(u, rl))), h), {==}))

def deliver_if3(b: Bool, +id: Nat, +s: M.World, +h: T(inv3(s))) -> T(inv3(M.deliver_if(b, id, s))):
  match b:
    case False{}:
      h
    case True{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          flush3(M.W{M.snoc(q, id), M.remove(id, haf), htk, t, d, u, tt, M.snoc(rl, id), n, tk, i, g}, I3_snoc(q, tt, u, rl, u, M.snoc(rl, id), tk, id, h, ur_mono_rl(u, rl, id), mem_ur_rl(u, rl, id)))

def deliver_tk3(b: Bool, +id: Nat, +s: M.World, +h: T(inv3(s))) -> T(inv3(M.deliver_tk(b, id, s))):
  match b:
    case False{}:
      h
    case True{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          flush3(M.W{M.snoc(q, id), haf, M.remove(id, htk), t, d, u, tt, M.snoc(rl, id), n, tk, i, g}, I3_snoc(q, tt, u, rl, u, M.snoc(rl, id), tk, id, h, ur_mono_rl(u, rl, id), mem_ur_rl(u, rl, id)))

def deliver_pick3(a: Bool, b: Bool, +id: Nat, +s: M.World, +h: T(inv3(s))) -> T(inv3(M.deliver_pick(a, b, id, s))):
  match a:
    case True{}:
      deliver_if3(True{}, id, s, h)
    case False{}:
      deliver_tk3(b, id, s, h)

def user_deliver3(+id: Nat, +s: M.World, +h: T(inv3(s))) -> T(inv3(M.user_deliver(id, s))):
  match s:
    case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
      deliver_pick3(Bool.and(M.mem(id, haf), Bool.not(g)), Bool.and(M.mem(id, htk), Bool.not(g)), id, M.W{q, haf, htk, t, d, u, tt, rl, n, tk, i, g}, h)

def drop_if3(b: Bool, +id: Nat, +s: M.World, +h: T(inv3(s))) -> T(inv3(M.drop_if(b, id, s))):
  match b:
    case False{}:
      h
    case True{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          h

def drop_tk3(b: Bool, +id: Nat, +s: M.World, +h: T(inv3(s))) -> T(inv3(M.drop_tk(b, id, s))):
  match b:
    case False{}:
      h
    case True{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          h

def drop_pick3(a: Bool, b: Bool, +id: Nat, +s: M.World, +h: T(inv3(s))) -> T(inv3(M.drop_pick(a, b, id, s))):
  match a:
    case True{}:
      drop_if3(True{}, id, s, h)
    case False{}:
      drop_tk3(b, id, s, h)

def user_drop3(+id: Nat, +s: M.World, +h: T(inv3(s))) -> T(inv3(M.user_drop(id, s))):
  match s:
    case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
      drop_pick3(M.mem(id, haf), M.mem(id, htk), id, M.W{q, haf, htk, t, d, u, tt, rl, n, tk, i, g}, h)

def step3(e: M.Ev, +s: M.World, +h: T(inv3(s))) -> T(inv3(M.step(e, s))):
  match e:
    case M.Send{fu, r}:
      send_ev3(fu, r, s, h)
    case M.Idle{}:
      idle_ev3(s, h)
    case M.Busy{}:
      busy_ev3(s, h)
    case M.Exit{}:
      exit3(s, h)
    case M.TakeoverBegin{}:
      takeover_begin3(s, h)
    case M.TakeoverEnd{}:
      takeover_end3(s, h)
    case M.UserDeliver{id}:
      user_deliver3(id, s, h)
    case M.UserDrop{id}:
      user_drop3(id, s, h)

def run3(+evs: +List<M.Ev>, +s: M.World, +h: T(inv3(s))) -> T(inv3(M.run(evs, s))):
  match evs:
    case Nil{}:
      h
    case Con{e, +rest}:
      run3(rest, M.step(e, s), step3(e, s, h))

def m3_main(+evs: +List<M.Ev>) -> T(S.all_in(S.tk_typed(S.world(evs)), List.append(&2, Nat, S.users(S.world(evs)), S.released(S.world(evs))))):
  and_l(S.all_in(S.tk_typed(S.world(evs)), UR(S.users(S.world(evs)), S.released(S.world(evs)))), S.implies(tk_of(S.world(evs)), S.all_in(S.queue(S.world(evs)), UR(S.users(S.world(evs)), S.released(S.world(evs))))), run3(evs, M.init(), {==}))

def Laws.m3_takeover_isolates(evs):
  m3_main(evs)

# ---- M2
def fl_go2(+q: +List<Nat>, +haf: +List<Nat>, +htk: +List<Nat>, +t: +List<Nat>, +d: +List<Nat>, +u: +List<Nat>, +tt: +List<Nat>, +rl: +List<Nat>, +n: Nat, +tk: Bool, +i: Bool, +g: Bool, +s0: +List<Nat>) -> T(S.all_in(haf, S.held_af(M.flush_go(q, M.W{s0, haf, htk, t, d, u, tt, rl, n, tk, i, g})))):
  match q:
    case Nil{}:
      all_in_refl(haf)
    case Con{+x, +rest}:
      all_in_refl(haf)

def fl2(+s: M.World) -> T(S.all_in(S.held_af(s), S.held_af(M.flush(s)))):
  match s:
    case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
      match i:
        case False{}:
          all_in_refl(haf)
        case True{}:
          fl_go2(q, haf, htk, t, d, u, tt, rl, n, tk, True{}, g, [])

def H2(+s: M.World, +s2: M.World) -> Bool:
  S.all_in(S.held_af(s), S.held_af(s2))

def send_auto2(fu: Bool, +s: M.World) -> T(H2(s, M.send_auto(fu, s))):
  match fu:
    case True{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          fl2(M.W{M.snoc(q, n), haf, htk, t, d, M.snoc(u, n), tt, rl, 1n+n, tk, i, g})
    case False{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          match tk:
            case True{}:
              all_in_refl(haf)
            case False{}:
              fl2(M.W{M.snoc(q, n), haf, htk, t, d, u, tt, rl, 1n+n, False{}, i, g})

def send_ev2(fu: Bool, r: M.Route, +s: M.World) -> T(H2(s, M.send_ev(fu, r, s))):
  match r:
    case M.Auto{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          match g:
            case True{}:
              all_in_refl(haf)
            case False{}:
              send_auto2(fu, M.W{q, haf, htk, t, d, u, tt, rl, n, tk, i, False{}})
    case M.AskFirst{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          match g:
            case True{}:
              all_in_refl(haf)
            case False{}:
              all_in_snoc_self(haf, n)
    case M.RouteDrop{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          match g:
            case True{}:
              all_in_refl(haf)
            case False{}:
              all_in_refl(haf)

def step2(e: M.Ev, +s: M.World) -> T(S.implies(Bool.not(S.is_user_call(e)), H2(s, M.step(e, s)))):
  match e:
    case M.Send{fu, r}:
      send_ev2(fu, r, s)
    case M.Idle{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          match g:
            case True{}:
              all_in_refl(haf)
            case False{}:
              fl2(M.W{q, haf, htk, t, d, u, tt, rl, n, tk, True{}, False{}})
    case M.Busy{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          all_in_refl(haf)
    case M.Exit{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          all_in_refl(haf)
    case M.TakeoverBegin{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          all_in_refl(haf)
    case M.TakeoverEnd{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          fl2(M.W{List.append(&2, Nat, q, htk), haf, [], t, d, u, tt, rl, n, False{}, i, g})
    case M.UserDeliver{id}:
      {==}
    case M.UserDrop{id}:
      {==}

def m2_main(+evs: +List<M.Ev>, e: M.Ev) -> T(S.implies(Bool.not(S.is_user_call(e)), S.all_in(S.held_af(S.world(evs)), S.held_af(M.step(e, S.world(evs)))))):
  step2(e, S.world(evs))

def Laws.m2_held_leaves_only_by_its_owner(evs, e):
  m2_main(evs, e)

# ---- M4
def prefix_refl(+l: +List<Nat>) -> T(S.is_prefix(l, l)):
  match l:
    case Nil{}:
      {==}
    case Con{+x, +rest}:
      and_i(Nat.is_eq(x, x), S.is_prefix(rest, rest), eq_refl(x), prefix_refl(rest))

def prefix_snoc(+l: +List<Nat>, +y: Nat) -> T(S.is_prefix(l, M.snoc(l, y))):
  match l:
    case Nil{}:
      {==}
    case Con{+x, +rest}:
      and_i(Nat.is_eq(x, x), S.is_prefix(rest, M.snoc(rest, y)), eq_refl(x), prefix_snoc(rest, y))

def fl_go4(+q: +List<Nat>, +haf: +List<Nat>, +htk: +List<Nat>, +t: +List<Nat>, +d: +List<Nat>, +u: +List<Nat>, +tt: +List<Nat>, +rl: +List<Nat>, +n: Nat, +tk: Bool, +i: Bool, +g: Bool, +s0: +List<Nat>) -> T(S.is_prefix(t, S.typed(M.flush_go(q, M.W{s0, haf, htk, t, d, u, tt, rl, n, tk, i, g})))):
  match q:
    case Nil{}:
      prefix_refl(t)
    case Con{+x, +rest}:
      prefix_snoc(t, x)

def fl4(+s: M.World) -> T(S.is_prefix(S.typed(s), S.typed(M.flush(s)))):
  match s:
    case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
      match i:
        case False{}:
          prefix_refl(t)
        case True{}:
          fl_go4(q, haf, htk, t, d, u, tt, rl, n, tk, True{}, g, [])

def send_auto4(fu: Bool, +s: M.World) -> T(S.is_prefix(S.typed(s), S.typed(M.send_auto(fu, s)))):
  match fu:
    case True{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          fl4(M.W{M.snoc(q, n), haf, htk, t, d, M.snoc(u, n), tt, rl, 1n+n, tk, i, g})
    case False{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          match tk:
            case True{}:
              prefix_refl(t)
            case False{}:
              fl4(M.W{M.snoc(q, n), haf, htk, t, d, u, tt, rl, 1n+n, False{}, i, g})

def send_ev4(fu: Bool, r: M.Route, +s: M.World) -> T(S.is_prefix(S.typed(s), S.typed(M.send_ev(fu, r, s)))):
  match r:
    case M.Auto{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          match g:
            case True{}:
              prefix_refl(t)
            case False{}:
              send_auto4(fu, M.W{q, haf, htk, t, d, u, tt, rl, n, tk, i, False{}})
    case M.AskFirst{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          match g:
            case True{}:
              prefix_refl(t)
            case False{}:
              prefix_refl(t)
    case M.RouteDrop{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          match g:
            case True{}:
              prefix_refl(t)
            case False{}:
              prefix_refl(t)

def deliver_if4(b: Bool, +id: Nat, +s: M.World) -> T(S.is_prefix(S.typed(s), S.typed(M.deliver_if(b, id, s)))):
  match b:
    case False{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          prefix_refl(t)
    case True{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          fl4(M.W{M.snoc(q, id), M.remove(id, haf), htk, t, d, u, tt, M.snoc(rl, id), n, tk, i, g})

def deliver_tk4(b: Bool, +id: Nat, +s: M.World) -> T(S.is_prefix(S.typed(s), S.typed(M.deliver_tk(b, id, s)))):
  match b:
    case False{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          prefix_refl(t)
    case True{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          fl4(M.W{M.snoc(q, id), haf, M.remove(id, htk), t, d, u, tt, M.snoc(rl, id), n, tk, i, g})

def deliver_pick4(a: Bool, b: Bool, +id: Nat, +s: M.World) -> T(S.is_prefix(S.typed(s), S.typed(M.deliver_pick(a, b, id, s)))):
  match a:
    case True{}:
      deliver_if4(True{}, id, s)
    case False{}:
      deliver_tk4(b, id, s)

def drop_if4(b: Bool, +id: Nat, +s: M.World) -> T(S.is_prefix(S.typed(s), S.typed(M.drop_if(b, id, s)))):
  match b:
    case False{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          prefix_refl(t)
    case True{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          prefix_refl(t)

def drop_tk4(b: Bool, +id: Nat, +s: M.World) -> T(S.is_prefix(S.typed(s), S.typed(M.drop_tk(b, id, s)))):
  match b:
    case False{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          prefix_refl(t)
    case True{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          prefix_refl(t)

def drop_pick4(a: Bool, b: Bool, +id: Nat, +s: M.World) -> T(S.is_prefix(S.typed(s), S.typed(M.drop_pick(a, b, id, s)))):
  match a:
    case True{}:
      drop_if4(True{}, id, s)
    case False{}:
      drop_tk4(b, id, s)

def step4(e: M.Ev, +s: M.World) -> T(S.is_prefix(S.typed(s), S.typed(M.step(e, s)))):
  match e:
    case M.Send{fu, r}:
      send_ev4(fu, r, s)
    case M.Idle{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          match g:
            case True{}:
              prefix_refl(t)
            case False{}:
              fl4(M.W{q, haf, htk, t, d, u, tt, rl, n, tk, True{}, False{}})
    case M.Busy{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          prefix_refl(t)
    case M.Exit{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          prefix_refl(t)
    case M.TakeoverBegin{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          prefix_refl(t)
    case M.TakeoverEnd{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          fl4(M.W{List.append(&2, Nat, q, htk), haf, [], t, d, u, tt, rl, n, False{}, i, g})
    case M.UserDeliver{+id}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          deliver_pick4(Bool.and(M.mem(id, haf), Bool.not(g)), Bool.and(M.mem(id, htk), Bool.not(g)), id, M.W{q, haf, htk, t, d, u, tt, rl, n, tk, i, g})
    case M.UserDrop{+id}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          drop_pick4(M.mem(id, haf), M.mem(id, htk), id, M.W{q, haf, htk, t, d, u, tt, rl, n, tk, i, g})

def m4_main(+evs: +List<M.Ev>, e: M.Ev) -> T(S.is_prefix(S.typed(S.world(evs)), S.typed(M.step(e, S.world(evs))))):
  step4(e, S.world(evs))

def Laws.m4_typed_only_grows_at_the_end(evs, e):
  m4_main(evs, e)

# ---- M7
def imp_and_l(b: Bool, x: Bool, y: Bool, +h: T(S.implies(b, Bool.and(x, y)))) -> T(S.implies(b, x)):
  match b:
    case False{}:
      {==}
    case True{}:
      and_l(x, y, h)

def imp_and_r(b: Bool, x: Bool, y: Bool, +h: T(S.implies(b, Bool.and(x, y)))) -> T(S.implies(b, y)):
  match b:
    case False{}:
      {==}
    case True{}:
      and_r(x, y, h)

def G7(+s: M.World) -> Bool:
  S.implies(S.gone(s), Bool.and(S.is_nil(S.queue(s)), S.is_nil(S.held_tk(s))))

def flush_go7(+q: +List<Nat>, +haf: +List<Nat>, +htk: +List<Nat>, +t: +List<Nat>, +d: +List<Nat>, +u: +List<Nat>, +tt: +List<Nat>, +rl: +List<Nat>, +n: Nat, +tk: Bool, +i: Bool, +g: Bool, +hq: T(S.implies(g, S.is_nil(q))), +hh: T(S.implies(g, S.is_nil(htk)))) -> T(G7(M.flush_go(q, M.W{[], haf, htk, t, d, u, tt, rl, n, tk, i, g}))):
  match q:
    case Nil{}:
      match g:
        case True{}:
          and_i(True{}, S.is_nil(htk), {==}, hh)
        case False{}:
          {==}
    case Con{+x, +rest}:
      match g:
        case True{}:
          no(T(G7(M.flush_go(x <> rest, M.W{[], haf, htk, t, d, u, tt, rl, n, tk, i, True{}}))), hq)
        case False{}:
          {==}

def flush7(+s: M.World, +h: T(G7(s))) -> T(G7(M.flush(s))):
  match s:
    case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
      match i:
        case False{}:
          h
        case True{}:
          flush_go7(q, haf, htk, t, d, u, tt, rl, n, tk, True{}, g, imp_and_l(g, S.is_nil(q), S.is_nil(htk), h), imp_and_r(g, S.is_nil(q), S.is_nil(htk), h))

def send_auto7(fu: Bool, +q: +List<Nat>, +haf: +List<Nat>, +htk: +List<Nat>, +t: +List<Nat>, +d: +List<Nat>, +u: +List<Nat>, +tt: +List<Nat>, +rl: +List<Nat>, +n: Nat, +tk: Bool, +i: Bool) -> T(G7(M.send_auto(fu, M.W{q, haf, htk, t, d, u, tt, rl, n, tk, i, False{}}))):
  match fu:
    case True{}:
      flush7(M.W{M.snoc(q, n), haf, htk, t, d, M.snoc(u, n), tt, rl, 1n+n, tk, i, False{}}, {==})
    case False{}:
      match tk:
        case True{}:
          {==}
        case False{}:
          flush7(M.W{M.snoc(q, n), haf, htk, t, d, u, tt, rl, 1n+n, False{}, i, False{}}, {==})

def send_ev7(fu: Bool, r: M.Route, +s: M.World, +h: T(G7(s))) -> T(G7(M.send_ev(fu, r, s))):
  match r:
    case M.Auto{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          match g:
            case True{}:
              h
            case False{}:
              send_auto7(fu, q, haf, htk, t, d, u, tt, rl, n, tk, i)
    case M.AskFirst{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          match g:
            case True{}:
              h
            case False{}:
              {==}
    case M.RouteDrop{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          match g:
            case True{}:
              h
            case False{}:
              {==}

def takeover_begin7(+s: M.World, +h: T(G7(s))) -> T(G7(M.takeover_begin(s))):
  match s:
    case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
      match q:
        case Nil{}:
          match htk:
            case Nil{}:
              imp_t(g)
            case Con{+y, +yr}:
              match g:
                case True{}:
                  no(T(G7(M.takeover_begin(M.W{[], haf, y <> yr, t, d, u, tt, rl, n, tk, i, True{}}))), h)
                case False{}:
                  {==}
        case Con{+x, +xr}:
          match g:
            case True{}:
              no(T(G7(M.takeover_begin(M.W{x <> xr, haf, htk, t, d, u, tt, rl, n, tk, i, True{}}))), h)
            case False{}:
              {==}

def takeover_end7(+s: M.World, +h: T(G7(s))) -> T(G7(M.takeover_end(s))):
  match s:
    case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
      match q:
        case Nil{}:
          match htk:
            case Nil{}:
              flush7(M.W{[], haf, [], t, d, u, tt, rl, n, False{}, i, g}, h)
            case Con{+y, +yr}:
              match g:
                case True{}:
                  no(T(G7(M.takeover_end(M.W{[], haf, y <> yr, t, d, u, tt, rl, n, tk, i, True{}}))), h)
                case False{}:
                  flush7(M.W{y <> yr, haf, [], t, d, u, tt, rl, n, False{}, i, False{}}, {==})
        case Con{+x, +xr}:
          match g:
            case True{}:
              no(T(G7(M.takeover_end(M.W{x <> xr, haf, htk, t, d, u, tt, rl, n, tk, i, True{}}))), h)
            case False{}:
              flush7(M.W{List.append(&2, Nat, x <> xr, htk), haf, [], t, d, u, tt, rl, n, False{}, i, False{}}, {==})

def notand(m: Bool) -> T(Bool.not(Bool.and(m, False{}))):
  match m:
    case False{}:
      {==}
    case True{}:
      {==}

def ab7(m: Bool, g: Bool) -> T(S.implies(g, Bool.not(Bool.and(m, Bool.not(g))))):
  match g:
    case True{}:
      notand(m)
    case False{}:
      {==}

def deliver_pick7(a: Bool, b: Bool, +id: Nat, +s: M.World, +h: T(G7(s)), +ha: T(S.implies(S.gone(s), Bool.not(a))), +hb: T(S.implies(S.gone(s), Bool.not(b)))) -> T(G7(M.deliver_pick(a, b, id, s))):
  match a:
    case True{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          match g:
            case True{}:
              no(T(G7(M.deliver_pick(True{}, b, id, M.W{q, haf, htk, t, d, u, tt, rl, n, tk, i, True{}}))), ha)
            case False{}:
              flush7(M.W{M.snoc(q, id), M.remove(id, haf), htk, t, d, u, tt, M.snoc(rl, id), n, tk, i, False{}}, {==})
    case False{}:
      match b:
        case False{}:
          h
        case True{}:
          match s:
            case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
              match g:
                case True{}:
                  no(T(G7(M.deliver_pick(False{}, True{}, id, M.W{q, haf, htk, t, d, u, tt, rl, n, tk, i, True{}}))), hb)
                case False{}:
                  flush7(M.W{M.snoc(q, id), haf, M.remove(id, htk), t, d, u, tt, M.snoc(rl, id), n, tk, i, False{}}, {==})

def drop_pick7(a: Bool, b: Bool, +id: Nat, +s: M.World, +h: T(G7(s))) -> T(G7(M.drop_pick(a, b, id, s))):
  match a:
    case True{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          h
    case False{}:
      match b:
        case False{}:
          h
        case True{}:
          match s:
            case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
              match htk:
                case Nil{}:
                  h
                case Con{+y, +yr}:
                  match g:
                    case True{}:
                      no(T(G7(M.drop_pick(False{}, True{}, id, M.W{q, haf, y <> yr, t, d, u, tt, rl, n, tk, i, True{}}))), and_r(S.is_nil(q), False{}, h))
                    case False{}:
                      {==}

def step7(e: M.Ev, +s: M.World, +h: T(G7(s))) -> T(G7(M.step(e, s))):
  match e:
    case M.Send{fu, r}:
      send_ev7(fu, r, s, h)
    case M.Idle{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          match g:
            case True{}:
              h
            case False{}:
              flush7(M.W{q, haf, htk, t, d, u, tt, rl, n, tk, True{}, False{}}, {==})
    case M.Busy{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          h
    case M.Exit{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          {==}
    case M.TakeoverBegin{}:
      takeover_begin7(s, h)
    case M.TakeoverEnd{}:
      takeover_end7(s, h)
    case M.UserDeliver{+id}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          deliver_pick7(Bool.and(M.mem(id, haf), Bool.not(g)), Bool.and(M.mem(id, htk), Bool.not(g)), id, M.W{q, haf, htk, t, d, u, tt, rl, n, tk, i, g}, h, ab7(M.mem(id, haf), g), ab7(M.mem(id, htk), g))
    case M.UserDrop{+id}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          drop_pick7(M.mem(id, haf), M.mem(id, htk), id, M.W{q, haf, htk, t, d, u, tt, rl, n, tk, i, g}, h)

def run7(+evs: +List<M.Ev>, +s: M.World, +h: T(G7(s))) -> T(G7(M.run(evs, s))):
  match evs:
    case Nil{}:
      h
    case Con{e, +rest}:
      run7(rest, M.step(e, s), step7(e, s, h))

def send_gone7(fu: Bool, r: M.Route, +s: M.World) -> T(S.implies(S.gone(s), Nat.is_eq(S.next_id(M.step(M.Send{fu, r}, s)), S.next_id(s)))):
  match r:
    case M.Auto{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          match g:
            case True{}:
              eq_refl(n)
            case False{}:
              {==}
    case M.AskFirst{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          match g:
            case True{}:
              eq_refl(n)
            case False{}:
              {==}
    case M.RouteDrop{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          match g:
            case True{}:
              eq_refl(n)
            case False{}:
              {==}

def m7_main(+evs: +List<M.Ev>, +fu: Bool, +r: M.Route) -> T(Bool.and(S.implies(S.gone(S.world(evs)), Bool.and(S.is_nil(S.queue(S.world(evs))), S.is_nil(S.held_tk(S.world(evs))))), S.implies(S.gone(S.world(evs)), Nat.is_eq(S.next_id(M.step(M.Send{fu, r}, S.world(evs))), S.next_id(S.world(evs)))))):
  and_i(G7(S.world(evs)), S.implies(S.gone(S.world(evs)), Nat.is_eq(S.next_id(M.step(M.Send{fu, r}, S.world(evs))), S.next_id(S.world(evs)))), run7(evs, M.init(), {==}), send_gone7(fu, r, S.world(evs)))

def Laws.m7_receiver_gone(evs, fu, r):
  m7_main(evs, fu, r)

# ---- M10
def le_zero(p: Nat) -> T(Nat.is_le(0n, p)):
  match p:
    case 0n:
      {==}
    case 1n+pp:
      {==}

def le_refl(n: Nat) -> T(Nat.is_le(n, n)):
  match n:
    case 0n:
      {==}
    case 1n+p:
      le_refl(p)

def le0_nil(+l: +List<Nat>, +h: T(Nat.is_le(S.len(l), 0n))) -> T(S.is_nil(l)):
  match l:
    case Nil{}:
      {==}
    case Con{+x, +rest}:
      no(T(S.is_nil(x <> rest)), h)

def imp10(g: Bool, X: Bool, f: T(Bool.not(g)) -> T(X)) -> T(S.implies(Bool.not(g), X)):
  match g:
    case True{}:
      {==}
    case False{}:
      f({==})

def idle2(+s: M.World) -> M.World:
  M.step(M.Idle{}, M.step(M.Busy{}, s))

def g10(+s: M.World, +hg: T(Bool.not(S.gone(s)))) -> T(Bool.not(S.gone(idle2(s)))):
  match s:
    case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
      match q:
        case Nil{}:
          match g:
            case True{}:
              no(T(Bool.not(S.gone(idle2(M.W{[], haf, htk, t, d, u, tt, rl, n, tk, i, True{}})))), hg)
            case False{}:
              {==}
        case Con{+x, +rest}:
          match g:
            case True{}:
              no(T(Bool.not(S.gone(idle2(M.W{x <> rest, haf, htk, t, d, u, tt, rl, n, tk, i, True{}})))), hg)
            case False{}:
              {==}

def k10(p: Nat, +s: M.World, +hg: T(Bool.not(S.gone(s))), +hk: T(Nat.is_le(S.len(S.queue(s)), 1n+p))) -> T(Nat.is_le(S.len(S.queue(idle2(s))), p)):
  match s:
    case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
      match q:
        case Nil{}:
          match g:
            case True{}:
              no(T(Nat.is_le(S.len(S.queue(idle2(M.W{[], haf, htk, t, d, u, tt, rl, n, tk, i, True{}}))), p)), hg)
            case False{}:
              le_zero(p)
        case Con{+x, +rest}:
          match g:
            case True{}:
              no(T(Nat.is_le(S.len(S.queue(idle2(M.W{x <> rest, haf, htk, t, d, u, tt, rl, n, tk, i, True{}}))), p)), hg)
            case False{}:
              hk

def cyc10(+k: Nat, +s: M.World, +hg: T(Bool.not(S.gone(s))), +hk: T(Nat.is_le(S.len(S.queue(s)), k))) -> T(S.is_nil(S.queue(M.run(S.cycle(k), s)))):
  match k:
    case 0n:
      le0_nil(S.queue(s), hk)
    case 1n++p:
      cyc10(p, idle2(s), g10(s, hg), k10(p, s, hg, hk))

def m10_main(+evs: +List<M.Ev>) -> T(S.implies(Bool.not(S.gone(S.world(evs))), S.is_nil(S.queue(M.run(S.cycle(S.len(S.queue(S.world(evs)))), S.world(evs)))))):
  imp10(S.gone(S.world(evs)), S.is_nil(S.queue(M.run(S.cycle(S.len(S.queue(S.world(evs)))), S.world(evs)))), hg => cyc10(S.len(S.queue(S.world(evs))), S.world(evs), hg, le_refl(S.len(S.queue(S.world(evs))))))

def Laws.m10_pending_to_idle_is_eventually_typed(evs):
  m10_main(evs)

