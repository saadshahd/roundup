import Base
import ./LAWS.bend as Laws
import ./model.bend as M
import ./spec.bend as S

law flush_haf:
  for +s: M.World
  {S.held_af(M.flush(s)) == S.held_af(s) : +List<Nat>}

def flush_haf(s):
  match s:
    case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
      match q i:
        case Nil{} False{}:
          {==}
        case Nil{} True{}:
          {==}
        case Con{+x, rest} False{}:
          {==}
        case Con{+x, rest} True{}:
          {==}

law eq_refl:
  for +x: Nat
  {Nat.is_eq(x, x) == True{} : Bool}

def eq_refl(x):
  match x:
    case 0n:
      {==}
    case 1n+p:
      eq_refl(p)

law or_true_r:
  for b: Bool
  {Bool.or(b, True{}) == True{} : Bool}

def or_true_r(b):
  match b:
    case False{}:
      {==}
    case True{}:
      {==}

law mem_cons:
  for +x: Nat
  for +y: Nat
  for +ys: +List<Nat>
  for +h: {M.mem(x, ys) == True{} : Bool}
  {M.mem(x, y <> ys) == True{} : Bool}

def mem_cons(x, y, ys, h):
  %Equal.sym(Bool, M.mem(x, ys), True{}, h) : {Bool.or(Nat.is_eq(x, y), _) == True{} : Bool}
  or_true_r(Nat.is_eq(x, y))

def disc_b(b: Bool) -> Type:
  match b:
    case False{}:
      Empty
    case True{}:
      Unit

def ff_tt(h: {False{} == True{} : Bool}) -> Empty:
  %Equal.sym(Bool, False{}, True{}, h) : disc_b(_)
  Unit{}

law and_l:
  for a: Bool
  for b: Bool
  for +h: {Bool.and(a, b) == True{} : Bool}
  {a == True{} : Bool}

def and_l(a, b, h):
  match a:
    case False{}:
      Empty.absurd({False{} == True{} : Bool}, ff_tt(h))
    case True{}:
      {==}

law and_r:
  for a: Bool
  for b: Bool
  for +h: {Bool.and(a, b) == True{} : Bool}
  {b == True{} : Bool}

def and_r(a, b, h):
  match a:
    case False{}:
      Empty.absurd({b == True{} : Bool}, ff_tt(h))
    case True{}:
      h

law all_in_cons:
  for +xs: +List<Nat>
  for +y: Nat
  for +ys: +List<Nat>
  for +h: {S.all_in(xs, ys) == True{} : Bool}
  {S.all_in(xs, y <> ys) == True{} : Bool}

def all_in_cons(xs, y, ys, h):
  match xs:
    case Nil{}:
      {==}
    case Con{+x, rest}:
      %Equal.sym(Bool, M.mem(x, ys), True{}, and_l(M.mem(x, ys), S.all_in(rest, ys), h)) : {Bool.and(Bool.or(Nat.is_eq(x, y), _), S.all_in(rest, y <> ys)) == True{} : Bool}
      %Equal.sym(Bool, S.all_in(rest, y <> ys), True{}, all_in_cons(rest, y, ys, and_r(M.mem(x, ys), S.all_in(rest, ys), h))) : {Bool.and(Bool.or(Nat.is_eq(x, y), True{}), _) == True{} : Bool}
      %Equal.sym(Bool, Bool.or(Nat.is_eq(x, y), True{}), True{}, or_true_r(Nat.is_eq(x, y))) : {Bool.and(_, True{}) == True{} : Bool}
      {==}

law all_in_refl:
  for +xs: +List<Nat>
  {S.all_in(xs, xs) == True{} : Bool}

def all_in_refl(xs):
  match xs:
    case Nil{}:
      {==}
    case Con{+x, rest}:
      %Equal.sym(Bool, Nat.is_eq(x, x), True{}, eq_refl(x)) : {Bool.and(Bool.or(_, M.mem(x, rest)), S.all_in(rest, x <> rest)) == True{} : Bool}
      %Equal.sym(Bool, S.all_in(rest, x <> rest), True{}, all_in_cons(rest, x, rest, all_in_refl(rest))) : {Bool.and(Bool.or(True{}, M.mem(x, rest)), _) == True{} : Bool}
      {==}

law all_in_snoc:
  for +xs: +List<Nat>
  for +y: Nat
  {S.all_in(xs, M.snoc(xs, y)) == True{} : Bool}

def all_in_snoc(xs, y):
  match xs:
    case Nil{}:
      {==}
    case Con{+x, rest}:
      %Equal.sym(Bool, Nat.is_eq(x, x), True{}, eq_refl(x)) : {Bool.and(Bool.or(_, M.mem(x, M.snoc(rest, y))), S.all_in(rest, x <> M.snoc(rest, y))) == True{} : Bool}
      %Equal.sym(Bool, S.all_in(rest, x <> M.snoc(rest, y)), True{}, all_in_cons(rest, x, M.snoc(rest, y), all_in_snoc(rest, y))) : {Bool.and(Bool.or(True{}, M.mem(x, M.snoc(rest, y))), _) == True{} : Bool}
      {==}

law flush_keeps:
  for +s: M.World
  {S.all_in(S.held_af(s), S.held_af(M.flush(s))) == True{} : Bool}

def flush_keeps(s):
  %Equal.sym(+List<Nat>, S.held_af(M.flush(s)), S.held_af(s), flush_haf(s)) : {S.all_in(S.held_af(s), _) == True{} : Bool}
  all_in_refl(S.held_af(s))

law keeps_send_route:
  for fu: Bool
  for +tk: Bool
  for +s: M.World
  {S.all_in(S.held_af(s), S.held_af(M.send_route(fu, tk, s))) == True{} : Bool}

def keeps_send_route(fu, tk, s):
  match fu:
    case True{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk2, +i, +g}:
          flush_keeps(M.W{M.snoc(q, n), haf, htk, t, d, M.snoc(u, n), tt, rl, 1n+n, tk2, i, g})
    case False{}:
      match tk:
        case True{}:
          match s:
            case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk2, +i, +g}:
              all_in_refl(haf)
        case False{}:
          match s:
            case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk2, +i, +g}:
              flush_keeps(M.W{M.snoc(q, n), haf, htk, t, d, u, tt, rl, 1n+n, tk2, i, g})

law keeps_step:
  for e: M.Ev
  for +s: M.World
  {S.implies(Bool.not(S.is_user_call(e)), S.all_in(S.held_af(s), S.held_af(M.step(e, s)))) == True{} : Bool}

def keeps_step(e, s):
  match e:
    case M.Send{fu, r}:
      match r:
        case M.Auto{}:
          match s:
            case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
              match g:
                case True{}:
                  all_in_refl(haf)
                case False{}:
                  keeps_send_route(fu, tk, M.W{q, haf, htk, t, d, u, tt, rl, n, tk, i, g})
        case M.AskFirst{}:
          match s:
            case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
              match g:
                case True{}:
                  all_in_refl(haf)
                case False{}:
                  all_in_snoc(haf, n)
        case M.RouteDrop{}:
          match s:
            case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
              match g:
                case True{}:
                  all_in_refl(haf)
                case False{}:
                  all_in_refl(haf)
    case M.Idle{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          match g:
            case True{}:
              all_in_refl(haf)
            case False{}:
              flush_keeps(M.W{q, haf, htk, t, d, u, tt, rl, n, tk, True{}, g})
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
          flush_keeps(M.W{List.append(&2, Nat, q, htk), haf, [], t, d, u, tt, rl, n, False{}, i, g})
    case M.UserDeliver{id}:
      {==}
    case M.UserDrop{id}:
      {==}

def Laws.m2_held_leaves_only_by_its_owner(evs, e):
  keeps_step(e, S.world(evs))

def disc_n(n: Nat) -> Type:
  match n:
    case 0n:
      Empty
    case 1n+p:
      Unit

def pred(n: Nat) -> Nat:
  match n:
    case 0n:
      0n
    case 1n+p:
      p

law succ_inj:
  for +a: Nat
  for +b: Nat
  for +h: {1n+a == 1n+b : Nat}
  {a == b : Nat}

def succ_inj(a, b, h):
  Equal.cong(Nat, Nat, n => pred(n), 1n+a, 1n+b, h)

def succ_zero(a: Nat, +h: {1n+a == 0n : Nat}) -> Empty:
  %h : disc_n(_)
  Unit{}

def zero_succ(a: Nat, +h: {0n == 1n+a : Nat}) -> Empty:
  succ_zero(a, Equal.sym(Nat, 0n, 1n+a, h))

def tt_ff(+h: {True{} == False{} : Bool}) -> Empty:
  %h : disc_b(_)
  Unit{}

law m10_go:
  for +k: Nat
  for +s: M.World
  for +hg: {S.gone(s) == False{} : Bool}
  for +hl: {S.len(S.queue(s)) == k : Nat}
  {S.is_nil(S.queue(M.run(S.cycle(k), s))) == True{} : Bool}

def m10_go(k, s, hg, hl):
  match k:
    case 0n:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          match q:
            case Nil{}:
              {==}
            case Con{+x, rest}:
              Empty.absurd({S.is_nil(S.queue(M.run(S.cycle(0n), s))) == True{} : Bool}, succ_zero(S.len(rest), hl))
    case 1n+p:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          match q g:
            case Nil{} _:
              Empty.absurd({S.is_nil(S.queue(M.run(S.cycle(1n+p), s))) == True{} : Bool}, zero_succ(p, hl))
            case Con{+x, rest} True{}:
              Empty.absurd({S.is_nil(S.queue(M.run(S.cycle(1n+p), s))) == True{} : Bool}, tt_ff(hg))
            case Con{+x, rest} False{}:
              m10_go(p, M.W{rest, haf, htk, M.snoc(t, x), d, u, M.record(tk, x, tt), rl, n, tk, False{}, False{}}, hg, succ_inj(S.len(rest), p, hl))

law m10_w:
  for +s: M.World
  {S.implies(Bool.not(S.gone(s)), S.is_nil(S.queue(M.run(S.cycle(S.len(S.queue(s))), s)))) == True{} : Bool}

def m10_w(s):
  match s:
    case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
      match g:
        case True{}:
          {==}
        case False{}:
          m10_go(S.len(q), M.W{q, haf, htk, t, d, u, tt, rl, n, tk, i, False{}}, {==}, {==})

def Laws.m10_pending_to_idle_is_eventually_typed(evs):
  m10_w(S.world(evs))

law or_mono:
  for a: Bool
  for b: Bool
  for c: Bool
  for +h: {Bool.or(a, b) == True{} : Bool}
  for f: @k: {b == True{} : Bool} -> {c == True{} : Bool}
  {Bool.or(a, c) == True{} : Bool}

def or_mono(a, b, c, h, f):
  match a:
    case False{}:
      f(h)
    case True{}:
      {==}

law or_intro_r:
  for a: Bool
  for b: Bool
  for +h: {b == True{} : Bool}
  {Bool.or(a, b) == True{} : Bool}

def or_intro_r(a, b, h):
  match a:
    case False{}:
      h
    case True{}:
      {==}

law eq_sound:
  for +x: Nat
  for +y: Nat
  for +h: {Nat.is_eq(x, y) == True{} : Bool}
  {x == y : Nat}

def eq_sound(x, y, h):
  match x y:
    case 0n 0n:
      {==}
    case 0n 1n+yp:
      Empty.absurd({0n == 1n+yp : Nat}, ff_tt(h))
    case 1n+xp 0n:
      Empty.absurd({1n+xp == 0n : Nat}, ff_tt(h))
    case 1n+xp 1n+yp:
      %eq_sound(xp, yp, h) : {1n+xp == 1n+_ : Nat}
      {==}

law or_elim:
  for a: Bool
  for b: Bool
  for c: Bool
  for +h: {Bool.or(a, b) == True{} : Bool}
  for f: @k: {a == True{} : Bool} -> {c == True{} : Bool}
  for g: @k: {b == True{} : Bool} -> {c == True{} : Bool}
  {c == True{} : Bool}

def or_elim(a, b, c, h, f, g):
  match a:
    case False{}:
      g(h)
    case True{}:
      f({==})

law mem_hd:
  for +x: Nat
  for +ys: +List<Nat>
  {M.mem(x, x <> ys) == True{} : Bool}

def mem_hd(x, ys):
  %Equal.sym(Bool, Nat.is_eq(x, x), True{}, eq_refl(x)) : {Bool.or(_, M.mem(x, ys)) == True{} : Bool}
  {==}

law mem_app_l:
  for +x: Nat
  for +a: +List<Nat>
  for +b: +List<Nat>
  for +h: {M.mem(x, a) == True{} : Bool}
  {M.mem(x, List.append(&2, Nat, a, b)) == True{} : Bool}

def mem_app_l(x, a, b, h):
  match a:
    case Nil{}:
      Empty.absurd({M.mem(x, List.append(&2, Nat, [], b)) == True{} : Bool}, ff_tt(h))
    case Con{+y, rest}:
      or_mono(Nat.is_eq(x, y), M.mem(x, rest), M.mem(x, List.append(&2, Nat, rest, b)), h, k => mem_app_l(x, rest, b, k))

law mem_app_r:
  for +x: Nat
  for +a: +List<Nat>
  for +b: +List<Nat>
  for +h: {M.mem(x, b) == True{} : Bool}
  {M.mem(x, List.append(&2, Nat, a, b)) == True{} : Bool}

def mem_app_r(x, a, b, h):
  match a:
    case Nil{}:
      h
    case Con{+y, rest}:
      %Equal.sym(Bool, M.mem(x, List.append(&2, Nat, rest, b)), True{}, mem_app_r(x, rest, b, h)) : {Bool.or(Nat.is_eq(x, y), _) == True{} : Bool}
      or_true_r(Nat.is_eq(x, y))

law mem_snoc_new:
  for +l: +List<Nat>
  for +y: Nat
  {M.mem(y, M.snoc(l, y)) == True{} : Bool}

def mem_snoc_new(l, y):
  mem_app_r(y, l, [y], mem_hd(y, []))

law mem_via_eq:
  for +x: Nat
  for +z: Nat
  for +ys: +List<Nat>
  for +j: {Nat.is_eq(x, z) == True{} : Bool}
  for +hz: {M.mem(z, ys) == True{} : Bool}
  {M.mem(x, ys) == True{} : Bool}

def mem_via_eq(x, z, ys, j, hz):
  %Equal.sym(Nat, x, z, eq_sound(x, z, j)) : {M.mem(_, ys) == True{} : Bool}
  hz

law or_intro_l:
  for a: Bool
  for b: Bool
  for +h: {a == True{} : Bool}
  {Bool.or(a, b) == True{} : Bool}

def or_intro_l(a, b, h):
  %Equal.sym(Bool, a, True{}, h) : {Bool.or(_, b) == True{} : Bool}
  {==}

# ---- M3: in2 / all2, "every element is a user's or released" ----

law and_intro:
  for a: Bool
  for b: Bool
  for +ha: {a == True{} : Bool}
  for +hb: {b == True{} : Bool}
  {Bool.and(a, b) == True{} : Bool}

def and_intro(a, b, ha, hb):
  %Equal.sym(Bool, a, True{}, ha) : {Bool.and(_, b) == True{} : Bool}
  hb

law or_monol:
  for a: Bool
  for a2: Bool
  for b: Bool
  for +h: {Bool.or(a, b) == True{} : Bool}
  for f: @k: {a == True{} : Bool} -> {a2 == True{} : Bool}
  {Bool.or(a2, b) == True{} : Bool}

def or_monol(a, a2, b, h, f):
  match a:
    case False{}:
      or_intro_r(a2, b, h)
    case True{}:
      or_intro_l(a2, b, f({==}))

def in2(+x: Nat, +u: +List<Nat>, +rl: +List<Nat>) -> Bool:
  Bool.or(M.mem(x, u), M.mem(x, rl))

def all2(xs: +List<Nat>, +u: +List<Nat>, +rl: +List<Nat>) -> Bool:
  match xs:
    case Nil{}:
      True{}
    case Con{+x, rest}:
      Bool.and(in2(x, u, rl), all2(rest, u, rl))

law in2_u_grow:
  for +x: Nat
  for +u: +List<Nat>
  for +rl: +List<Nat>
  for +n: Nat
  for +h: {in2(x, u, rl) == True{} : Bool}
  {in2(x, M.snoc(u, n), rl) == True{} : Bool}

def in2_u_grow(x, u, rl, n, h):
  or_monol(M.mem(x, u), M.mem(x, M.snoc(u, n)), M.mem(x, rl), h, k => mem_app_l(x, u, [n], k))

law in2_rl_grow:
  for +x: Nat
  for +u: +List<Nat>
  for +rl: +List<Nat>
  for +n: Nat
  for +h: {in2(x, u, rl) == True{} : Bool}
  {in2(x, u, M.snoc(rl, n)) == True{} : Bool}

def in2_rl_grow(x, u, rl, n, h):
  or_mono(M.mem(x, u), M.mem(x, rl), M.mem(x, M.snoc(rl, n)), h, k => mem_app_l(x, rl, [n], k))

law all2_u_grow:
  for +xs: +List<Nat>
  for +u: +List<Nat>
  for +rl: +List<Nat>
  for +n: Nat
  for +h: {all2(xs, u, rl) == True{} : Bool}
  {all2(xs, M.snoc(u, n), rl) == True{} : Bool}

def all2_u_grow(xs, u, rl, n, h):
  match xs:
    case Nil{}:
      {==}
    case Con{+x, rest}:
      and_intro(in2(x, M.snoc(u, n), rl), all2(rest, M.snoc(u, n), rl), in2_u_grow(x, u, rl, n, and_l(in2(x, u, rl), all2(rest, u, rl), h)), all2_u_grow(rest, u, rl, n, and_r(in2(x, u, rl), all2(rest, u, rl), h)))

law all2_rl_grow:
  for +xs: +List<Nat>
  for +u: +List<Nat>
  for +rl: +List<Nat>
  for +n: Nat
  for +h: {all2(xs, u, rl) == True{} : Bool}
  {all2(xs, u, M.snoc(rl, n)) == True{} : Bool}

def all2_rl_grow(xs, u, rl, n, h):
  match xs:
    case Nil{}:
      {==}
    case Con{+x, rest}:
      and_intro(in2(x, u, M.snoc(rl, n)), all2(rest, u, M.snoc(rl, n)), in2_rl_grow(x, u, rl, n, and_l(in2(x, u, rl), all2(rest, u, rl), h)), all2_rl_grow(rest, u, rl, n, and_r(in2(x, u, rl), all2(rest, u, rl), h)))

law all2_snoc:
  for +xs: +List<Nat>
  for +u: +List<Nat>
  for +rl: +List<Nat>
  for +y: Nat
  for +h: {all2(xs, u, rl) == True{} : Bool}
  for +hy: {in2(y, u, rl) == True{} : Bool}
  {all2(M.snoc(xs, y), u, rl) == True{} : Bool}

def all2_snoc(xs, u, rl, y, h, hy):
  match xs:
    case Nil{}:
      and_intro(in2(y, u, rl), True{}, hy, {==})
    case Con{+x, rest}:
      and_intro(in2(x, u, rl), all2(M.snoc(rest, y), u, rl), and_l(in2(x, u, rl), all2(rest, u, rl), h), all2_snoc(rest, u, rl, y, and_r(in2(x, u, rl), all2(rest, u, rl), h), hy))

law keep_step_all2:
  for b: Bool
  for +x: Nat
  for +tail: +List<Nat>
  for +u: +List<Nat>
  for +rl: +List<Nat>
  for +hb: {M.mem(x, u) == b : Bool}
  for +ht: {all2(tail, u, rl) == True{} : Bool}
  {all2(M.keep_step(b, x, tail), u, rl) == True{} : Bool}

def keep_step_all2(b, x, tail, u, rl, hb, ht):
  match b:
    case False{}:
      ht
    case True{}:
      and_intro(in2(x, u, rl), all2(tail, u, rl), or_intro_l(M.mem(x, u), M.mem(x, rl), hb), ht)

law all2_keep:
  for +q: +List<Nat>
  for +u: +List<Nat>
  for +rl: +List<Nat>
  {all2(M.keep_users(q, u), u, rl) == True{} : Bool}

def all2_keep(q, u, rl):
  match q:
    case Nil{}:
      {==}
    case Con{+x, rest}:
      keep_step_all2(M.mem(x, u), x, M.keep_users(rest, u), u, rl, {==}, all2_keep(rest, u, rl))

# ---- M3 invariant: what a Takeover let into the typed record, and what it lets stay pending ----

def Inv3(s: M.World) -> Data:
  match s:
    case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
      {Bool.and(all2(tt, u, rl), S.implies(tk, all2(q, u, rl))) == True{} : Bool}

law mk3:
  for +tt: +List<Nat>
  for +q: +List<Nat>
  for +u: +List<Nat>
  for +rl: +List<Nat>
  for +tk: Bool
  for +ia: {all2(tt, u, rl) == True{} : Bool}
  for ib: @k: {tk == True{} : Bool} -> {all2(q, u, rl) == True{} : Bool}
  {Bool.and(all2(tt, u, rl), S.implies(tk, all2(q, u, rl))) == True{} : Bool}

def mk3(tt, q, u, rl, tk, ia, ib):
  match tk:
    case True{}:
      and_intro(all2(tt, u, rl), all2(q, u, rl), ia, ib({==}))
    case False{}:
      and_intro(all2(tt, u, rl), True{}, ia, {==})

law p1:
  for +tt: +List<Nat>
  for +q: +List<Nat>
  for +u: +List<Nat>
  for +rl: +List<Nat>
  for +tk: Bool
  for +I: {Bool.and(all2(tt, u, rl), S.implies(tk, all2(q, u, rl))) == True{} : Bool}
  {all2(tt, u, rl) == True{} : Bool}

def p1(tt, q, u, rl, tk, I):
  and_l(all2(tt, u, rl), S.implies(tk, all2(q, u, rl)), I)

law p2:
  for +tt: +List<Nat>
  for +q: +List<Nat>
  for +u: +List<Nat>
  for +rl: +List<Nat>
  for +tk: Bool
  for +I: {Bool.and(all2(tt, u, rl), S.implies(tk, all2(q, u, rl))) == True{} : Bool}
  for +ht: {tk == True{} : Bool}
  {all2(q, u, rl) == True{} : Bool}

def p2(tt, q, u, rl, tk, I, ht):
  match tk:
    case True{}:
      and_r(all2(tt, u, rl), all2(q, u, rl), I)
    case False{}:
      Empty.absurd({all2(q, u, rl) == True{} : Bool}, ff_tt(ht))

law inv3_init:
  Inv3(M.init())

def inv3_init():
  {==}

law inv3_flush:
  for +s: M.World
  for +I: Inv3(s)
  Inv3(M.flush(s))

def inv3_flush(s, I):
  match s:
    case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
      match q tk i:
        case Nil{} _ False{}:
          I
        case Nil{} _ True{}:
          I
        case Con{+x, rest} True{} False{}:
          I
        case Con{+x, rest} False{} False{}:
          I
        case Con{+x, rest} True{} True{}:
          mk3(M.snoc(tt, x), rest, u, rl, True{}, all2_snoc(tt, u, rl, x, p1(tt, x <> rest, u, rl, True{}, I), and_l(in2(x, u, rl), all2(rest, u, rl), p2(tt, x <> rest, u, rl, True{}, I, {==}))), k => and_r(in2(x, u, rl), all2(rest, u, rl), p2(tt, x <> rest, u, rl, True{}, I, {==})))
        case Con{+x, rest} False{} True{}:
          mk3(tt, rest, u, rl, False{}, p1(tt, x <> rest, u, rl, False{}, I), k => Empty.absurd({all2(rest, u, rl) == True{} : Bool}, ff_tt(k)))

law inv3_send_auto:
  for fu: Bool
  for +s: M.World
  for +I: Inv3(s)
  Inv3(M.send_auto(fu, s))

def inv3_send_auto(fu, s, I):
  match fu:
    case True{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          inv3_flush(M.W{M.snoc(q, n), haf, htk, t, d, M.snoc(u, n), tt, rl, 1n+n, tk, i, g}, mk3(tt, M.snoc(q, n), M.snoc(u, n), rl, tk, all2_u_grow(tt, u, rl, n, p1(tt, q, u, rl, tk, I)), k => all2_snoc(q, M.snoc(u, n), rl, n, all2_u_grow(q, u, rl, n, p2(tt, q, u, rl, tk, I, k)), or_intro_l(M.mem(n, M.snoc(u, n)), M.mem(n, rl), mem_snoc_new(u, n)))))
    case False{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          match tk:
            case True{}:
              I
            case False{}:
              inv3_flush(M.W{M.snoc(q, n), haf, htk, t, d, u, tt, rl, 1n+n, False{}, i, g}, mk3(tt, M.snoc(q, n), u, rl, False{}, p1(tt, q, u, rl, False{}, I), k => Empty.absurd({all2(M.snoc(q, n), u, rl) == True{} : Bool}, ff_tt(k))))

law inv3_owned:
  for fu: Bool
  for +n: Nat
  for +q: +List<Nat>
  for +tt: +List<Nat>
  for +u: +List<Nat>
  for +rl: +List<Nat>
  for +tk: Bool
  for +I: {Bool.and(all2(tt, u, rl), S.implies(tk, all2(q, u, rl))) == True{} : Bool}
  {Bool.and(all2(tt, M.owned(fu, n, u), rl), S.implies(tk, all2(q, M.owned(fu, n, u), rl))) == True{} : Bool}

def inv3_owned(fu, n, q, tt, u, rl, tk, I):
  match fu:
    case True{}:
      mk3(tt, q, M.snoc(u, n), rl, tk, all2_u_grow(tt, u, rl, n, p1(tt, q, u, rl, tk, I)), k => all2_u_grow(q, u, rl, n, p2(tt, q, u, rl, tk, I, k)))
    case False{}:
      I

law inv3_deliver_if:
  for b: Bool
  for +id: Nat
  for +s: M.World
  for +I: Inv3(s)
  Inv3(M.deliver_if(b, id, s))

def inv3_deliver_if(b, id, s, I):
  match b:
    case False{}:
      I
    case True{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          inv3_flush(M.W{M.snoc(q, id), M.remove(id, haf), htk, t, d, u, tt, M.snoc(rl, id), n, tk, i, g}, mk3(tt, M.snoc(q, id), u, M.snoc(rl, id), tk, all2_rl_grow(tt, u, rl, id, p1(tt, q, u, rl, tk, I)), k => all2_snoc(q, u, M.snoc(rl, id), id, all2_rl_grow(q, u, rl, id, p2(tt, q, u, rl, tk, I, k)), or_intro_r(M.mem(id, u), M.mem(id, M.snoc(rl, id)), mem_snoc_new(rl, id)))))

law inv3_deliver_tk:
  for b: Bool
  for +id: Nat
  for +s: M.World
  for +I: Inv3(s)
  Inv3(M.deliver_tk(b, id, s))

def inv3_deliver_tk(b, id, s, I):
  match b:
    case False{}:
      I
    case True{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          inv3_flush(M.W{M.snoc(q, id), haf, M.remove(id, htk), t, d, u, tt, M.snoc(rl, id), n, tk, i, g}, mk3(tt, M.snoc(q, id), u, M.snoc(rl, id), tk, all2_rl_grow(tt, u, rl, id, p1(tt, q, u, rl, tk, I)), k => all2_snoc(q, u, M.snoc(rl, id), id, all2_rl_grow(q, u, rl, id, p2(tt, q, u, rl, tk, I, k)), or_intro_r(M.mem(id, u), M.mem(id, M.snoc(rl, id)), mem_snoc_new(rl, id)))))

law inv3_deliver_pick:
  for a: Bool
  for b: Bool
  for +id: Nat
  for +s: M.World
  for +I: Inv3(s)
  Inv3(M.deliver_pick(a, b, id, s))

def inv3_deliver_pick(a, b, id, s, I):
  match a:
    case True{}:
      inv3_deliver_if(True{}, id, s, I)
    case False{}:
      inv3_deliver_tk(b, id, s, I)

law inv3_drop_if:
  for b: Bool
  for +id: Nat
  for +s: M.World
  for +I: Inv3(s)
  Inv3(M.drop_if(b, id, s))

def inv3_drop_if(b, id, s, I):
  match b:
    case False{}:
      I
    case True{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          I

law inv3_drop_tk:
  for b: Bool
  for +id: Nat
  for +s: M.World
  for +I: Inv3(s)
  Inv3(M.drop_tk(b, id, s))

def inv3_drop_tk(b, id, s, I):
  match b:
    case False{}:
      I
    case True{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          I

law inv3_drop_pick:
  for a: Bool
  for b: Bool
  for +id: Nat
  for +s: M.World
  for +I: Inv3(s)
  Inv3(M.drop_pick(a, b, id, s))

def inv3_drop_pick(a, b, id, s, I):
  match a:
    case True{}:
      inv3_drop_if(True{}, id, s, I)
    case False{}:
      inv3_drop_tk(b, id, s, I)

law inv3_step:
  for e: M.Ev
  for +s: M.World
  for +I: Inv3(s)
  Inv3(M.step(e, s))

def inv3_step(e, s, I):
  match e:
    case M.Send{fu, r}:
      match r:
        case M.Auto{}:
          match s:
            case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
              match g:
                case True{}:
                  I
                case False{}:
                  inv3_send_auto(fu, M.W{q, haf, htk, t, d, u, tt, rl, n, tk, i, False{}}, I)
        case M.AskFirst{}:
          match s:
            case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
              match g:
                case True{}:
                  I
                case False{}:
                  inv3_owned(fu, n, q, tt, u, rl, tk, I)
        case M.RouteDrop{}:
          match s:
            case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
              match g:
                case True{}:
                  I
                case False{}:
                  inv3_owned(fu, n, q, tt, u, rl, tk, I)
    case M.Idle{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          match g:
            case True{}:
              I
            case False{}:
              inv3_flush(M.W{q, haf, htk, t, d, u, tt, rl, n, tk, True{}, False{}}, I)
    case M.Busy{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          I
    case M.Exit{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          mk3(tt, [], u, rl, tk, p1(tt, q, u, rl, tk, I), k => {==})
    case M.TakeoverBegin{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          mk3(tt, M.keep_users(q, u), u, rl, True{}, p1(tt, q, u, rl, tk, I), k => all2_keep(q, u, rl))
    case M.TakeoverEnd{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          inv3_flush(M.W{List.append(&2, Nat, q, htk), haf, [], t, d, u, tt, rl, n, False{}, i, g}, mk3(tt, List.append(&2, Nat, q, htk), u, rl, False{}, p1(tt, q, u, rl, tk, I), k => Empty.absurd({all2(List.append(&2, Nat, q, htk), u, rl) == True{} : Bool}, ff_tt(k))))
    case M.UserDeliver{+id}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          inv3_deliver_pick(Bool.and(M.mem(id, haf), Bool.not(g)), Bool.and(M.mem(id, htk), Bool.not(g)), id, M.W{q, haf, htk, t, d, u, tt, rl, n, tk, i, g}, I)
    case M.UserDrop{+id}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          inv3_drop_pick(M.mem(id, haf), M.mem(id, htk), id, M.W{q, haf, htk, t, d, u, tt, rl, n, tk, i, g}, I)

law inv3_run:
  for +evs: +List<M.Ev>
  for +s: M.World
  for +I: Inv3(s)
  Inv3(M.run(evs, s))

def inv3_run(evs, s, I):
  match evs:
    case Nil{}:
      I
    case Con{e, rest}:
      inv3_run(rest, M.step(e, s), inv3_step(e, s, I))

law all2_all_in:
  for +xs: +List<Nat>
  for +u: +List<Nat>
  for +rl: +List<Nat>
  for +h: {all2(xs, u, rl) == True{} : Bool}
  {S.all_in(xs, List.append(&2, Nat, u, rl)) == True{} : Bool}

def all2_all_in(xs, u, rl, h):
  match xs:
    case Nil{}:
      {==}
    case Con{+x, rest}:
      and_intro(M.mem(x, List.append(&2, Nat, u, rl)), S.all_in(rest, List.append(&2, Nat, u, rl)), or_elim(M.mem(x, u), M.mem(x, rl), M.mem(x, List.append(&2, Nat, u, rl)), and_l(in2(x, u, rl), all2(rest, u, rl), h), k => mem_app_l(x, u, rl, k), k => mem_app_r(x, u, rl, k)), all2_all_in(rest, u, rl, and_r(in2(x, u, rl), all2(rest, u, rl), h)))

law m3_of:
  for +s: M.World
  for +I: Inv3(s)
  {S.all_in(S.tk_typed(s), List.append(&2, Nat, S.users(s), S.released(s))) == True{} : Bool}

def m3_of(s, I):
  match s:
    case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
      all2_all_in(tt, u, rl, p1(tt, q, u, rl, tk, I))

law m3_run:
  for +evs: +List<M.Ev>
  for +s: M.World
  for +I: Inv3(s)
  {S.all_in(S.tk_typed(M.run(evs, s)), List.append(&2, Nat, S.users(M.run(evs, s)), S.released(M.run(evs, s)))) == True{} : Bool}

def m3_run(evs, s, I):
  match evs:
    case Nil{}:
      m3_of(s, I)
    case Con{e, rest}:
      m3_run(rest, M.step(e, s), inv3_step(e, s, I))

def Laws.m3_takeover_isolates(evs):
  m3_run(evs, M.init(), inv3_init())


# ---- M1: nodup, disjoint and bounds, kept by every step ----

law nor_l:
  for e: Bool
  for m: Bool
  for +h: {Bool.not(Bool.or(e, m)) == True{} : Bool}
  {e == False{} : Bool}

def nor_l(e, m, h):
  match e:
    case False{}:
      {==}
    case True{}:
      Empty.absurd({True{} == False{} : Bool}, ff_tt(h))

law nor_r:
  for e: Bool
  for m: Bool
  for +h: {Bool.not(Bool.or(e, m)) == True{} : Bool}
  {Bool.not(m) == True{} : Bool}

def nor_r(e, m, h):
  match e:
    case False{}:
      h
    case True{}:
      Empty.absurd({Bool.not(m) == True{} : Bool}, ff_tt(h))

law nor_i:
  for e: Bool
  for m: Bool
  for +he: {e == False{} : Bool}
  for +hm: {Bool.not(m) == True{} : Bool}
  {Bool.not(Bool.or(e, m)) == True{} : Bool}

def nor_i(e, m, he, hm):
  %Equal.sym(Bool, e, False{}, he) : {Bool.not(Bool.or(_, m)) == True{} : Bool}
  hm

law f_app:
  for +x: Nat
  for +a: +List<Nat>
  for +b: +List<Nat>
  for +ha: {Bool.not(M.mem(x, a)) == True{} : Bool}
  for +hb: {Bool.not(M.mem(x, b)) == True{} : Bool}
  {Bool.not(M.mem(x, List.append(&2, Nat, a, b))) == True{} : Bool}

def f_app(x, a, b, ha, hb):
  match a:
    case Nil{}:
      hb
    case Con{+y, rest}:
      nor_i(Nat.is_eq(x, y), M.mem(x, List.append(&2, Nat, rest, b)), nor_l(Nat.is_eq(x, y), M.mem(x, rest), ha), f_app(x, rest, b, nor_r(Nat.is_eq(x, y), M.mem(x, rest), ha), hb))

law f_app_l:
  for +x: Nat
  for +a: +List<Nat>
  for +b: +List<Nat>
  for +h: {Bool.not(M.mem(x, List.append(&2, Nat, a, b))) == True{} : Bool}
  {Bool.not(M.mem(x, a)) == True{} : Bool}

def f_app_l(x, a, b, h):
  match a:
    case Nil{}:
      {==}
    case Con{+y, rest}:
      nor_i(Nat.is_eq(x, y), M.mem(x, rest), nor_l(Nat.is_eq(x, y), M.mem(x, List.append(&2, Nat, rest, b)), h), f_app_l(x, rest, b, nor_r(Nat.is_eq(x, y), M.mem(x, List.append(&2, Nat, rest, b)), h)))

law f_app_r:
  for +x: Nat
  for +a: +List<Nat>
  for +b: +List<Nat>
  for +h: {Bool.not(M.mem(x, List.append(&2, Nat, a, b))) == True{} : Bool}
  {Bool.not(M.mem(x, b)) == True{} : Bool}

def f_app_r(x, a, b, h):
  match a:
    case Nil{}:
      h
    case Con{+y, rest}:
      f_app_r(x, rest, b, nor_r(Nat.is_eq(x, y), M.mem(x, List.append(&2, Nat, rest, b)), h))

law eq_comm:
  for +x: Nat
  for +y: Nat
  {Nat.is_eq(x, y) == Nat.is_eq(y, x) : Bool}

def eq_comm(x, y):
  match x y:
    case 0n 0n:
      {==}
    case 0n 1n+yp:
      {==}
    case 1n+xp 0n:
      {==}
    case 1n+xp 1n+yp:
      eq_comm(xp, yp)

law z1:
  for +y: Nat
  for +z: Nat
  for +s: +List<Nat>
  for +h: {Bool.not(M.mem(z, y <> s)) == True{} : Bool}
  {Nat.is_eq(y, z) == False{} : Bool}

def z1(y, z, s, h):
  %eq_comm(z, y) : {_ == False{} : Bool}
  nor_l(Nat.is_eq(z, y), M.mem(z, s), h)

law d_cons_r1:
  for +a: +List<Nat>
  for +y: Nat
  for +s: +List<Nat>
  for +h: {S.disjoint(a, y <> s) == True{} : Bool}
  {Bool.not(M.mem(y, a)) == True{} : Bool}

def d_cons_r1(a, y, s, h):
  match a:
    case Nil{}:
      {==}
    case Con{+z, rest}:
      nor_i(Nat.is_eq(y, z), M.mem(y, rest), z1(y, z, s, and_l(Bool.not(M.mem(z, y <> s)), S.disjoint(rest, y <> s), h)), d_cons_r1(rest, y, s, and_r(Bool.not(M.mem(z, y <> s)), S.disjoint(rest, y <> s), h)))

law d_cons_r2:
  for +a: +List<Nat>
  for +y: Nat
  for +s: +List<Nat>
  for +h: {S.disjoint(a, y <> s) == True{} : Bool}
  {S.disjoint(a, s) == True{} : Bool}

def d_cons_r2(a, y, s, h):
  match a:
    case Nil{}:
      {==}
    case Con{+z, rest}:
      and_intro(Bool.not(M.mem(z, s)), S.disjoint(rest, s), nor_r(Nat.is_eq(z, y), M.mem(z, s), and_l(Bool.not(M.mem(z, y <> s)), S.disjoint(rest, y <> s), h)), d_cons_r2(rest, y, s, and_r(Bool.not(M.mem(z, y <> s)), S.disjoint(rest, y <> s), h)))

law d_sym:
  for +b: +List<Nat>
  for +a: +List<Nat>
  for +h: {S.disjoint(a, b) == True{} : Bool}
  {S.disjoint(b, a) == True{} : Bool}

def d_sym(b, a, h):
  match b:
    case Nil{}:
      {==}
    case Con{+y, s}:
      and_intro(Bool.not(M.mem(y, a)), S.disjoint(s, a), d_cons_r1(a, y, s, h), d_sym(s, a, d_cons_r2(a, y, s, h)))

law d_mem_eq:
  for +y: Nat
  for +z: Nat
  for +c: +List<Nat>
  for +h: {Bool.not(M.mem(z, c)) == True{} : Bool}
  for +k: {Nat.is_eq(y, z) == True{} : Bool}
  {Bool.not(M.mem(y, c)) == True{} : Bool}

def d_mem_eq(y, z, c, h, k):
  %Equal.sym(Nat, y, z, eq_sound(y, z, k)) : {Bool.not(M.mem(_, c)) == True{} : Bool}
  h

law d_mem:
  for +a: +List<Nat>
  for +y: Nat
  for +c: +List<Nat>
  for +h: {S.disjoint(a, c) == True{} : Bool}
  for +m: {M.mem(y, a) == True{} : Bool}
  {Bool.not(M.mem(y, c)) == True{} : Bool}

def d_mem(a, y, c, h, m):
  match a:
    case Nil{}:
      Empty.absurd({Bool.not(M.mem(y, c)) == True{} : Bool}, ff_tt(m))
    case Con{+z, rest}:
      or_elim(Nat.is_eq(y, z), M.mem(y, rest), Bool.not(M.mem(y, c)), m, k => d_mem_eq(y, z, c, and_l(Bool.not(M.mem(z, c)), S.disjoint(rest, c), h), k), k => d_mem(rest, y, c, and_r(Bool.not(M.mem(z, c)), S.disjoint(rest, c), h), k))

law d_app_l:
  for +a: +List<Nat>
  for +b: +List<Nat>
  for +c: +List<Nat>
  for +ha: {S.disjoint(a, c) == True{} : Bool}
  for +hb: {S.disjoint(b, c) == True{} : Bool}
  {S.disjoint(List.append(&2, Nat, a, b), c) == True{} : Bool}

def d_app_l(a, b, c, ha, hb):
  match a:
    case Nil{}:
      hb
    case Con{+x, rest}:
      and_intro(Bool.not(M.mem(x, c)), S.disjoint(List.append(&2, Nat, rest, b), c), and_l(Bool.not(M.mem(x, c)), S.disjoint(rest, c), ha), d_app_l(rest, b, c, and_r(Bool.not(M.mem(x, c)), S.disjoint(rest, c), ha), hb))

law d_sub:
  for +b: +List<Nat>
  for +a: +List<Nat>
  for +c: +List<Nat>
  for +hd: {S.disjoint(a, c) == True{} : Bool}
  for +hs: {S.all_in(b, a) == True{} : Bool}
  {S.disjoint(b, c) == True{} : Bool}

def d_sub(b, a, c, hd, hs):
  match b:
    case Nil{}:
      {==}
    case Con{+y, rest}:
      and_intro(Bool.not(M.mem(y, c)), S.disjoint(rest, c), d_mem(a, y, c, hd, and_l(M.mem(y, a), S.all_in(rest, a), hs)), d_sub(rest, a, c, hd, and_r(M.mem(y, a), S.all_in(rest, a), hs)))

law n_app:
  for +a: +List<Nat>
  for +b: +List<Nat>
  for +hn: {S.nodup(a) == True{} : Bool}
  for +hm: {S.nodup(b) == True{} : Bool}
  for +hd: {S.disjoint(a, b) == True{} : Bool}
  {S.nodup(List.append(&2, Nat, a, b)) == True{} : Bool}

def n_app(a, b, hn, hm, hd):
  match a:
    case Nil{}:
      hm
    case Con{+x, rest}:
      and_intro(Bool.not(M.mem(x, List.append(&2, Nat, rest, b))), S.nodup(List.append(&2, Nat, rest, b)), f_app(x, rest, b, and_l(Bool.not(M.mem(x, rest)), S.nodup(rest), hn), and_l(Bool.not(M.mem(x, b)), S.disjoint(rest, b), hd)), n_app(rest, b, and_r(Bool.not(M.mem(x, rest)), S.nodup(rest), hn), hm, and_r(Bool.not(M.mem(x, b)), S.disjoint(rest, b), hd)))

law n_snoc:
  for +a: +List<Nat>
  for +x: Nat
  for +hn: {S.nodup(a) == True{} : Bool}
  for +hf: {Bool.not(M.mem(x, a)) == True{} : Bool}
  {S.nodup(M.snoc(a, x)) == True{} : Bool}

def n_snoc(a, x, hn, hf):
  n_app(a, [x], hn, {==}, d_sym(a, [x], and_intro(Bool.not(M.mem(x, a)), True{}, hf, {==})))

law d_snoc_l:
  for +a: +List<Nat>
  for +x: Nat
  for +c: +List<Nat>
  for +ha: {S.disjoint(a, c) == True{} : Bool}
  for +hf: {Bool.not(M.mem(x, c)) == True{} : Bool}
  {S.disjoint(M.snoc(a, x), c) == True{} : Bool}

def d_snoc_l(a, x, c, ha, hf):
  d_app_l(a, [x], c, ha, and_intro(Bool.not(M.mem(x, c)), True{}, hf, {==}))

law f_sub:
  for +y: Nat
  for +r: +List<Nat>
  for +t: +List<Nat>
  for +hf: {Bool.not(M.mem(y, r)) == True{} : Bool}
  for +hs: {S.all_in(t, r) == True{} : Bool}
  {Bool.not(M.mem(y, t)) == True{} : Bool}

def f_sub(y, r, t, hf, hs):
  and_l(Bool.not(M.mem(y, t)), True{}, d_sym([y], t, d_sub(t, r, [y], d_sym(r, [y], and_intro(Bool.not(M.mem(y, r)), True{}, hf, {==})), hs)))

law sub_cons:
  for +t: +List<Nat>
  for +y: Nat
  for +r: +List<Nat>
  for +h: {S.all_in(t, r) == True{} : Bool}
  {S.all_in(M.remove_step(False{}, y, t), y <> r) == True{} : Bool}

def sub_cons(t, y, r, h):
  and_intro(M.mem(y, y <> r), S.all_in(t, y <> r), mem_hd(y, r), all_in_cons(t, y, r, h))

law sub_rs:
  for b: Bool
  for +y: Nat
  for +t: +List<Nat>
  for +r: +List<Nat>
  for +h: {S.all_in(t, r) == True{} : Bool}
  {S.all_in(M.remove_step(b, y, t), y <> r) == True{} : Bool}

def sub_rs(b, y, t, r, h):
  match b:
    case True{}:
      all_in_cons(t, y, r, h)
    case False{}:
      sub_cons(t, y, r, h)

law sub_remove:
  for +x: Nat
  for +a: +List<Nat>
  {S.all_in(M.remove(x, a), a) == True{} : Bool}

def sub_remove(x, a):
  match a:
    case Nil{}:
      {==}
    case Con{+y, rest}:
      sub_rs(Nat.is_eq(x, y), y, M.remove(x, rest), rest, sub_remove(x, rest))

law n_rs:
  for b: Bool
  for +y: Nat
  for +t: +List<Nat>
  for +r: +List<Nat>
  for +hf: {Bool.not(M.mem(y, r)) == True{} : Bool}
  for +hs: {S.all_in(t, r) == True{} : Bool}
  for +hn: {S.nodup(t) == True{} : Bool}
  {S.nodup(M.remove_step(b, y, t)) == True{} : Bool}

def n_rs(b, y, t, r, hf, hs, hn):
  match b:
    case True{}:
      hn
    case False{}:
      and_intro(Bool.not(M.mem(y, t)), S.nodup(t), f_sub(y, r, t, hf, hs), hn)

law n_remove:
  for +x: Nat
  for +a: +List<Nat>
  for +h: {S.nodup(a) == True{} : Bool}
  {S.nodup(M.remove(x, a)) == True{} : Bool}

def n_remove(x, a, h):
  match a:
    case Nil{}:
      {==}
    case Con{+y, rest}:
      n_rs(Nat.is_eq(x, y), y, M.remove(x, rest), rest, and_l(Bool.not(M.mem(y, rest)), S.nodup(rest), h), sub_remove(x, rest), n_remove(x, rest, and_r(Bool.not(M.mem(y, rest)), S.nodup(rest), h)))

law sub_ks:
  for b: Bool
  for +y: Nat
  for +t: +List<Nat>
  for +r: +List<Nat>
  for +h: {S.all_in(t, r) == True{} : Bool}
  {S.all_in(M.keep_step(b, y, t), y <> r) == True{} : Bool}

def sub_ks(b, y, t, r, h):
  match b:
    case True{}:
      and_intro(M.mem(y, y <> r), S.all_in(t, y <> r), mem_hd(y, r), all_in_cons(t, y, r, h))
    case False{}:
      all_in_cons(t, y, r, h)

law sub_ds:
  for b: Bool
  for +y: Nat
  for +t: +List<Nat>
  for +r: +List<Nat>
  for +h: {S.all_in(t, r) == True{} : Bool}
  {S.all_in(M.drop_step(b, y, t), y <> r) == True{} : Bool}

def sub_ds(b, y, t, r, h):
  match b:
    case True{}:
      all_in_cons(t, y, r, h)
    case False{}:
      and_intro(M.mem(y, y <> r), S.all_in(t, y <> r), mem_hd(y, r), all_in_cons(t, y, r, h))

law sub_keep:
  for +q: +List<Nat>
  for +u: +List<Nat>
  {S.all_in(M.keep_users(q, u), q) == True{} : Bool}

def sub_keep(q, u):
  match q:
    case Nil{}:
      {==}
    case Con{+x, rest}:
      sub_ks(M.mem(x, u), x, M.keep_users(rest, u), rest, sub_keep(rest, u))

law sub_drop:
  for +q: +List<Nat>
  for +u: +List<Nat>
  {S.all_in(M.drop_users(q, u), q) == True{} : Bool}

def sub_drop(q, u):
  match q:
    case Nil{}:
      {==}
    case Con{+x, rest}:
      sub_ds(M.mem(x, u), x, M.drop_users(rest, u), rest, sub_drop(rest, u))

law n_ks:
  for b: Bool
  for +y: Nat
  for +t: +List<Nat>
  for +r: +List<Nat>
  for +hf: {Bool.not(M.mem(y, r)) == True{} : Bool}
  for +hs: {S.all_in(t, r) == True{} : Bool}
  for +hn: {S.nodup(t) == True{} : Bool}
  {S.nodup(M.keep_step(b, y, t)) == True{} : Bool}

def n_ks(b, y, t, r, hf, hs, hn):
  match b:
    case True{}:
      and_intro(Bool.not(M.mem(y, t)), S.nodup(t), f_sub(y, r, t, hf, hs), hn)
    case False{}:
      hn

law n_ds:
  for b: Bool
  for +y: Nat
  for +t: +List<Nat>
  for +r: +List<Nat>
  for +hf: {Bool.not(M.mem(y, r)) == True{} : Bool}
  for +hs: {S.all_in(t, r) == True{} : Bool}
  for +hn: {S.nodup(t) == True{} : Bool}
  {S.nodup(M.drop_step(b, y, t)) == True{} : Bool}

def n_ds(b, y, t, r, hf, hs, hn):
  match b:
    case True{}:
      hn
    case False{}:
      and_intro(Bool.not(M.mem(y, t)), S.nodup(t), f_sub(y, r, t, hf, hs), hn)

law n_keep:
  for +q: +List<Nat>
  for +u: +List<Nat>
  for +h: {S.nodup(q) == True{} : Bool}
  {S.nodup(M.keep_users(q, u)) == True{} : Bool}

def n_keep(q, u, h):
  match q:
    case Nil{}:
      {==}
    case Con{+x, rest}:
      n_ks(M.mem(x, u), x, M.keep_users(rest, u), rest, and_l(Bool.not(M.mem(x, rest)), S.nodup(rest), h), sub_keep(rest, u), n_keep(rest, u, and_r(Bool.not(M.mem(x, rest)), S.nodup(rest), h)))

law n_drop:
  for +q: +List<Nat>
  for +u: +List<Nat>
  for +h: {S.nodup(q) == True{} : Bool}
  {S.nodup(M.drop_users(q, u)) == True{} : Bool}

def n_drop(q, u, h):
  match q:
    case Nil{}:
      {==}
    case Con{+x, rest}:
      n_ds(M.mem(x, u), x, M.drop_users(rest, u), rest, and_l(Bool.not(M.mem(x, rest)), S.nodup(rest), h), sub_drop(rest, u), n_drop(rest, u, and_r(Bool.not(M.mem(x, rest)), S.nodup(rest), h)))

law d_kd_b:
  for b: Bool
  for +x: Nat
  for +kr: +List<Nat>
  for +dr: +List<Nat>
  for +r: +List<Nat>
  for +hf: {Bool.not(M.mem(x, r)) == True{} : Bool}
  for +hk: {S.all_in(kr, r) == True{} : Bool}
  for +hd: {S.all_in(dr, r) == True{} : Bool}
  for +ih: {S.disjoint(kr, dr) == True{} : Bool}
  {S.disjoint(M.keep_step(b, x, kr), M.drop_step(b, x, dr)) == True{} : Bool}

def d_kd_b(b, x, kr, dr, r, hf, hk, hd, ih):
  match b:
    case True{}:
      and_intro(Bool.not(M.mem(x, dr)), S.disjoint(kr, dr), f_sub(x, r, dr, hf, hd), ih)
    case False{}:
      d_sym(kr, x <> dr, and_intro(Bool.not(M.mem(x, kr)), S.disjoint(dr, kr), f_sub(x, r, kr, hf, hk), d_sym(dr, kr, ih)))

law d_kd:
  for +q: +List<Nat>
  for +u: +List<Nat>
  for +h: {S.nodup(q) == True{} : Bool}
  {S.disjoint(M.keep_users(q, u), M.drop_users(q, u)) == True{} : Bool}

def d_kd(q, u, h):
  match q:
    case Nil{}:
      {==}
    case Con{+x, rest}:
      d_kd_b(M.mem(x, u), x, M.keep_users(rest, u), M.drop_users(rest, u), rest, and_l(Bool.not(M.mem(x, rest)), S.nodup(rest), h), sub_keep(rest, u), sub_drop(rest, u), d_kd(rest, u, and_r(Bool.not(M.mem(x, rest)), S.nodup(rest), h)))

def lt(y: Nat, n: Nat) -> Bool:
  match y n:
    case 0n 0n:
      False{}
    case 0n 1n+b:
      True{}
    case 1n+a 0n:
      False{}
    case 1n+a 1n+b:
      lt(a, b)

def bl(xs: +List<Nat>, +n: Nat) -> Bool:
  match xs:
    case Nil{}:
      True{}
    case Con{+x, rest}:
      Bool.and(lt(x, n), bl(rest, n))

law lt_succ:
  for +y: Nat
  for +n: Nat
  for +h: {lt(y, n) == True{} : Bool}
  {lt(y, 1n+n) == True{} : Bool}

def lt_succ(y, n, h):
  match y n:
    case 0n 0n:
      Empty.absurd({lt(0n, 1n) == True{} : Bool}, ff_tt(h))
    case 0n 1n+b:
      {==}
    case 1n+a 0n:
      Empty.absurd({lt(1n+a, 1n) == True{} : Bool}, ff_tt(h))
    case 1n+a 1n+b:
      lt_succ(a, b, h)

law lt_self:
  for +n: Nat
  {lt(n, 1n+n) == True{} : Bool}

def lt_self(n):
  match n:
    case 0n:
      {==}
    case 1n+p:
      lt_self(p)

law lt_ne:
  for +y: Nat
  for +n: Nat
  for +h: {lt(y, n) == True{} : Bool}
  {Nat.is_eq(y, n) == False{} : Bool}

def lt_ne(y, n, h):
  match y n:
    case 0n 0n:
      Empty.absurd({Nat.is_eq(0n, 0n) == False{} : Bool}, ff_tt(h))
    case 0n 1n+b:
      {==}
    case 1n+a 0n:
      Empty.absurd({Nat.is_eq(1n+a, 0n) == False{} : Bool}, ff_tt(h))
    case 1n+a 1n+b:
      lt_ne(a, b, h)

law bl_succ:
  for +xs: +List<Nat>
  for +n: Nat
  for +h: {bl(xs, n) == True{} : Bool}
  {bl(xs, 1n+n) == True{} : Bool}

def bl_succ(xs, n, h):
  match xs:
    case Nil{}:
      {==}
    case Con{+x, rest}:
      and_intro(lt(x, 1n+n), bl(rest, 1n+n), lt_succ(x, n, and_l(lt(x, n), bl(rest, n), h)), bl_succ(rest, n, and_r(lt(x, n), bl(rest, n), h)))

law bl_fresh1:
  for +n: Nat
  for +x: Nat
  for +h: {lt(x, n) == True{} : Bool}
  {Nat.is_eq(n, x) == False{} : Bool}

def bl_fresh1(n, x, h):
  %eq_comm(x, n) : {_ == False{} : Bool}
  lt_ne(x, n, h)

law bl_fresh:
  for +xs: +List<Nat>
  for +n: Nat
  for +h: {bl(xs, n) == True{} : Bool}
  {Bool.not(M.mem(n, xs)) == True{} : Bool}

def bl_fresh(xs, n, h):
  match xs:
    case Nil{}:
      {==}
    case Con{+x, rest}:
      nor_i(Nat.is_eq(n, x), M.mem(n, rest), bl_fresh1(n, x, and_l(lt(x, n), bl(rest, n), h)), bl_fresh(rest, n, and_r(lt(x, n), bl(rest, n), h)))

law bl_mem_eq:
  for +y: Nat
  for +z: Nat
  for +n: Nat
  for +h: {lt(z, n) == True{} : Bool}
  for +k: {Nat.is_eq(y, z) == True{} : Bool}
  {lt(y, n) == True{} : Bool}

def bl_mem_eq(y, z, n, h, k):
  %Equal.sym(Nat, y, z, eq_sound(y, z, k)) : {lt(_, n) == True{} : Bool}
  h

law bl_mem:
  for +xs: +List<Nat>
  for +y: Nat
  for +n: Nat
  for +h: {bl(xs, n) == True{} : Bool}
  for +m: {M.mem(y, xs) == True{} : Bool}
  {lt(y, n) == True{} : Bool}

def bl_mem(xs, y, n, h, m):
  match xs:
    case Nil{}:
      Empty.absurd({lt(y, n) == True{} : Bool}, ff_tt(m))
    case Con{+z, rest}:
      or_elim(Nat.is_eq(y, z), M.mem(y, rest), lt(y, n), m, k => bl_mem_eq(y, z, n, and_l(lt(z, n), bl(rest, n), h), k), k => bl_mem(rest, y, n, and_r(lt(z, n), bl(rest, n), h), k))

law bl_sub:
  for +b: +List<Nat>
  for +a: +List<Nat>
  for +n: Nat
  for +hb: {bl(a, n) == True{} : Bool}
  for +hs: {S.all_in(b, a) == True{} : Bool}
  {bl(b, n) == True{} : Bool}

def bl_sub(b, a, n, hb, hs):
  match b:
    case Nil{}:
      {==}
    case Con{+y, rest}:
      and_intro(lt(y, n), bl(rest, n), bl_mem(a, y, n, hb, and_l(M.mem(y, a), S.all_in(rest, a), hs)), bl_sub(rest, a, n, hb, and_r(M.mem(y, a), S.all_in(rest, a), hs)))

law bl_app:
  for +a: +List<Nat>
  for +b: +List<Nat>
  for +n: Nat
  for +ha: {bl(a, n) == True{} : Bool}
  for +hb: {bl(b, n) == True{} : Bool}
  {bl(List.append(&2, Nat, a, b), n) == True{} : Bool}

def bl_app(a, b, n, ha, hb):
  match a:
    case Nil{}:
      hb
    case Con{+x, rest}:
      and_intro(lt(x, n), bl(List.append(&2, Nat, rest, b), n), and_l(lt(x, n), bl(rest, n), ha), bl_app(rest, b, n, and_r(lt(x, n), bl(rest, n), ha), hb))

law bl_snoc:
  for +a: +List<Nat>
  for +x: Nat
  for +n: Nat
  for +ha: {bl(a, n) == True{} : Bool}
  for +hx: {lt(x, n) == True{} : Bool}
  {bl(M.snoc(a, x), n) == True{} : Bool}

def bl_snoc(a, x, n, ha, hx):
  bl_app(a, [x], n, ha, and_intro(lt(x, n), True{}, hx, {==}))

law f_rs:
  for b: Bool
  for +x: Nat
  for +y: Nat
  for +t: +List<Nat>
  for +hb: {b == Nat.is_eq(x, y) : Bool}
  for +ih: {Bool.not(M.mem(x, t)) == True{} : Bool}
  {Bool.not(M.mem(x, M.remove_step(b, y, t))) == True{} : Bool}

def f_rs(b, x, y, t, hb, ih):
  match b:
    case True{}:
      ih
    case False{}:
      nor_i(Nat.is_eq(x, y), M.mem(x, t), Equal.sym(Bool, False{}, Nat.is_eq(x, y), hb), ih)

law f_remove:
  for +x: Nat
  for +a: +List<Nat>
  for +h: {S.nodup(a) == True{} : Bool}
  {Bool.not(M.mem(x, M.remove(x, a))) == True{} : Bool}

def f_remove(x, a, h):
  match a:
    case Nil{}:
      {==}
    case Con{+y, rest}:
      f_rs(Nat.is_eq(x, y), x, y, M.remove(x, rest), {==}, f_remove(x, rest, and_r(Bool.not(M.mem(y, rest)), S.nodup(rest), h)))

law d_nil_r:
  for +a: +List<Nat>
  {S.disjoint(a, []) == True{} : Bool}

def d_nil_r(a):
  match a:
    case Nil{}:
      {==}
    case Con{+x, rest}:
      and_intro(Bool.not(M.mem(x, [])), S.disjoint(rest, []), {==}, d_nil_r(rest))

def inv18(+q: +List<Nat>, +h: +List<Nat>, +k: +List<Nat>, +t: +List<Nat>, +d: +List<Nat>, +n: Nat) -> Bool:
  bl(d, n)

def inv17(+q: +List<Nat>, +h: +List<Nat>, +k: +List<Nat>, +t: +List<Nat>, +d: +List<Nat>, +n: Nat) -> Bool:
  Bool.and(bl(t, n), inv18(q, h, k, t, d, n))

def inv16(+q: +List<Nat>, +h: +List<Nat>, +k: +List<Nat>, +t: +List<Nat>, +d: +List<Nat>, +n: Nat) -> Bool:
  Bool.and(bl(k, n), inv17(q, h, k, t, d, n))

def inv15(+q: +List<Nat>, +h: +List<Nat>, +k: +List<Nat>, +t: +List<Nat>, +d: +List<Nat>, +n: Nat) -> Bool:
  Bool.and(bl(h, n), inv16(q, h, k, t, d, n))

def inv14(+q: +List<Nat>, +h: +List<Nat>, +k: +List<Nat>, +t: +List<Nat>, +d: +List<Nat>, +n: Nat) -> Bool:
  Bool.and(bl(q, n), inv15(q, h, k, t, d, n))

def inv13(+q: +List<Nat>, +h: +List<Nat>, +k: +List<Nat>, +t: +List<Nat>, +d: +List<Nat>, +n: Nat) -> Bool:
  Bool.and(S.disjoint(t, d), inv14(q, h, k, t, d, n))

def inv12(+q: +List<Nat>, +h: +List<Nat>, +k: +List<Nat>, +t: +List<Nat>, +d: +List<Nat>, +n: Nat) -> Bool:
  Bool.and(S.disjoint(k, d), inv13(q, h, k, t, d, n))

def inv11(+q: +List<Nat>, +h: +List<Nat>, +k: +List<Nat>, +t: +List<Nat>, +d: +List<Nat>, +n: Nat) -> Bool:
  Bool.and(S.disjoint(k, t), inv12(q, h, k, t, d, n))

def inv10(+q: +List<Nat>, +h: +List<Nat>, +k: +List<Nat>, +t: +List<Nat>, +d: +List<Nat>, +n: Nat) -> Bool:
  Bool.and(S.disjoint(h, d), inv11(q, h, k, t, d, n))

def inv9(+q: +List<Nat>, +h: +List<Nat>, +k: +List<Nat>, +t: +List<Nat>, +d: +List<Nat>, +n: Nat) -> Bool:
  Bool.and(S.disjoint(h, t), inv10(q, h, k, t, d, n))

def inv8(+q: +List<Nat>, +h: +List<Nat>, +k: +List<Nat>, +t: +List<Nat>, +d: +List<Nat>, +n: Nat) -> Bool:
  Bool.and(S.disjoint(h, k), inv9(q, h, k, t, d, n))

def inv7(+q: +List<Nat>, +h: +List<Nat>, +k: +List<Nat>, +t: +List<Nat>, +d: +List<Nat>, +n: Nat) -> Bool:
  Bool.and(S.disjoint(q, d), inv8(q, h, k, t, d, n))

def inv6(+q: +List<Nat>, +h: +List<Nat>, +k: +List<Nat>, +t: +List<Nat>, +d: +List<Nat>, +n: Nat) -> Bool:
  Bool.and(S.disjoint(q, t), inv7(q, h, k, t, d, n))

def inv5(+q: +List<Nat>, +h: +List<Nat>, +k: +List<Nat>, +t: +List<Nat>, +d: +List<Nat>, +n: Nat) -> Bool:
  Bool.and(S.disjoint(q, k), inv6(q, h, k, t, d, n))

def inv4(+q: +List<Nat>, +h: +List<Nat>, +k: +List<Nat>, +t: +List<Nat>, +d: +List<Nat>, +n: Nat) -> Bool:
  Bool.and(S.disjoint(q, h), inv5(q, h, k, t, d, n))

def inv3(+q: +List<Nat>, +h: +List<Nat>, +k: +List<Nat>, +t: +List<Nat>, +d: +List<Nat>, +n: Nat) -> Bool:
  Bool.and(S.nodup(t), inv4(q, h, k, t, d, n))

def inv2(+q: +List<Nat>, +h: +List<Nat>, +k: +List<Nat>, +t: +List<Nat>, +d: +List<Nat>, +n: Nat) -> Bool:
  Bool.and(S.nodup(k), inv3(q, h, k, t, d, n))

def inv1(+q: +List<Nat>, +h: +List<Nat>, +k: +List<Nat>, +t: +List<Nat>, +d: +List<Nat>, +n: Nat) -> Bool:
  Bool.and(S.nodup(h), inv2(q, h, k, t, d, n))

def inv0(+q: +List<Nat>, +h: +List<Nat>, +k: +List<Nat>, +t: +List<Nat>, +d: +List<Nat>, +n: Nat) -> Bool:
  Bool.and(S.nodup(q), inv1(q, h, k, t, d, n))

def inv(+q: +List<Nat>, +h: +List<Nat>, +k: +List<Nat>, +t: +List<Nat>, +d: +List<Nat>, +n: Nat) -> Bool:
  inv0(q, h, k, t, d, n)

law pnq:
  for +q: +List<Nat>
  for +h: +List<Nat>
  for +k: +List<Nat>
  for +t: +List<Nat>
  for +d: +List<Nat>
  for +n: Nat
  for +I: {inv(q, h, k, t, d, n) == True{} : Bool}
  {S.nodup(q) == True{} : Bool}

def pnq(q, h, k, t, d, n, I):
  and_l(S.nodup(q), inv1(q, h, k, t, d, n), I)

law pnh:
  for +q: +List<Nat>
  for +h: +List<Nat>
  for +k: +List<Nat>
  for +t: +List<Nat>
  for +d: +List<Nat>
  for +n: Nat
  for +I: {inv(q, h, k, t, d, n) == True{} : Bool}
  {S.nodup(h) == True{} : Bool}

def pnh(q, h, k, t, d, n, I):
  and_l(S.nodup(h), inv2(q, h, k, t, d, n), and_r(S.nodup(q), inv1(q, h, k, t, d, n), I))

law pnk:
  for +q: +List<Nat>
  for +h: +List<Nat>
  for +k: +List<Nat>
  for +t: +List<Nat>
  for +d: +List<Nat>
  for +n: Nat
  for +I: {inv(q, h, k, t, d, n) == True{} : Bool}
  {S.nodup(k) == True{} : Bool}

def pnk(q, h, k, t, d, n, I):
  and_l(S.nodup(k), inv3(q, h, k, t, d, n), and_r(S.nodup(h), inv2(q, h, k, t, d, n), and_r(S.nodup(q), inv1(q, h, k, t, d, n), I)))

law pnt:
  for +q: +List<Nat>
  for +h: +List<Nat>
  for +k: +List<Nat>
  for +t: +List<Nat>
  for +d: +List<Nat>
  for +n: Nat
  for +I: {inv(q, h, k, t, d, n) == True{} : Bool}
  {S.nodup(t) == True{} : Bool}

def pnt(q, h, k, t, d, n, I):
  and_l(S.nodup(t), inv4(q, h, k, t, d, n), and_r(S.nodup(k), inv3(q, h, k, t, d, n), and_r(S.nodup(h), inv2(q, h, k, t, d, n), and_r(S.nodup(q), inv1(q, h, k, t, d, n), I))))

law pdqh:
  for +q: +List<Nat>
  for +h: +List<Nat>
  for +k: +List<Nat>
  for +t: +List<Nat>
  for +d: +List<Nat>
  for +n: Nat
  for +I: {inv(q, h, k, t, d, n) == True{} : Bool}
  {S.disjoint(q, h) == True{} : Bool}

def pdqh(q, h, k, t, d, n, I):
  and_l(S.disjoint(q, h), inv5(q, h, k, t, d, n), and_r(S.nodup(t), inv4(q, h, k, t, d, n), and_r(S.nodup(k), inv3(q, h, k, t, d, n), and_r(S.nodup(h), inv2(q, h, k, t, d, n), and_r(S.nodup(q), inv1(q, h, k, t, d, n), I)))))

law pdqk:
  for +q: +List<Nat>
  for +h: +List<Nat>
  for +k: +List<Nat>
  for +t: +List<Nat>
  for +d: +List<Nat>
  for +n: Nat
  for +I: {inv(q, h, k, t, d, n) == True{} : Bool}
  {S.disjoint(q, k) == True{} : Bool}

def pdqk(q, h, k, t, d, n, I):
  and_l(S.disjoint(q, k), inv6(q, h, k, t, d, n), and_r(S.disjoint(q, h), inv5(q, h, k, t, d, n), and_r(S.nodup(t), inv4(q, h, k, t, d, n), and_r(S.nodup(k), inv3(q, h, k, t, d, n), and_r(S.nodup(h), inv2(q, h, k, t, d, n), and_r(S.nodup(q), inv1(q, h, k, t, d, n), I))))))

law pdqt:
  for +q: +List<Nat>
  for +h: +List<Nat>
  for +k: +List<Nat>
  for +t: +List<Nat>
  for +d: +List<Nat>
  for +n: Nat
  for +I: {inv(q, h, k, t, d, n) == True{} : Bool}
  {S.disjoint(q, t) == True{} : Bool}

def pdqt(q, h, k, t, d, n, I):
  and_l(S.disjoint(q, t), inv7(q, h, k, t, d, n), and_r(S.disjoint(q, k), inv6(q, h, k, t, d, n), and_r(S.disjoint(q, h), inv5(q, h, k, t, d, n), and_r(S.nodup(t), inv4(q, h, k, t, d, n), and_r(S.nodup(k), inv3(q, h, k, t, d, n), and_r(S.nodup(h), inv2(q, h, k, t, d, n), and_r(S.nodup(q), inv1(q, h, k, t, d, n), I)))))))

law pdqd:
  for +q: +List<Nat>
  for +h: +List<Nat>
  for +k: +List<Nat>
  for +t: +List<Nat>
  for +d: +List<Nat>
  for +n: Nat
  for +I: {inv(q, h, k, t, d, n) == True{} : Bool}
  {S.disjoint(q, d) == True{} : Bool}

def pdqd(q, h, k, t, d, n, I):
  and_l(S.disjoint(q, d), inv8(q, h, k, t, d, n), and_r(S.disjoint(q, t), inv7(q, h, k, t, d, n), and_r(S.disjoint(q, k), inv6(q, h, k, t, d, n), and_r(S.disjoint(q, h), inv5(q, h, k, t, d, n), and_r(S.nodup(t), inv4(q, h, k, t, d, n), and_r(S.nodup(k), inv3(q, h, k, t, d, n), and_r(S.nodup(h), inv2(q, h, k, t, d, n), and_r(S.nodup(q), inv1(q, h, k, t, d, n), I))))))))

law pdhk:
  for +q: +List<Nat>
  for +h: +List<Nat>
  for +k: +List<Nat>
  for +t: +List<Nat>
  for +d: +List<Nat>
  for +n: Nat
  for +I: {inv(q, h, k, t, d, n) == True{} : Bool}
  {S.disjoint(h, k) == True{} : Bool}

def pdhk(q, h, k, t, d, n, I):
  and_l(S.disjoint(h, k), inv9(q, h, k, t, d, n), and_r(S.disjoint(q, d), inv8(q, h, k, t, d, n), and_r(S.disjoint(q, t), inv7(q, h, k, t, d, n), and_r(S.disjoint(q, k), inv6(q, h, k, t, d, n), and_r(S.disjoint(q, h), inv5(q, h, k, t, d, n), and_r(S.nodup(t), inv4(q, h, k, t, d, n), and_r(S.nodup(k), inv3(q, h, k, t, d, n), and_r(S.nodup(h), inv2(q, h, k, t, d, n), and_r(S.nodup(q), inv1(q, h, k, t, d, n), I)))))))))

law pdht:
  for +q: +List<Nat>
  for +h: +List<Nat>
  for +k: +List<Nat>
  for +t: +List<Nat>
  for +d: +List<Nat>
  for +n: Nat
  for +I: {inv(q, h, k, t, d, n) == True{} : Bool}
  {S.disjoint(h, t) == True{} : Bool}

def pdht(q, h, k, t, d, n, I):
  and_l(S.disjoint(h, t), inv10(q, h, k, t, d, n), and_r(S.disjoint(h, k), inv9(q, h, k, t, d, n), and_r(S.disjoint(q, d), inv8(q, h, k, t, d, n), and_r(S.disjoint(q, t), inv7(q, h, k, t, d, n), and_r(S.disjoint(q, k), inv6(q, h, k, t, d, n), and_r(S.disjoint(q, h), inv5(q, h, k, t, d, n), and_r(S.nodup(t), inv4(q, h, k, t, d, n), and_r(S.nodup(k), inv3(q, h, k, t, d, n), and_r(S.nodup(h), inv2(q, h, k, t, d, n), and_r(S.nodup(q), inv1(q, h, k, t, d, n), I))))))))))

law pdhd:
  for +q: +List<Nat>
  for +h: +List<Nat>
  for +k: +List<Nat>
  for +t: +List<Nat>
  for +d: +List<Nat>
  for +n: Nat
  for +I: {inv(q, h, k, t, d, n) == True{} : Bool}
  {S.disjoint(h, d) == True{} : Bool}

def pdhd(q, h, k, t, d, n, I):
  and_l(S.disjoint(h, d), inv11(q, h, k, t, d, n), and_r(S.disjoint(h, t), inv10(q, h, k, t, d, n), and_r(S.disjoint(h, k), inv9(q, h, k, t, d, n), and_r(S.disjoint(q, d), inv8(q, h, k, t, d, n), and_r(S.disjoint(q, t), inv7(q, h, k, t, d, n), and_r(S.disjoint(q, k), inv6(q, h, k, t, d, n), and_r(S.disjoint(q, h), inv5(q, h, k, t, d, n), and_r(S.nodup(t), inv4(q, h, k, t, d, n), and_r(S.nodup(k), inv3(q, h, k, t, d, n), and_r(S.nodup(h), inv2(q, h, k, t, d, n), and_r(S.nodup(q), inv1(q, h, k, t, d, n), I)))))))))))

law pdkt:
  for +q: +List<Nat>
  for +h: +List<Nat>
  for +k: +List<Nat>
  for +t: +List<Nat>
  for +d: +List<Nat>
  for +n: Nat
  for +I: {inv(q, h, k, t, d, n) == True{} : Bool}
  {S.disjoint(k, t) == True{} : Bool}

def pdkt(q, h, k, t, d, n, I):
  and_l(S.disjoint(k, t), inv12(q, h, k, t, d, n), and_r(S.disjoint(h, d), inv11(q, h, k, t, d, n), and_r(S.disjoint(h, t), inv10(q, h, k, t, d, n), and_r(S.disjoint(h, k), inv9(q, h, k, t, d, n), and_r(S.disjoint(q, d), inv8(q, h, k, t, d, n), and_r(S.disjoint(q, t), inv7(q, h, k, t, d, n), and_r(S.disjoint(q, k), inv6(q, h, k, t, d, n), and_r(S.disjoint(q, h), inv5(q, h, k, t, d, n), and_r(S.nodup(t), inv4(q, h, k, t, d, n), and_r(S.nodup(k), inv3(q, h, k, t, d, n), and_r(S.nodup(h), inv2(q, h, k, t, d, n), and_r(S.nodup(q), inv1(q, h, k, t, d, n), I))))))))))))

law pdkd:
  for +q: +List<Nat>
  for +h: +List<Nat>
  for +k: +List<Nat>
  for +t: +List<Nat>
  for +d: +List<Nat>
  for +n: Nat
  for +I: {inv(q, h, k, t, d, n) == True{} : Bool}
  {S.disjoint(k, d) == True{} : Bool}

def pdkd(q, h, k, t, d, n, I):
  and_l(S.disjoint(k, d), inv13(q, h, k, t, d, n), and_r(S.disjoint(k, t), inv12(q, h, k, t, d, n), and_r(S.disjoint(h, d), inv11(q, h, k, t, d, n), and_r(S.disjoint(h, t), inv10(q, h, k, t, d, n), and_r(S.disjoint(h, k), inv9(q, h, k, t, d, n), and_r(S.disjoint(q, d), inv8(q, h, k, t, d, n), and_r(S.disjoint(q, t), inv7(q, h, k, t, d, n), and_r(S.disjoint(q, k), inv6(q, h, k, t, d, n), and_r(S.disjoint(q, h), inv5(q, h, k, t, d, n), and_r(S.nodup(t), inv4(q, h, k, t, d, n), and_r(S.nodup(k), inv3(q, h, k, t, d, n), and_r(S.nodup(h), inv2(q, h, k, t, d, n), and_r(S.nodup(q), inv1(q, h, k, t, d, n), I)))))))))))))

law pdtd:
  for +q: +List<Nat>
  for +h: +List<Nat>
  for +k: +List<Nat>
  for +t: +List<Nat>
  for +d: +List<Nat>
  for +n: Nat
  for +I: {inv(q, h, k, t, d, n) == True{} : Bool}
  {S.disjoint(t, d) == True{} : Bool}

def pdtd(q, h, k, t, d, n, I):
  and_l(S.disjoint(t, d), inv14(q, h, k, t, d, n), and_r(S.disjoint(k, d), inv13(q, h, k, t, d, n), and_r(S.disjoint(k, t), inv12(q, h, k, t, d, n), and_r(S.disjoint(h, d), inv11(q, h, k, t, d, n), and_r(S.disjoint(h, t), inv10(q, h, k, t, d, n), and_r(S.disjoint(h, k), inv9(q, h, k, t, d, n), and_r(S.disjoint(q, d), inv8(q, h, k, t, d, n), and_r(S.disjoint(q, t), inv7(q, h, k, t, d, n), and_r(S.disjoint(q, k), inv6(q, h, k, t, d, n), and_r(S.disjoint(q, h), inv5(q, h, k, t, d, n), and_r(S.nodup(t), inv4(q, h, k, t, d, n), and_r(S.nodup(k), inv3(q, h, k, t, d, n), and_r(S.nodup(h), inv2(q, h, k, t, d, n), and_r(S.nodup(q), inv1(q, h, k, t, d, n), I))))))))))))))

law pbq:
  for +q: +List<Nat>
  for +h: +List<Nat>
  for +k: +List<Nat>
  for +t: +List<Nat>
  for +d: +List<Nat>
  for +n: Nat
  for +I: {inv(q, h, k, t, d, n) == True{} : Bool}
  {bl(q, n) == True{} : Bool}

def pbq(q, h, k, t, d, n, I):
  and_l(bl(q, n), inv15(q, h, k, t, d, n), and_r(S.disjoint(t, d), inv14(q, h, k, t, d, n), and_r(S.disjoint(k, d), inv13(q, h, k, t, d, n), and_r(S.disjoint(k, t), inv12(q, h, k, t, d, n), and_r(S.disjoint(h, d), inv11(q, h, k, t, d, n), and_r(S.disjoint(h, t), inv10(q, h, k, t, d, n), and_r(S.disjoint(h, k), inv9(q, h, k, t, d, n), and_r(S.disjoint(q, d), inv8(q, h, k, t, d, n), and_r(S.disjoint(q, t), inv7(q, h, k, t, d, n), and_r(S.disjoint(q, k), inv6(q, h, k, t, d, n), and_r(S.disjoint(q, h), inv5(q, h, k, t, d, n), and_r(S.nodup(t), inv4(q, h, k, t, d, n), and_r(S.nodup(k), inv3(q, h, k, t, d, n), and_r(S.nodup(h), inv2(q, h, k, t, d, n), and_r(S.nodup(q), inv1(q, h, k, t, d, n), I)))))))))))))))

law pbh:
  for +q: +List<Nat>
  for +h: +List<Nat>
  for +k: +List<Nat>
  for +t: +List<Nat>
  for +d: +List<Nat>
  for +n: Nat
  for +I: {inv(q, h, k, t, d, n) == True{} : Bool}
  {bl(h, n) == True{} : Bool}

def pbh(q, h, k, t, d, n, I):
  and_l(bl(h, n), inv16(q, h, k, t, d, n), and_r(bl(q, n), inv15(q, h, k, t, d, n), and_r(S.disjoint(t, d), inv14(q, h, k, t, d, n), and_r(S.disjoint(k, d), inv13(q, h, k, t, d, n), and_r(S.disjoint(k, t), inv12(q, h, k, t, d, n), and_r(S.disjoint(h, d), inv11(q, h, k, t, d, n), and_r(S.disjoint(h, t), inv10(q, h, k, t, d, n), and_r(S.disjoint(h, k), inv9(q, h, k, t, d, n), and_r(S.disjoint(q, d), inv8(q, h, k, t, d, n), and_r(S.disjoint(q, t), inv7(q, h, k, t, d, n), and_r(S.disjoint(q, k), inv6(q, h, k, t, d, n), and_r(S.disjoint(q, h), inv5(q, h, k, t, d, n), and_r(S.nodup(t), inv4(q, h, k, t, d, n), and_r(S.nodup(k), inv3(q, h, k, t, d, n), and_r(S.nodup(h), inv2(q, h, k, t, d, n), and_r(S.nodup(q), inv1(q, h, k, t, d, n), I))))))))))))))))

law pbk:
  for +q: +List<Nat>
  for +h: +List<Nat>
  for +k: +List<Nat>
  for +t: +List<Nat>
  for +d: +List<Nat>
  for +n: Nat
  for +I: {inv(q, h, k, t, d, n) == True{} : Bool}
  {bl(k, n) == True{} : Bool}

def pbk(q, h, k, t, d, n, I):
  and_l(bl(k, n), inv17(q, h, k, t, d, n), and_r(bl(h, n), inv16(q, h, k, t, d, n), and_r(bl(q, n), inv15(q, h, k, t, d, n), and_r(S.disjoint(t, d), inv14(q, h, k, t, d, n), and_r(S.disjoint(k, d), inv13(q, h, k, t, d, n), and_r(S.disjoint(k, t), inv12(q, h, k, t, d, n), and_r(S.disjoint(h, d), inv11(q, h, k, t, d, n), and_r(S.disjoint(h, t), inv10(q, h, k, t, d, n), and_r(S.disjoint(h, k), inv9(q, h, k, t, d, n), and_r(S.disjoint(q, d), inv8(q, h, k, t, d, n), and_r(S.disjoint(q, t), inv7(q, h, k, t, d, n), and_r(S.disjoint(q, k), inv6(q, h, k, t, d, n), and_r(S.disjoint(q, h), inv5(q, h, k, t, d, n), and_r(S.nodup(t), inv4(q, h, k, t, d, n), and_r(S.nodup(k), inv3(q, h, k, t, d, n), and_r(S.nodup(h), inv2(q, h, k, t, d, n), and_r(S.nodup(q), inv1(q, h, k, t, d, n), I)))))))))))))))))

law pbt:
  for +q: +List<Nat>
  for +h: +List<Nat>
  for +k: +List<Nat>
  for +t: +List<Nat>
  for +d: +List<Nat>
  for +n: Nat
  for +I: {inv(q, h, k, t, d, n) == True{} : Bool}
  {bl(t, n) == True{} : Bool}

def pbt(q, h, k, t, d, n, I):
  and_l(bl(t, n), inv18(q, h, k, t, d, n), and_r(bl(k, n), inv17(q, h, k, t, d, n), and_r(bl(h, n), inv16(q, h, k, t, d, n), and_r(bl(q, n), inv15(q, h, k, t, d, n), and_r(S.disjoint(t, d), inv14(q, h, k, t, d, n), and_r(S.disjoint(k, d), inv13(q, h, k, t, d, n), and_r(S.disjoint(k, t), inv12(q, h, k, t, d, n), and_r(S.disjoint(h, d), inv11(q, h, k, t, d, n), and_r(S.disjoint(h, t), inv10(q, h, k, t, d, n), and_r(S.disjoint(h, k), inv9(q, h, k, t, d, n), and_r(S.disjoint(q, d), inv8(q, h, k, t, d, n), and_r(S.disjoint(q, t), inv7(q, h, k, t, d, n), and_r(S.disjoint(q, k), inv6(q, h, k, t, d, n), and_r(S.disjoint(q, h), inv5(q, h, k, t, d, n), and_r(S.nodup(t), inv4(q, h, k, t, d, n), and_r(S.nodup(k), inv3(q, h, k, t, d, n), and_r(S.nodup(h), inv2(q, h, k, t, d, n), and_r(S.nodup(q), inv1(q, h, k, t, d, n), I))))))))))))))))))

law pbd:
  for +q: +List<Nat>
  for +h: +List<Nat>
  for +k: +List<Nat>
  for +t: +List<Nat>
  for +d: +List<Nat>
  for +n: Nat
  for +I: {inv(q, h, k, t, d, n) == True{} : Bool}
  {bl(d, n) == True{} : Bool}

def pbd(q, h, k, t, d, n, I):
  and_r(bl(t, n), inv18(q, h, k, t, d, n), and_r(bl(k, n), inv17(q, h, k, t, d, n), and_r(bl(h, n), inv16(q, h, k, t, d, n), and_r(bl(q, n), inv15(q, h, k, t, d, n), and_r(S.disjoint(t, d), inv14(q, h, k, t, d, n), and_r(S.disjoint(k, d), inv13(q, h, k, t, d, n), and_r(S.disjoint(k, t), inv12(q, h, k, t, d, n), and_r(S.disjoint(h, d), inv11(q, h, k, t, d, n), and_r(S.disjoint(h, t), inv10(q, h, k, t, d, n), and_r(S.disjoint(h, k), inv9(q, h, k, t, d, n), and_r(S.disjoint(q, d), inv8(q, h, k, t, d, n), and_r(S.disjoint(q, t), inv7(q, h, k, t, d, n), and_r(S.disjoint(q, k), inv6(q, h, k, t, d, n), and_r(S.disjoint(q, h), inv5(q, h, k, t, d, n), and_r(S.nodup(t), inv4(q, h, k, t, d, n), and_r(S.nodup(k), inv3(q, h, k, t, d, n), and_r(S.nodup(h), inv2(q, h, k, t, d, n), and_r(S.nodup(q), inv1(q, h, k, t, d, n), I))))))))))))))))))

law mk_inv:
  for +q: +List<Nat>
  for +h: +List<Nat>
  for +k: +List<Nat>
  for +t: +List<Nat>
  for +d: +List<Nat>
  for +n: Nat
  for +e0: {S.nodup(q) == True{} : Bool}
  for +e1: {S.nodup(h) == True{} : Bool}
  for +e2: {S.nodup(k) == True{} : Bool}
  for +e3: {S.nodup(t) == True{} : Bool}
  for +e4: {S.disjoint(q, h) == True{} : Bool}
  for +e5: {S.disjoint(q, k) == True{} : Bool}
  for +e6: {S.disjoint(q, t) == True{} : Bool}
  for +e7: {S.disjoint(q, d) == True{} : Bool}
  for +e8: {S.disjoint(h, k) == True{} : Bool}
  for +e9: {S.disjoint(h, t) == True{} : Bool}
  for +e10: {S.disjoint(h, d) == True{} : Bool}
  for +e11: {S.disjoint(k, t) == True{} : Bool}
  for +e12: {S.disjoint(k, d) == True{} : Bool}
  for +e13: {S.disjoint(t, d) == True{} : Bool}
  for +e14: {bl(q, n) == True{} : Bool}
  for +e15: {bl(h, n) == True{} : Bool}
  for +e16: {bl(k, n) == True{} : Bool}
  for +e17: {bl(t, n) == True{} : Bool}
  for +e18: {bl(d, n) == True{} : Bool}
  {inv(q, h, k, t, d, n) == True{} : Bool}

def mk_inv(q, h, k, t, d, n, e0, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11, e12, e13, e14, e15, e16, e17, e18):
  and_intro(S.nodup(q), inv1(q, h, k, t, d, n), e0, and_intro(S.nodup(h), inv2(q, h, k, t, d, n), e1, and_intro(S.nodup(k), inv3(q, h, k, t, d, n), e2, and_intro(S.nodup(t), inv4(q, h, k, t, d, n), e3, and_intro(S.disjoint(q, h), inv5(q, h, k, t, d, n), e4, and_intro(S.disjoint(q, k), inv6(q, h, k, t, d, n), e5, and_intro(S.disjoint(q, t), inv7(q, h, k, t, d, n), e6, and_intro(S.disjoint(q, d), inv8(q, h, k, t, d, n), e7, and_intro(S.disjoint(h, k), inv9(q, h, k, t, d, n), e8, and_intro(S.disjoint(h, t), inv10(q, h, k, t, d, n), e9, and_intro(S.disjoint(h, d), inv11(q, h, k, t, d, n), e10, and_intro(S.disjoint(k, t), inv12(q, h, k, t, d, n), e11, and_intro(S.disjoint(k, d), inv13(q, h, k, t, d, n), e12, and_intro(S.disjoint(t, d), inv14(q, h, k, t, d, n), e13, and_intro(bl(q, n), inv15(q, h, k, t, d, n), e14, and_intro(bl(h, n), inv16(q, h, k, t, d, n), e15, and_intro(bl(k, n), inv17(q, h, k, t, d, n), e16, and_intro(bl(t, n), inv18(q, h, k, t, d, n), e17, e18))))))))))))))))))
law frame_send_q:
  for +q: +List<Nat>
  for +h: +List<Nat>
  for +k: +List<Nat>
  for +t: +List<Nat>
  for +d: +List<Nat>
  for +n: Nat
  for +I: {inv(q, h, k, t, d, n) == True{} : Bool}
  {inv(M.snoc(q, n), h, k, t, d, 1n+n) == True{} : Bool}

def frame_send_q(q, h, k, t, d, n, I):
  mk_inv(M.snoc(q, n), h, k, t, d, 1n+n, n_snoc(q, n, pnq(q, h, k, t, d, n, I), bl_fresh(q, n, pbq(q, h, k, t, d, n, I))), pnh(q, h, k, t, d, n, I), pnk(q, h, k, t, d, n, I), pnt(q, h, k, t, d, n, I), d_snoc_l(q, n, h, pdqh(q, h, k, t, d, n, I), bl_fresh(h, n, pbh(q, h, k, t, d, n, I))), d_snoc_l(q, n, k, pdqk(q, h, k, t, d, n, I), bl_fresh(k, n, pbk(q, h, k, t, d, n, I))), d_snoc_l(q, n, t, pdqt(q, h, k, t, d, n, I), bl_fresh(t, n, pbt(q, h, k, t, d, n, I))), d_snoc_l(q, n, d, pdqd(q, h, k, t, d, n, I), bl_fresh(d, n, pbd(q, h, k, t, d, n, I))), pdhk(q, h, k, t, d, n, I), pdht(q, h, k, t, d, n, I), pdhd(q, h, k, t, d, n, I), pdkt(q, h, k, t, d, n, I), pdkd(q, h, k, t, d, n, I), pdtd(q, h, k, t, d, n, I), bl_snoc(q, n, 1n+n, bl_succ(q, n, pbq(q, h, k, t, d, n, I)), lt_self(n)), bl_succ(h, n, pbh(q, h, k, t, d, n, I)), bl_succ(k, n, pbk(q, h, k, t, d, n, I)), bl_succ(t, n, pbt(q, h, k, t, d, n, I)), bl_succ(d, n, pbd(q, h, k, t, d, n, I)))


law frame_send_h:
  for +q: +List<Nat>
  for +h: +List<Nat>
  for +k: +List<Nat>
  for +t: +List<Nat>
  for +d: +List<Nat>
  for +n: Nat
  for +I: {inv(q, h, k, t, d, n) == True{} : Bool}
  {inv(q, M.snoc(h, n), k, t, d, 1n+n) == True{} : Bool}

def frame_send_h(q, h, k, t, d, n, I):
  mk_inv(q, M.snoc(h, n), k, t, d, 1n+n, pnq(q, h, k, t, d, n, I), n_snoc(h, n, pnh(q, h, k, t, d, n, I), bl_fresh(h, n, pbh(q, h, k, t, d, n, I))), pnk(q, h, k, t, d, n, I), pnt(q, h, k, t, d, n, I), d_sym(q, M.snoc(h, n), d_snoc_l(h, n, q, d_sym(h, q, pdqh(q, h, k, t, d, n, I)), bl_fresh(q, n, pbq(q, h, k, t, d, n, I)))), pdqk(q, h, k, t, d, n, I), pdqt(q, h, k, t, d, n, I), pdqd(q, h, k, t, d, n, I), d_snoc_l(h, n, k, pdhk(q, h, k, t, d, n, I), bl_fresh(k, n, pbk(q, h, k, t, d, n, I))), d_snoc_l(h, n, t, pdht(q, h, k, t, d, n, I), bl_fresh(t, n, pbt(q, h, k, t, d, n, I))), d_snoc_l(h, n, d, pdhd(q, h, k, t, d, n, I), bl_fresh(d, n, pbd(q, h, k, t, d, n, I))), pdkt(q, h, k, t, d, n, I), pdkd(q, h, k, t, d, n, I), pdtd(q, h, k, t, d, n, I), bl_succ(q, n, pbq(q, h, k, t, d, n, I)), bl_snoc(h, n, 1n+n, bl_succ(h, n, pbh(q, h, k, t, d, n, I)), lt_self(n)), bl_succ(k, n, pbk(q, h, k, t, d, n, I)), bl_succ(t, n, pbt(q, h, k, t, d, n, I)), bl_succ(d, n, pbd(q, h, k, t, d, n, I)))


law frame_send_k:
  for +q: +List<Nat>
  for +h: +List<Nat>
  for +k: +List<Nat>
  for +t: +List<Nat>
  for +d: +List<Nat>
  for +n: Nat
  for +I: {inv(q, h, k, t, d, n) == True{} : Bool}
  {inv(q, h, M.snoc(k, n), t, d, 1n+n) == True{} : Bool}

def frame_send_k(q, h, k, t, d, n, I):
  mk_inv(q, h, M.snoc(k, n), t, d, 1n+n, pnq(q, h, k, t, d, n, I), pnh(q, h, k, t, d, n, I), n_snoc(k, n, pnk(q, h, k, t, d, n, I), bl_fresh(k, n, pbk(q, h, k, t, d, n, I))), pnt(q, h, k, t, d, n, I), pdqh(q, h, k, t, d, n, I), d_sym(q, M.snoc(k, n), d_snoc_l(k, n, q, d_sym(k, q, pdqk(q, h, k, t, d, n, I)), bl_fresh(q, n, pbq(q, h, k, t, d, n, I)))), pdqt(q, h, k, t, d, n, I), pdqd(q, h, k, t, d, n, I), d_sym(h, M.snoc(k, n), d_snoc_l(k, n, h, d_sym(k, h, pdhk(q, h, k, t, d, n, I)), bl_fresh(h, n, pbh(q, h, k, t, d, n, I)))), pdht(q, h, k, t, d, n, I), pdhd(q, h, k, t, d, n, I), d_snoc_l(k, n, t, pdkt(q, h, k, t, d, n, I), bl_fresh(t, n, pbt(q, h, k, t, d, n, I))), d_snoc_l(k, n, d, pdkd(q, h, k, t, d, n, I), bl_fresh(d, n, pbd(q, h, k, t, d, n, I))), pdtd(q, h, k, t, d, n, I), bl_succ(q, n, pbq(q, h, k, t, d, n, I)), bl_succ(h, n, pbh(q, h, k, t, d, n, I)), bl_snoc(k, n, 1n+n, bl_succ(k, n, pbk(q, h, k, t, d, n, I)), lt_self(n)), bl_succ(t, n, pbt(q, h, k, t, d, n, I)), bl_succ(d, n, pbd(q, h, k, t, d, n, I)))


law frame_send_d:
  for +q: +List<Nat>
  for +h: +List<Nat>
  for +k: +List<Nat>
  for +t: +List<Nat>
  for +d: +List<Nat>
  for +n: Nat
  for +I: {inv(q, h, k, t, d, n) == True{} : Bool}
  {inv(q, h, k, t, M.snoc(d, n), 1n+n) == True{} : Bool}

def frame_send_d(q, h, k, t, d, n, I):
  mk_inv(q, h, k, t, M.snoc(d, n), 1n+n, pnq(q, h, k, t, d, n, I), pnh(q, h, k, t, d, n, I), pnk(q, h, k, t, d, n, I), pnt(q, h, k, t, d, n, I), pdqh(q, h, k, t, d, n, I), pdqk(q, h, k, t, d, n, I), pdqt(q, h, k, t, d, n, I), d_sym(q, M.snoc(d, n), d_snoc_l(d, n, q, d_sym(d, q, pdqd(q, h, k, t, d, n, I)), bl_fresh(q, n, pbq(q, h, k, t, d, n, I)))), pdhk(q, h, k, t, d, n, I), pdht(q, h, k, t, d, n, I), d_sym(h, M.snoc(d, n), d_snoc_l(d, n, h, d_sym(d, h, pdhd(q, h, k, t, d, n, I)), bl_fresh(h, n, pbh(q, h, k, t, d, n, I)))), pdkt(q, h, k, t, d, n, I), d_sym(k, M.snoc(d, n), d_snoc_l(d, n, k, d_sym(d, k, pdkd(q, h, k, t, d, n, I)), bl_fresh(k, n, pbk(q, h, k, t, d, n, I)))), d_sym(t, M.snoc(d, n), d_snoc_l(d, n, t, d_sym(d, t, pdtd(q, h, k, t, d, n, I)), bl_fresh(t, n, pbt(q, h, k, t, d, n, I)))), bl_succ(q, n, pbq(q, h, k, t, d, n, I)), bl_succ(h, n, pbh(q, h, k, t, d, n, I)), bl_succ(k, n, pbk(q, h, k, t, d, n, I)), bl_succ(t, n, pbt(q, h, k, t, d, n, I)), bl_snoc(d, n, 1n+n, bl_succ(d, n, pbd(q, h, k, t, d, n, I)), lt_self(n)))

law frame_deliver_h:
  for +q: +List<Nat>
  for +h: +List<Nat>
  for +k: +List<Nat>
  for +t: +List<Nat>
  for +d: +List<Nat>
  for +n: Nat
  for +id: Nat
  for +m: {M.mem(id, h) == True{} : Bool}
  for +I: {inv(q, h, k, t, d, n) == True{} : Bool}
  {inv(M.snoc(q, id), M.remove(id, h), k, t, d, n) == True{} : Bool}

def frame_deliver_h(q, h, k, t, d, n, id, m, I):
  mk_inv(M.snoc(q, id), M.remove(id, h), k, t, d, n, n_snoc(q, id, pnq(q, h, k, t, d, n, I), d_mem(h, id, q, d_sym(h, q, pdqh(q, h, k, t, d, n, I)), m)), n_remove(id, h, pnh(q, h, k, t, d, n, I)), pnk(q, h, k, t, d, n, I), pnt(q, h, k, t, d, n, I), d_snoc_l(q, id, M.remove(id, h), d_sym(q, M.remove(id, h), d_sub(M.remove(id, h), h, q, d_sym(h, q, pdqh(q, h, k, t, d, n, I)), sub_remove(id, h))), f_remove(id, h, pnh(q, h, k, t, d, n, I))), d_snoc_l(q, id, k, pdqk(q, h, k, t, d, n, I), d_mem(h, id, k, pdhk(q, h, k, t, d, n, I), m)), d_snoc_l(q, id, t, pdqt(q, h, k, t, d, n, I), d_mem(h, id, t, pdht(q, h, k, t, d, n, I), m)), d_snoc_l(q, id, d, pdqd(q, h, k, t, d, n, I), d_mem(h, id, d, pdhd(q, h, k, t, d, n, I), m)), d_sub(M.remove(id, h), h, k, pdhk(q, h, k, t, d, n, I), sub_remove(id, h)), d_sub(M.remove(id, h), h, t, pdht(q, h, k, t, d, n, I), sub_remove(id, h)), d_sub(M.remove(id, h), h, d, pdhd(q, h, k, t, d, n, I), sub_remove(id, h)), pdkt(q, h, k, t, d, n, I), pdkd(q, h, k, t, d, n, I), pdtd(q, h, k, t, d, n, I), bl_snoc(q, id, n, pbq(q, h, k, t, d, n, I), bl_mem(h, id, n, pbh(q, h, k, t, d, n, I), m)), bl_sub(M.remove(id, h), h, n, pbh(q, h, k, t, d, n, I), sub_remove(id, h)), pbk(q, h, k, t, d, n, I), pbt(q, h, k, t, d, n, I), pbd(q, h, k, t, d, n, I))


law frame_deliver_k:
  for +q: +List<Nat>
  for +h: +List<Nat>
  for +k: +List<Nat>
  for +t: +List<Nat>
  for +d: +List<Nat>
  for +n: Nat
  for +id: Nat
  for +m: {M.mem(id, k) == True{} : Bool}
  for +I: {inv(q, h, k, t, d, n) == True{} : Bool}
  {inv(M.snoc(q, id), h, M.remove(id, k), t, d, n) == True{} : Bool}

def frame_deliver_k(q, h, k, t, d, n, id, m, I):
  mk_inv(M.snoc(q, id), h, M.remove(id, k), t, d, n, n_snoc(q, id, pnq(q, h, k, t, d, n, I), d_mem(k, id, q, d_sym(k, q, pdqk(q, h, k, t, d, n, I)), m)), pnh(q, h, k, t, d, n, I), n_remove(id, k, pnk(q, h, k, t, d, n, I)), pnt(q, h, k, t, d, n, I), d_snoc_l(q, id, h, pdqh(q, h, k, t, d, n, I), d_mem(k, id, h, d_sym(k, h, pdhk(q, h, k, t, d, n, I)), m)), d_snoc_l(q, id, M.remove(id, k), d_sym(q, M.remove(id, k), d_sub(M.remove(id, k), k, q, d_sym(k, q, pdqk(q, h, k, t, d, n, I)), sub_remove(id, k))), f_remove(id, k, pnk(q, h, k, t, d, n, I))), d_snoc_l(q, id, t, pdqt(q, h, k, t, d, n, I), d_mem(k, id, t, pdkt(q, h, k, t, d, n, I), m)), d_snoc_l(q, id, d, pdqd(q, h, k, t, d, n, I), d_mem(k, id, d, pdkd(q, h, k, t, d, n, I), m)), d_sym(h, M.remove(id, k), d_sub(M.remove(id, k), k, h, d_sym(k, h, pdhk(q, h, k, t, d, n, I)), sub_remove(id, k))), pdht(q, h, k, t, d, n, I), pdhd(q, h, k, t, d, n, I), d_sub(M.remove(id, k), k, t, pdkt(q, h, k, t, d, n, I), sub_remove(id, k)), d_sub(M.remove(id, k), k, d, pdkd(q, h, k, t, d, n, I), sub_remove(id, k)), pdtd(q, h, k, t, d, n, I), bl_snoc(q, id, n, pbq(q, h, k, t, d, n, I), bl_mem(k, id, n, pbk(q, h, k, t, d, n, I), m)), pbh(q, h, k, t, d, n, I), bl_sub(M.remove(id, k), k, n, pbk(q, h, k, t, d, n, I), sub_remove(id, k)), pbt(q, h, k, t, d, n, I), pbd(q, h, k, t, d, n, I))


law frame_drop_h:
  for +q: +List<Nat>
  for +h: +List<Nat>
  for +k: +List<Nat>
  for +t: +List<Nat>
  for +d: +List<Nat>
  for +n: Nat
  for +id: Nat
  for +m: {M.mem(id, h) == True{} : Bool}
  for +I: {inv(q, h, k, t, d, n) == True{} : Bool}
  {inv(q, M.remove(id, h), k, t, M.snoc(d, id), n) == True{} : Bool}

def frame_drop_h(q, h, k, t, d, n, id, m, I):
  mk_inv(q, M.remove(id, h), k, t, M.snoc(d, id), n, pnq(q, h, k, t, d, n, I), n_remove(id, h, pnh(q, h, k, t, d, n, I)), pnk(q, h, k, t, d, n, I), pnt(q, h, k, t, d, n, I), d_sym(q, M.remove(id, h), d_sub(M.remove(id, h), h, q, d_sym(h, q, pdqh(q, h, k, t, d, n, I)), sub_remove(id, h))), pdqk(q, h, k, t, d, n, I), pdqt(q, h, k, t, d, n, I), d_sym(q, M.snoc(d, id), d_snoc_l(d, id, q, d_sym(d, q, pdqd(q, h, k, t, d, n, I)), d_mem(h, id, q, d_sym(h, q, pdqh(q, h, k, t, d, n, I)), m))), d_sub(M.remove(id, h), h, k, pdhk(q, h, k, t, d, n, I), sub_remove(id, h)), d_sub(M.remove(id, h), h, t, pdht(q, h, k, t, d, n, I), sub_remove(id, h)), d_sym(M.remove(id, h), M.snoc(d, id), d_snoc_l(d, id, M.remove(id, h), d_sym(d, M.remove(id, h), d_sub(M.remove(id, h), h, d, pdhd(q, h, k, t, d, n, I), sub_remove(id, h))), f_remove(id, h, pnh(q, h, k, t, d, n, I)))), pdkt(q, h, k, t, d, n, I), d_sym(k, M.snoc(d, id), d_snoc_l(d, id, k, d_sym(d, k, pdkd(q, h, k, t, d, n, I)), d_mem(h, id, k, pdhk(q, h, k, t, d, n, I), m))), d_sym(t, M.snoc(d, id), d_snoc_l(d, id, t, d_sym(d, t, pdtd(q, h, k, t, d, n, I)), d_mem(h, id, t, pdht(q, h, k, t, d, n, I), m))), pbq(q, h, k, t, d, n, I), bl_sub(M.remove(id, h), h, n, pbh(q, h, k, t, d, n, I), sub_remove(id, h)), pbk(q, h, k, t, d, n, I), pbt(q, h, k, t, d, n, I), bl_snoc(d, id, n, pbd(q, h, k, t, d, n, I), bl_mem(h, id, n, pbh(q, h, k, t, d, n, I), m)))


law frame_drop_k:
  for +q: +List<Nat>
  for +h: +List<Nat>
  for +k: +List<Nat>
  for +t: +List<Nat>
  for +d: +List<Nat>
  for +n: Nat
  for +id: Nat
  for +m: {M.mem(id, k) == True{} : Bool}
  for +I: {inv(q, h, k, t, d, n) == True{} : Bool}
  {inv(q, h, M.remove(id, k), t, M.snoc(d, id), n) == True{} : Bool}

def frame_drop_k(q, h, k, t, d, n, id, m, I):
  mk_inv(q, h, M.remove(id, k), t, M.snoc(d, id), n, pnq(q, h, k, t, d, n, I), pnh(q, h, k, t, d, n, I), n_remove(id, k, pnk(q, h, k, t, d, n, I)), pnt(q, h, k, t, d, n, I), pdqh(q, h, k, t, d, n, I), d_sym(q, M.remove(id, k), d_sub(M.remove(id, k), k, q, d_sym(k, q, pdqk(q, h, k, t, d, n, I)), sub_remove(id, k))), pdqt(q, h, k, t, d, n, I), d_sym(q, M.snoc(d, id), d_snoc_l(d, id, q, d_sym(d, q, pdqd(q, h, k, t, d, n, I)), d_mem(k, id, q, d_sym(k, q, pdqk(q, h, k, t, d, n, I)), m))), d_sym(h, M.remove(id, k), d_sub(M.remove(id, k), k, h, d_sym(k, h, pdhk(q, h, k, t, d, n, I)), sub_remove(id, k))), pdht(q, h, k, t, d, n, I), d_sym(h, M.snoc(d, id), d_snoc_l(d, id, h, d_sym(d, h, pdhd(q, h, k, t, d, n, I)), d_mem(k, id, h, d_sym(k, h, pdhk(q, h, k, t, d, n, I)), m))), d_sub(M.remove(id, k), k, t, pdkt(q, h, k, t, d, n, I), sub_remove(id, k)), d_sym(M.remove(id, k), M.snoc(d, id), d_snoc_l(d, id, M.remove(id, k), d_sym(d, M.remove(id, k), d_sub(M.remove(id, k), k, d, pdkd(q, h, k, t, d, n, I), sub_remove(id, k))), f_remove(id, k, pnk(q, h, k, t, d, n, I)))), d_sym(t, M.snoc(d, id), d_snoc_l(d, id, t, d_sym(d, t, pdtd(q, h, k, t, d, n, I)), d_mem(k, id, t, pdkt(q, h, k, t, d, n, I), m))), pbq(q, h, k, t, d, n, I), pbh(q, h, k, t, d, n, I), bl_sub(M.remove(id, k), k, n, pbk(q, h, k, t, d, n, I), sub_remove(id, k)), pbt(q, h, k, t, d, n, I), bl_snoc(d, id, n, pbd(q, h, k, t, d, n, I), bl_mem(k, id, n, pbk(q, h, k, t, d, n, I), m)))

law frame_flush:
  for +x: Nat
  for +r: +List<Nat>
  for +h: +List<Nat>
  for +k: +List<Nat>
  for +t: +List<Nat>
  for +d: +List<Nat>
  for +n: Nat
  for +I: {inv(x <> r, h, k, t, d, n) == True{} : Bool}
  {inv(r, h, k, M.snoc(t, x), d, n) == True{} : Bool}

def frame_flush(x, r, h, k, t, d, n, I):
  mk_inv(r, h, k, M.snoc(t, x), d, n, and_r(Bool.not(M.mem(x, r)), S.nodup(r), pnq(x <> r, h, k, t, d, n, I)), pnh(x <> r, h, k, t, d, n, I), pnk(x <> r, h, k, t, d, n, I), n_snoc(t, x, pnt(x <> r, h, k, t, d, n, I), and_l(Bool.not(M.mem(x, t)), S.disjoint(r, t), pdqt(x <> r, h, k, t, d, n, I))), and_r(Bool.not(M.mem(x, h)), S.disjoint(r, h), pdqh(x <> r, h, k, t, d, n, I)), and_r(Bool.not(M.mem(x, k)), S.disjoint(r, k), pdqk(x <> r, h, k, t, d, n, I)), d_sym(r, M.snoc(t, x), d_snoc_l(t, x, r, d_sym(t, r, and_r(Bool.not(M.mem(x, t)), S.disjoint(r, t), pdqt(x <> r, h, k, t, d, n, I))), and_l(Bool.not(M.mem(x, r)), S.nodup(r), pnq(x <> r, h, k, t, d, n, I)))), and_r(Bool.not(M.mem(x, d)), S.disjoint(r, d), pdqd(x <> r, h, k, t, d, n, I)), pdhk(x <> r, h, k, t, d, n, I), d_sym(h, M.snoc(t, x), d_snoc_l(t, x, h, d_sym(t, h, pdht(x <> r, h, k, t, d, n, I)), and_l(Bool.not(M.mem(x, h)), S.disjoint(r, h), pdqh(x <> r, h, k, t, d, n, I)))), pdhd(x <> r, h, k, t, d, n, I), d_sym(k, M.snoc(t, x), d_snoc_l(t, x, k, d_sym(t, k, pdkt(x <> r, h, k, t, d, n, I)), and_l(Bool.not(M.mem(x, k)), S.disjoint(r, k), pdqk(x <> r, h, k, t, d, n, I)))), pdkd(x <> r, h, k, t, d, n, I), d_snoc_l(t, x, d, pdtd(x <> r, h, k, t, d, n, I), and_l(Bool.not(M.mem(x, d)), S.disjoint(r, d), pdqd(x <> r, h, k, t, d, n, I))), and_r(lt(x, n), bl(r, n), pbq(x <> r, h, k, t, d, n, I)), pbh(x <> r, h, k, t, d, n, I), pbk(x <> r, h, k, t, d, n, I), bl_snoc(t, x, n, pbt(x <> r, h, k, t, d, n, I), and_l(lt(x, n), bl(r, n), pbq(x <> r, h, k, t, d, n, I))), pbd(x <> r, h, k, t, d, n, I))


law frame_takeover_begin:
  for +q: +List<Nat>
  for +h: +List<Nat>
  for +k: +List<Nat>
  for +t: +List<Nat>
  for +d: +List<Nat>
  for +n: Nat
  for +u: +List<Nat>
  for +I: {inv(q, h, k, t, d, n) == True{} : Bool}
  {inv(M.keep_users(q, u), h, List.append(&2, Nat, k, M.drop_users(q, u)), t, d, n) == True{} : Bool}

def frame_takeover_begin(q, h, k, t, d, n, u, I):
  mk_inv(M.keep_users(q, u), h, List.append(&2, Nat, k, M.drop_users(q, u)), t, d, n, n_keep(q, u, pnq(q, h, k, t, d, n, I)), pnh(q, h, k, t, d, n, I), n_app(k, M.drop_users(q, u), pnk(q, h, k, t, d, n, I), n_drop(q, u, pnq(q, h, k, t, d, n, I)), d_sym(k, M.drop_users(q, u), d_sub(M.drop_users(q, u), q, k, pdqk(q, h, k, t, d, n, I), sub_drop(q, u)))), pnt(q, h, k, t, d, n, I), d_sub(M.keep_users(q, u), q, h, pdqh(q, h, k, t, d, n, I), sub_keep(q, u)), d_sym(M.keep_users(q, u), List.append(&2, Nat, k, M.drop_users(q, u)), d_app_l(k, M.drop_users(q, u), M.keep_users(q, u), d_sym(k, M.keep_users(q, u), d_sub(M.keep_users(q, u), q, k, pdqk(q, h, k, t, d, n, I), sub_keep(q, u))), d_sym(M.drop_users(q, u), M.keep_users(q, u), d_kd(q, u, pnq(q, h, k, t, d, n, I))))), d_sub(M.keep_users(q, u), q, t, pdqt(q, h, k, t, d, n, I), sub_keep(q, u)), d_sub(M.keep_users(q, u), q, d, pdqd(q, h, k, t, d, n, I), sub_keep(q, u)), d_sym(h, List.append(&2, Nat, k, M.drop_users(q, u)), d_app_l(k, M.drop_users(q, u), h, d_sym(k, h, pdhk(q, h, k, t, d, n, I)), d_sub(M.drop_users(q, u), q, h, pdqh(q, h, k, t, d, n, I), sub_drop(q, u)))), pdht(q, h, k, t, d, n, I), pdhd(q, h, k, t, d, n, I), d_app_l(k, M.drop_users(q, u), t, pdkt(q, h, k, t, d, n, I), d_sub(M.drop_users(q, u), q, t, pdqt(q, h, k, t, d, n, I), sub_drop(q, u))), d_app_l(k, M.drop_users(q, u), d, pdkd(q, h, k, t, d, n, I), d_sub(M.drop_users(q, u), q, d, pdqd(q, h, k, t, d, n, I), sub_drop(q, u))), pdtd(q, h, k, t, d, n, I), bl_sub(M.keep_users(q, u), q, n, pbq(q, h, k, t, d, n, I), sub_keep(q, u)), pbh(q, h, k, t, d, n, I), bl_app(k, M.drop_users(q, u), n, pbk(q, h, k, t, d, n, I), bl_sub(M.drop_users(q, u), q, n, pbq(q, h, k, t, d, n, I), sub_drop(q, u))), pbt(q, h, k, t, d, n, I), pbd(q, h, k, t, d, n, I))


law frame_takeover_end:
  for +q: +List<Nat>
  for +h: +List<Nat>
  for +k: +List<Nat>
  for +t: +List<Nat>
  for +d: +List<Nat>
  for +n: Nat
  for +I: {inv(q, h, k, t, d, n) == True{} : Bool}
  {inv(List.append(&2, Nat, q, k), h, [], t, d, n) == True{} : Bool}

def frame_takeover_end(q, h, k, t, d, n, I):
  mk_inv(List.append(&2, Nat, q, k), h, [], t, d, n, n_app(q, k, pnq(q, h, k, t, d, n, I), pnk(q, h, k, t, d, n, I), pdqk(q, h, k, t, d, n, I)), pnh(q, h, k, t, d, n, I), {==}, pnt(q, h, k, t, d, n, I), d_app_l(q, k, h, pdqh(q, h, k, t, d, n, I), d_sym(k, h, pdhk(q, h, k, t, d, n, I))), d_nil_r(List.append(&2, Nat, q, k)), d_app_l(q, k, t, pdqt(q, h, k, t, d, n, I), pdkt(q, h, k, t, d, n, I)), d_app_l(q, k, d, pdqd(q, h, k, t, d, n, I), pdkd(q, h, k, t, d, n, I)), d_nil_r(h), pdht(q, h, k, t, d, n, I), pdhd(q, h, k, t, d, n, I), {==}, {==}, pdtd(q, h, k, t, d, n, I), bl_app(q, k, n, pbq(q, h, k, t, d, n, I), pbk(q, h, k, t, d, n, I)), pbh(q, h, k, t, d, n, I), {==}, pbt(q, h, k, t, d, n, I), pbd(q, h, k, t, d, n, I))


law frame_exit:
  for +q: +List<Nat>
  for +h: +List<Nat>
  for +k: +List<Nat>
  for +t: +List<Nat>
  for +d: +List<Nat>
  for +n: Nat
  for +I: {inv(q, h, k, t, d, n) == True{} : Bool}
  {inv([], h, [], t, List.append(&2, Nat, d, List.append(&2, Nat, q, k)), n) == True{} : Bool}

def frame_exit(q, h, k, t, d, n, I):
  mk_inv([], h, [], t, List.append(&2, Nat, d, List.append(&2, Nat, q, k)), n, {==}, pnh(q, h, k, t, d, n, I), {==}, pnt(q, h, k, t, d, n, I), {==}, {==}, {==}, {==}, d_nil_r(h), pdht(q, h, k, t, d, n, I), d_sym(h, List.append(&2, Nat, d, List.append(&2, Nat, q, k)), d_app_l(d, List.append(&2, Nat, q, k), h, d_sym(d, h, pdhd(q, h, k, t, d, n, I)), d_app_l(q, k, h, pdqh(q, h, k, t, d, n, I), d_sym(k, h, pdhk(q, h, k, t, d, n, I))))), {==}, {==}, d_sym(t, List.append(&2, Nat, d, List.append(&2, Nat, q, k)), d_app_l(d, List.append(&2, Nat, q, k), t, d_sym(d, t, pdtd(q, h, k, t, d, n, I)), d_app_l(q, k, t, pdqt(q, h, k, t, d, n, I), pdkt(q, h, k, t, d, n, I)))), {==}, pbh(q, h, k, t, d, n, I), {==}, pbt(q, h, k, t, d, n, I), bl_app(d, List.append(&2, Nat, q, k), n, pbd(q, h, k, t, d, n, I), bl_app(q, k, n, pbq(q, h, k, t, d, n, I), pbk(q, h, k, t, d, n, I))))


def InvW(s: M.World) -> Data:
  match s:
    case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
      {inv(q, haf, htk, t, d, n) == True{} : Bool}

law invw_init:
  InvW(M.init())

def invw_init():
  {==}

law invw_flush:
  for +s: M.World
  for +I: InvW(s)
  InvW(M.flush(s))

def invw_flush(s, I):
  match s:
    case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
      match q i:
        case Nil{} False{}:
          I
        case Nil{} True{}:
          I
        case Con{+x, rest} False{}:
          I
        case Con{+x, rest} True{}:
          frame_flush(x, rest, haf, htk, t, d, n, I)

law invw_send_route:
  for fu: Bool
  for tk: Bool
  for +s: M.World
  for +I: InvW(s)
  InvW(M.send_route(fu, tk, s))

def invw_send_route(fu, tk, s, I):
  match fu:
    case True{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          invw_flush(M.W{M.snoc(q, n), haf, htk, t, d, M.snoc(u, n), tt, rl, 1n+n, tk, i, g}, frame_send_q(q, haf, htk, t, d, n, I))
    case False{}:
      match tk:
        case True{}:
          match s:
            case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
              frame_send_k(q, haf, htk, t, d, n, I)
        case False{}:
          match s:
            case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
              invw_flush(M.W{M.snoc(q, n), haf, htk, t, d, u, tt, rl, 1n+n, tk, i, g}, frame_send_q(q, haf, htk, t, d, n, I))

law invw_send_auto:
  for fu: Bool
  for +s: M.World
  for +I: InvW(s)
  InvW(M.send_auto(fu, s))

def invw_send_auto(fu, s, I):
  match s:
    case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
      invw_send_route(fu, tk, M.W{q, haf, htk, t, d, u, tt, rl, n, tk, i, g}, I)

law invw_deliver_if:
  for +id: Nat
  for +s: M.World
  for +I: InvW(s)
  for +m: {M.mem(id, S.held_af(s)) == True{} : Bool}
  InvW(M.deliver_if(True{}, id, s))

def invw_deliver_if(id, s, I, m):
  match s:
    case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
      invw_flush(M.W{M.snoc(q, id), M.remove(id, haf), htk, t, d, u, tt, M.snoc(rl, id), n, tk, i, g}, frame_deliver_h(q, haf, htk, t, d, n, id, m, I))

law invw_deliver_tk:
  for b: Bool
  for +id: Nat
  for +s: M.World
  for +I: InvW(s)
  for hb: @k: {b == True{} : Bool} -> {M.mem(id, S.held_tk(s)) == True{} : Bool}
  InvW(M.deliver_tk(b, id, s))

def invw_deliver_tk(b, id, s, I, hb):
  match b:
    case False{}:
      I
    case True{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          invw_flush(M.W{M.snoc(q, id), haf, M.remove(id, htk), t, d, u, tt, M.snoc(rl, id), n, tk, i, g}, frame_deliver_k(q, haf, htk, t, d, n, id, hb({==}), I))

law invw_deliver_pick:
  for a: Bool
  for b: Bool
  for +id: Nat
  for +s: M.World
  for +I: InvW(s)
  for ha: @k: {a == True{} : Bool} -> {M.mem(id, S.held_af(s)) == True{} : Bool}
  for hb: @k: {b == True{} : Bool} -> {M.mem(id, S.held_tk(s)) == True{} : Bool}
  InvW(M.deliver_pick(a, b, id, s))

def invw_deliver_pick(a, b, id, s, I, ha, hb):
  match a:
    case True{}:
      invw_deliver_if(id, s, I, ha({==}))
    case False{}:
      invw_deliver_tk(b, id, s, I, hb)

law invw_drop_if:
  for +id: Nat
  for +s: M.World
  for +I: InvW(s)
  for +m: {M.mem(id, S.held_af(s)) == True{} : Bool}
  InvW(M.drop_if(True{}, id, s))

def invw_drop_if(id, s, I, m):
  match s:
    case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
      frame_drop_h(q, haf, htk, t, d, n, id, m, I)

law invw_drop_tk:
  for b: Bool
  for +id: Nat
  for +s: M.World
  for +I: InvW(s)
  for hb: @k: {b == True{} : Bool} -> {M.mem(id, S.held_tk(s)) == True{} : Bool}
  InvW(M.drop_tk(b, id, s))

def invw_drop_tk(b, id, s, I, hb):
  match b:
    case False{}:
      I
    case True{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          frame_drop_k(q, haf, htk, t, d, n, id, hb({==}), I)

law invw_drop_pick:
  for a: Bool
  for b: Bool
  for +id: Nat
  for +s: M.World
  for +I: InvW(s)
  for ha: @k: {a == True{} : Bool} -> {M.mem(id, S.held_af(s)) == True{} : Bool}
  for hb: @k: {b == True{} : Bool} -> {M.mem(id, S.held_tk(s)) == True{} : Bool}
  InvW(M.drop_pick(a, b, id, s))

def invw_drop_pick(a, b, id, s, I, ha, hb):
  match a:
    case True{}:
      invw_drop_if(id, s, I, ha({==}))
    case False{}:
      invw_drop_tk(b, id, s, I, hb)

law invw_step:
  for e: M.Ev
  for +s: M.World
  for +I: InvW(s)
  InvW(M.step(e, s))

def invw_step(e, s, I):
  match e:
    case M.Send{fu, r}:
      match r:
        case M.Auto{}:
          match s:
            case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
              match g:
                case True{}:
                  I
                case False{}:
                  invw_send_auto(fu, M.W{q, haf, htk, t, d, u, tt, rl, n, tk, i, False{}}, I)
        case M.AskFirst{}:
          match s:
            case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
              match g:
                case True{}:
                  I
                case False{}:
                  frame_send_h(q, haf, htk, t, d, n, I)
        case M.RouteDrop{}:
          match s:
            case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
              match g:
                case True{}:
                  I
                case False{}:
                  frame_send_d(q, haf, htk, t, d, n, I)
    case M.Idle{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          match g:
            case True{}:
              I
            case False{}:
              invw_flush(M.W{q, haf, htk, t, d, u, tt, rl, n, tk, True{}, False{}}, I)
    case M.Busy{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          I
    case M.Exit{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          frame_exit(q, haf, htk, t, d, n, I)
    case M.TakeoverBegin{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          frame_takeover_begin(q, haf, htk, t, d, n, u, I)
    case M.TakeoverEnd{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          invw_flush(M.W{List.append(&2, Nat, q, htk), haf, [], t, d, u, tt, rl, n, False{}, i, g}, frame_takeover_end(q, haf, htk, t, d, n, I))
    case M.UserDeliver{+id}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          invw_deliver_pick(Bool.and(M.mem(id, haf), Bool.not(g)), Bool.and(M.mem(id, htk), Bool.not(g)), id, M.W{q, haf, htk, t, d, u, tt, rl, n, tk, i, g}, I, k => and_l(M.mem(id, haf), Bool.not(g), k), k => and_l(M.mem(id, htk), Bool.not(g), k))
    case M.UserDrop{+id}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          invw_drop_pick(M.mem(id, haf), M.mem(id, htk), id, M.W{q, haf, htk, t, d, u, tt, rl, n, tk, i, g}, I, k => k, k => k)

law m1_of:
  for +s: M.World
  for +I: InvW(s)
  {S.nodup(S.typed(s)) == True{} : Bool}

def m1_of(s, I):
  match s:
    case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
      pnt(q, haf, htk, t, d, n, I)

law m1_run:
  for +evs: +List<M.Ev>
  for +s: M.World
  for +I: InvW(s)
  {S.nodup(S.typed(M.run(evs, s))) == True{} : Bool}

def m1_run(evs, s, I):
  match evs:
    case Nil{}:
      m1_of(s, I)
    case Con{e, rest}:
      m1_run(rest, M.step(e, s), invw_step(e, s, I))

def Laws.m1_final_and_at_most_once(evs):
  m1_run(evs, M.init(), invw_init())

law m5_of:
  for +s: M.World
  for +I: InvW(s)
  {Bool.and(S.disjoint(S.typed(s), S.dropped(s)), Bool.and(S.disjoint(S.typed(s), S.queue(s)), Bool.and(S.disjoint(S.typed(s), S.held_af(s)), S.disjoint(S.typed(s), S.held_tk(s))))) == True{} : Bool}

def m5_of(s, I):
  match s:
    case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
      and_intro(S.disjoint(t, d), Bool.and(S.disjoint(t, q), Bool.and(S.disjoint(t, haf), S.disjoint(t, htk))), pdtd(q, haf, htk, t, d, n, I), and_intro(S.disjoint(t, q), Bool.and(S.disjoint(t, haf), S.disjoint(t, htk)), d_sym(t, q, pdqt(q, haf, htk, t, d, n, I)), and_intro(S.disjoint(t, haf), S.disjoint(t, htk), d_sym(t, haf, pdht(q, haf, htk, t, d, n, I)), d_sym(t, htk, pdkt(q, haf, htk, t, d, n, I)))))

law m5_run:
  for +evs: +List<M.Ev>
  for +s: M.World
  for +I: InvW(s)
  {Bool.and(S.disjoint(S.typed(M.run(evs, s)), S.dropped(M.run(evs, s))), Bool.and(S.disjoint(S.typed(M.run(evs, s)), S.queue(M.run(evs, s))), Bool.and(S.disjoint(S.typed(M.run(evs, s)), S.held_af(M.run(evs, s))), S.disjoint(S.typed(M.run(evs, s)), S.held_tk(M.run(evs, s)))))) == True{} : Bool}

def m5_run(evs, s, I):
  match evs:
    case Nil{}:
      m5_of(s, I)
    case Con{e, rest}:
      m5_run(rest, M.step(e, s), invw_step(e, s, I))

def Laws.m5_terminal_states_are_absorbing(evs):
  m5_run(evs, M.init(), invw_init())

# ---- M4: the typed list only grows at its end ----

law pre_refl:
  for +xs: +List<Nat>
  {S.is_prefix(xs, xs) == True{} : Bool}

def pre_refl(xs):
  match xs:
    case Nil{}:
      {==}
    case Con{+x, rest}:
      %Equal.sym(Bool, Nat.is_eq(x, x), True{}, eq_refl(x)) : {Bool.and(_, S.is_prefix(rest, rest)) == True{} : Bool}
      pre_refl(rest)

law pre_snoc:
  for +xs: +List<Nat>
  for +y: Nat
  {S.is_prefix(xs, M.snoc(xs, y)) == True{} : Bool}

def pre_snoc(xs, y):
  match xs:
    case Nil{}:
      {==}
    case Con{+x, rest}:
      %Equal.sym(Bool, Nat.is_eq(x, x), True{}, eq_refl(x)) : {Bool.and(_, S.is_prefix(rest, M.snoc(rest, y))) == True{} : Bool}
      pre_snoc(rest, y)

law pre_flush:
  for +s: M.World
  {S.is_prefix(S.typed(s), S.typed(M.flush(s))) == True{} : Bool}

def pre_flush(s):
  match s:
    case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
      match q i:
        case Nil{} False{}:
          pre_refl(t)
        case Nil{} True{}:
          pre_refl(t)
        case Con{+x, rest} False{}:
          pre_refl(t)
        case Con{+x, rest} True{}:
          pre_snoc(t, x)

law pre_send_route:
  for fu: Bool
  for tk: Bool
  for +s: M.World
  {S.is_prefix(S.typed(s), S.typed(M.send_route(fu, tk, s))) == True{} : Bool}

def pre_send_route(fu, tk, s):
  match fu:
    case True{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          pre_flush(M.W{M.snoc(q, n), haf, htk, t, d, M.snoc(u, n), tt, rl, 1n+n, tk, i, g})
    case False{}:
      match tk:
        case True{}:
          match s:
            case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
              pre_refl(t)
        case False{}:
          match s:
            case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
              pre_flush(M.W{M.snoc(q, n), haf, htk, t, d, u, tt, rl, 1n+n, tk, i, g})

law pre_deliver_tk:
  for b: Bool
  for +id: Nat
  for +s: M.World
  {S.is_prefix(S.typed(s), S.typed(M.deliver_tk(b, id, s))) == True{} : Bool}

def pre_deliver_tk(b, id, s):
  match b:
    case False{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          pre_refl(t)
    case True{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          pre_flush(M.W{M.snoc(q, id), haf, M.remove(id, htk), t, d, u, tt, M.snoc(rl, id), n, tk, i, g})

law pre_deliver_pick:
  for a: Bool
  for b: Bool
  for +id: Nat
  for +s: M.World
  {S.is_prefix(S.typed(s), S.typed(M.deliver_pick(a, b, id, s))) == True{} : Bool}

def pre_deliver_pick(a, b, id, s):
  match a:
    case True{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          pre_flush(M.W{M.snoc(q, id), M.remove(id, haf), htk, t, d, u, tt, M.snoc(rl, id), n, tk, i, g})
    case False{}:
      pre_deliver_tk(b, id, s)

law pre_drop_tk:
  for b: Bool
  for +id: Nat
  for +s: M.World
  {S.is_prefix(S.typed(s), S.typed(M.drop_tk(b, id, s))) == True{} : Bool}

def pre_drop_tk(b, id, s):
  match b:
    case False{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          pre_refl(t)
    case True{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          pre_refl(t)

law pre_drop_pick:
  for a: Bool
  for b: Bool
  for +id: Nat
  for +s: M.World
  {S.is_prefix(S.typed(s), S.typed(M.drop_pick(a, b, id, s))) == True{} : Bool}

def pre_drop_pick(a, b, id, s):
  match a:
    case True{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          pre_refl(t)
    case False{}:
      pre_drop_tk(b, id, s)

law pre_step:
  for e: M.Ev
  for +s: M.World
  {S.is_prefix(S.typed(s), S.typed(M.step(e, s))) == True{} : Bool}

def pre_step(e, s):
  match e:
    case M.Send{fu, r}:
      match r:
        case M.Auto{}:
          match s:
            case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
              match g:
                case True{}:
                  pre_refl(t)
                case False{}:
                  pre_send_route(fu, tk, M.W{q, haf, htk, t, d, u, tt, rl, n, tk, i, False{}})
        case M.AskFirst{}:
          match s:
            case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
              match g:
                case True{}:
                  pre_refl(t)
                case False{}:
                  pre_refl(t)
        case M.RouteDrop{}:
          match s:
            case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
              match g:
                case True{}:
                  pre_refl(t)
                case False{}:
                  pre_refl(t)
    case M.Idle{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          match g:
            case True{}:
              pre_refl(t)
            case False{}:
              pre_flush(M.W{q, haf, htk, t, d, u, tt, rl, n, tk, True{}, False{}})
    case M.Busy{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          pre_refl(t)
    case M.Exit{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          pre_refl(t)
    case M.TakeoverBegin{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          pre_refl(t)
    case M.TakeoverEnd{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          pre_flush(M.W{List.append(&2, Nat, q, htk), haf, [], t, d, u, tt, rl, n, False{}, i, g})
    case M.UserDeliver{+id}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          pre_deliver_pick(Bool.and(M.mem(id, haf), Bool.not(g)), Bool.and(M.mem(id, htk), Bool.not(g)), id, M.W{q, haf, htk, t, d, u, tt, rl, n, tk, i, g})
    case M.UserDrop{+id}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          pre_drop_pick(M.mem(id, haf), M.mem(id, htk), id, M.W{q, haf, htk, t, d, u, tt, rl, n, tk, i, g})

def Laws.m4_typed_only_grows_at_the_end(evs, e):
  pre_step(e, S.world(evs))

# ---- M7: once the receiver is gone, nothing is pending and nothing waits on a Takeover ----

law not_true:
  for b: Bool
  for +h: {Bool.not(b) == True{} : Bool}
  {b == False{} : Bool}

def not_true(b, h):
  match b:
    case False{}:
      {==}
    case True{}:
      Empty.absurd({True{} == False{} : Bool}, ff_tt(h))

def Inv7(s: M.World) -> Data:
  match s:
    case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
      {S.implies(g, Bool.and(S.is_nil(q), S.is_nil(htk))) == True{} : Bool}

law inv7_init:
  Inv7(M.init())

def inv7_init():
  {==}

law inv7_flush:
  for +s: M.World
  for +I: Inv7(s)
  Inv7(M.flush(s))

def inv7_flush(s, I):
  match s:
    case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
      match q i:
        case Nil{} False{}:
          I
        case Nil{} True{}:
          I
        case Con{+x, rest} False{}:
          I
        case Con{+x, rest} True{}:
          match g:
            case True{}:
              Empty.absurd({S.implies(True{}, Bool.and(S.is_nil(rest), S.is_nil(htk))) == True{} : Bool}, ff_tt(I))
            case False{}:
              {==}

law inv7_send_route:
  for fu: Bool
  for tk: Bool
  for +s: M.World
  for +hg: {S.gone(s) == False{} : Bool}
  Inv7(M.send_route(fu, tk, s))

def inv7_send_route(fu, tk, s, hg):
  match fu:
    case True{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          match g:
            case True{}:
              Empty.absurd(Inv7(M.flush(M.W{M.snoc(q, n), haf, htk, t, d, M.snoc(u, n), tt, rl, 1n+n, tk, i, True{}})), tt_ff(hg))
            case False{}:
              inv7_flush(M.W{M.snoc(q, n), haf, htk, t, d, M.snoc(u, n), tt, rl, 1n+n, tk, i, False{}}, {==})
    case False{}:
      match tk:
        case True{}:
          match s:
            case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
              match g:
                case True{}:
                  Empty.absurd(Inv7(M.W{q, haf, M.snoc(htk, n), t, d, u, tt, rl, 1n+n, tk, i, True{}}), tt_ff(hg))
                case False{}:
                  {==}
        case False{}:
          match s:
            case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
              match g:
                case True{}:
                  Empty.absurd(Inv7(M.flush(M.W{M.snoc(q, n), haf, htk, t, d, u, tt, rl, 1n+n, tk, i, True{}})), tt_ff(hg))
                case False{}:
                  inv7_flush(M.W{M.snoc(q, n), haf, htk, t, d, u, tt, rl, 1n+n, tk, i, False{}}, {==})

law inv7_deliver_tk:
  for b: Bool
  for +id: Nat
  for +s: M.World
  for +I: Inv7(s)
  for hb: @k: {b == True{} : Bool} -> {S.gone(s) == False{} : Bool}
  Inv7(M.deliver_tk(b, id, s))

def inv7_deliver_tk(b, id, s, I, hb):
  match b:
    case False{}:
      I
    case True{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          match g:
            case True{}:
              Empty.absurd(Inv7(M.flush(M.W{M.snoc(q, id), haf, M.remove(id, htk), t, d, u, tt, M.snoc(rl, id), n, tk, i, True{}})), tt_ff(hb({==})))
            case False{}:
              inv7_flush(M.W{M.snoc(q, id), haf, M.remove(id, htk), t, d, u, tt, M.snoc(rl, id), n, tk, i, False{}}, {==})

law inv7_deliver_pick:
  for a: Bool
  for b: Bool
  for +id: Nat
  for +s: M.World
  for +I: Inv7(s)
  for ha: @k: {a == True{} : Bool} -> {S.gone(s) == False{} : Bool}
  for hb: @k: {b == True{} : Bool} -> {S.gone(s) == False{} : Bool}
  Inv7(M.deliver_pick(a, b, id, s))

def inv7_deliver_pick(a, b, id, s, I, ha, hb):
  match a:
    case True{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          match g:
            case True{}:
              Empty.absurd(Inv7(M.flush(M.W{M.snoc(q, id), M.remove(id, haf), htk, t, d, u, tt, M.snoc(rl, id), n, tk, i, True{}})), tt_ff(ha({==})))
            case False{}:
              inv7_flush(M.W{M.snoc(q, id), M.remove(id, haf), htk, t, d, u, tt, M.snoc(rl, id), n, tk, i, False{}}, {==})
    case False{}:
      inv7_deliver_tk(b, id, s, I, hb)

law inv7_drop_tk:
  for b: Bool
  for +id: Nat
  for +s: M.World
  for +I: Inv7(s)
  Inv7(M.drop_tk(b, id, s))

def inv7_drop_tk(b, id, s, I):
  match b:
    case False{}:
      I
    case True{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          match q htk g:
            case Nil{} Nil{} False{}:
              {==}
            case Nil{} Nil{} True{}:
              {==}
            case Nil{} Con{+y, ys} False{}:
              {==}
            case Nil{} Con{+y, ys} True{}:
              Empty.absurd(Inv7(M.W{[], haf, M.remove(id, y <> ys), t, M.snoc(d, id), u, tt, rl, n, tk, i, True{}}), ff_tt(I))
            case Con{+x, xs} Nil{} False{}:
              {==}
            case Con{+x, xs} Nil{} True{}:
              Empty.absurd(Inv7(M.W{x <> xs, haf, M.remove(id, []), t, M.snoc(d, id), u, tt, rl, n, tk, i, True{}}), ff_tt(I))
            case Con{+x, xs} Con{+y, ys} False{}:
              {==}
            case Con{+x, xs} Con{+y, ys} True{}:
              Empty.absurd(Inv7(M.W{x <> xs, haf, M.remove(id, y <> ys), t, M.snoc(d, id), u, tt, rl, n, tk, i, True{}}), ff_tt(I))

law inv7_drop_pick:
  for a: Bool
  for b: Bool
  for +id: Nat
  for +s: M.World
  for +I: Inv7(s)
  Inv7(M.drop_pick(a, b, id, s))

def inv7_drop_pick(a, b, id, s, I):
  match a:
    case True{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          I
    case False{}:
      inv7_drop_tk(b, id, s, I)

law inv7_step:
  for e: M.Ev
  for +s: M.World
  for +I: Inv7(s)
  Inv7(M.step(e, s))

def inv7_step(e, s, I):
  match e:
    case M.Send{fu, r}:
      match r:
        case M.Auto{}:
          match s:
            case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
              match g:
                case True{}:
                  I
                case False{}:
                  inv7_send_route(fu, tk, M.W{q, haf, htk, t, d, u, tt, rl, n, tk, i, False{}}, {==})
        case M.AskFirst{}:
          match s:
            case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
              match g:
                case True{}:
                  I
                case False{}:
                  {==}
        case M.RouteDrop{}:
          match s:
            case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
              match g:
                case True{}:
                  I
                case False{}:
                  {==}
    case M.Idle{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          match g:
            case True{}:
              I
            case False{}:
              inv7_flush(M.W{q, haf, htk, t, d, u, tt, rl, n, tk, True{}, False{}}, {==})
    case M.Busy{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          I
    case M.Exit{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          {==}
    case M.TakeoverBegin{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          match q htk g:
            case Nil{} Nil{} False{}:
              {==}
            case Nil{} Nil{} True{}:
              {==}
            case Nil{} Con{+y, ys} False{}:
              {==}
            case Nil{} Con{+y, ys} True{}:
              Empty.absurd(Inv7(M.W{[], haf, List.append(&2, Nat, y <> ys, []), t, d, u, tt, rl, n, True{}, i, True{}}), ff_tt(I))
            case Con{+x, xs} Nil{} False{}:
              {==}
            case Con{+x, xs} Nil{} True{}:
              Empty.absurd(Inv7(M.W{M.keep_users(x <> xs, u), haf, List.append(&2, Nat, [], M.drop_users(x <> xs, u)), t, d, u, tt, rl, n, True{}, i, True{}}), ff_tt(I))
            case Con{+x, xs} Con{+y, ys} False{}:
              {==}
            case Con{+x, xs} Con{+y, ys} True{}:
              Empty.absurd(Inv7(M.W{M.keep_users(x <> xs, u), haf, List.append(&2, Nat, y <> ys, M.drop_users(x <> xs, u)), t, d, u, tt, rl, n, True{}, i, True{}}), ff_tt(I))
    case M.TakeoverEnd{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          match q htk g:
            case Nil{} Nil{} False{}:
              inv7_flush(M.W{List.append(&2, Nat, [], []), haf, [], t, d, u, tt, rl, n, False{}, i, False{}}, {==})
            case Nil{} Nil{} True{}:
              inv7_flush(M.W{List.append(&2, Nat, [], []), haf, [], t, d, u, tt, rl, n, False{}, i, True{}}, {==})
            case Nil{} Con{+y, ys} False{}:
              inv7_flush(M.W{List.append(&2, Nat, [], y <> ys), haf, [], t, d, u, tt, rl, n, False{}, i, False{}}, {==})
            case Nil{} Con{+y, ys} True{}:
              Empty.absurd(Inv7(M.flush(M.W{List.append(&2, Nat, [], y <> ys), haf, [], t, d, u, tt, rl, n, False{}, i, True{}})), ff_tt(I))
            case Con{+x, xs} Nil{} False{}:
              inv7_flush(M.W{List.append(&2, Nat, x <> xs, []), haf, [], t, d, u, tt, rl, n, False{}, i, False{}}, {==})
            case Con{+x, xs} Nil{} True{}:
              Empty.absurd(Inv7(M.flush(M.W{List.append(&2, Nat, x <> xs, []), haf, [], t, d, u, tt, rl, n, False{}, i, True{}})), ff_tt(I))
            case Con{+x, xs} Con{+y, ys} False{}:
              inv7_flush(M.W{List.append(&2, Nat, x <> xs, y <> ys), haf, [], t, d, u, tt, rl, n, False{}, i, False{}}, {==})
            case Con{+x, xs} Con{+y, ys} True{}:
              Empty.absurd(Inv7(M.flush(M.W{List.append(&2, Nat, x <> xs, y <> ys), haf, [], t, d, u, tt, rl, n, False{}, i, True{}})), ff_tt(I))
    case M.UserDeliver{+id}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          inv7_deliver_pick(Bool.and(M.mem(id, haf), Bool.not(g)), Bool.and(M.mem(id, htk), Bool.not(g)), id, M.W{q, haf, htk, t, d, u, tt, rl, n, tk, i, g}, I, k => not_true(g, and_r(M.mem(id, haf), Bool.not(g), k)), k => not_true(g, and_r(M.mem(id, htk), Bool.not(g), k)))
    case M.UserDrop{+id}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          inv7_drop_pick(M.mem(id, haf), M.mem(id, htk), id, M.W{q, haf, htk, t, d, u, tt, rl, n, tk, i, g}, I)

law m7_of:
  for fu: Bool
  for r: M.Route
  for +s: M.World
  for +I: Inv7(s)
  {Bool.and(S.implies(S.gone(s), Bool.and(S.is_nil(S.queue(s)), S.is_nil(S.held_tk(s)))), S.implies(S.gone(s), Nat.is_eq(S.next_id(M.step(M.Send{fu, r}, s)), S.next_id(s)))) == True{} : Bool}

def m7_of(fu, r, s, I):
  match s:
    case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
      match g:
        case True{}:
          and_intro(Bool.and(S.is_nil(q), S.is_nil(htk)), Nat.is_eq(n, n), I, eq_refl(n))
        case False{}:
          {==}

law m7_run:
  for +evs: +List<M.Ev>
  for fu: Bool
  for r: M.Route
  for +s: M.World
  for +I: Inv7(s)
  {Bool.and(S.implies(S.gone(M.run(evs, s)), Bool.and(S.is_nil(S.queue(M.run(evs, s))), S.is_nil(S.held_tk(M.run(evs, s))))), S.implies(S.gone(M.run(evs, s)), Nat.is_eq(S.next_id(M.step(M.Send{fu, r}, M.run(evs, s))), S.next_id(M.run(evs, s))))) == True{} : Bool}

def m7_run(evs, fu, r, s, I):
  match evs:
    case Nil{}:
      m7_of(fu, r, s, I)
    case Con{e, rest}:
      m7_run(rest, fu, r, M.step(e, s), inv7_step(e, s, I))

def Laws.m7_receiver_gone(evs, fu, r):
  m7_run(evs, fu, r, M.init(), inv7_init())

# ---- M6: the lengths of the five lists add up to the ids handed out ----

def T5(+a: Nat, +b: Nat, +c: Nat, +d: Nat, +e: Nat) -> Nat:
  Nat.add(a, Nat.add(b, Nat.add(c, Nat.add(d, e))))

law add_sr:
  for +a: Nat
  for +b: Nat
  {Nat.add(a, 1n+b) == 1n+Nat.add(a, b) : Nat}

def add_sr(a, b):
  match a:
    case 0n:
      {==}
    case 1n+p:
      Equal.cong(Nat, Nat, z => 1n+z, Nat.add(p, 1n+b), 1n+Nat.add(p, b), add_sr(p, b))

law add_zero_r:
  for +a: Nat
  {Nat.add(a, 0n) == a : Nat}

def add_zero_r(a):
  match a:
    case 0n:
      {==}
    case 1n+p:
      Equal.cong(Nat, Nat, z => 1n+z, Nat.add(p, 0n), p, add_zero_r(p))

law add_assoc:
  for +a: Nat
  for +b: Nat
  for +c: Nat
  {Nat.add(Nat.add(a, b), c) == Nat.add(a, Nat.add(b, c)) : Nat}

def add_assoc(a, b, c):
  match a:
    case 0n:
      {==}
    case 1n+p:
      Equal.cong(Nat, Nat, z => 1n+z, Nat.add(Nat.add(p, b), c), Nat.add(p, Nat.add(b, c)), add_assoc(p, b, c))

law w3:
  for +a: Nat
  for +b: Nat
  for +x: Nat
  {Nat.add(a, Nat.add(b, 1n+x)) == 1n+Nat.add(a, Nat.add(b, x)) : Nat}

def w3(a, b, x):
  Equal.trans(Nat, Nat.add(a, Nat.add(b, 1n+x)), Nat.add(a, 1n+Nat.add(b, x)), 1n+Nat.add(a, Nat.add(b, x)), Equal.cong(Nat, Nat, z => Nat.add(a, z), Nat.add(b, 1n+x), 1n+Nat.add(b, x), add_sr(b, x)), add_sr(a, Nat.add(b, x)))

law w4:
  for +a: Nat
  for +b: Nat
  for +c: Nat
  for +y: Nat
  {Nat.add(a, Nat.add(b, Nat.add(c, 1n+y))) == 1n+Nat.add(a, Nat.add(b, Nat.add(c, y))) : Nat}

def w4(a, b, c, y):
  Equal.trans(Nat, Nat.add(a, Nat.add(b, Nat.add(c, 1n+y))), Nat.add(a, Nat.add(b, 1n+Nat.add(c, y))), 1n+Nat.add(a, Nat.add(b, Nat.add(c, y))), Equal.cong(Nat, Nat, z => Nat.add(a, Nat.add(b, z)), Nat.add(c, 1n+y), 1n+Nat.add(c, y), add_sr(c, y)), w3(a, b, Nat.add(c, y)))

law w5:
  for +a: Nat
  for +b: Nat
  for +c: Nat
  for +d: Nat
  for +z: Nat
  {Nat.add(a, Nat.add(b, Nat.add(c, Nat.add(d, 1n+z)))) == 1n+Nat.add(a, Nat.add(b, Nat.add(c, Nat.add(d, z)))) : Nat}

def w5(a, b, c, d, z):
  Equal.trans(Nat, Nat.add(a, Nat.add(b, Nat.add(c, Nat.add(d, 1n+z)))), Nat.add(a, Nat.add(b, Nat.add(c, 1n+Nat.add(d, z)))), 1n+Nat.add(a, Nat.add(b, Nat.add(c, Nat.add(d, z)))), Equal.cong(Nat, Nat, y => Nat.add(a, Nat.add(b, Nat.add(c, y))), Nat.add(d, 1n+z), 1n+Nat.add(d, z), add_sr(d, z)), w4(a, b, c, Nat.add(d, z)))

law t2:
  for +a: Nat
  for +b: Nat
  for +c: Nat
  for +d: Nat
  for +e: Nat
  {T5(a, 1n+b, c, d, e) == 1n+T5(a, b, c, d, e) : Nat}

def t2(a, b, c, d, e):
  add_sr(a, Nat.add(b, Nat.add(c, Nat.add(d, e))))

law t3:
  for +a: Nat
  for +b: Nat
  for +c: Nat
  for +d: Nat
  for +e: Nat
  {T5(a, b, 1n+c, d, e) == 1n+T5(a, b, c, d, e) : Nat}

def t3(a, b, c, d, e):
  w3(a, b, Nat.add(c, Nat.add(d, e)))

law t4:
  for +a: Nat
  for +b: Nat
  for +c: Nat
  for +d: Nat
  for +e: Nat
  {T5(a, b, c, 1n+d, e) == 1n+T5(a, b, c, d, e) : Nat}

def t4(a, b, c, d, e):
  w4(a, b, c, Nat.add(d, e))

law t5:
  for +a: Nat
  for +b: Nat
  for +c: Nat
  for +d: Nat
  for +e: Nat
  {T5(a, b, c, d, 1n+e) == 1n+T5(a, b, c, d, e) : Nat}

def t5(a, b, c, d, e):
  w5(a, b, c, d, e)

law mv13:
  for +a: Nat
  for +m: Nat
  for +b: Nat
  for +c: Nat
  for +d: Nat
  for +e: Nat
  {T5(Nat.add(a, m), b, c, d, e) == T5(a, b, Nat.add(c, m), d, e) : Nat}

def mv13(a, m, b, c, d, e):
  match m:
    case 0n:
      Equal.trans(Nat, T5(Nat.add(a, 0n), b, c, d, e), T5(a, b, c, d, e), T5(a, b, Nat.add(c, 0n), d, e), Equal.cong(Nat, Nat, z => T5(z, b, c, d, e), Nat.add(a, 0n), a, add_zero_r(a)), Equal.cong(Nat, Nat, z => T5(a, b, z, d, e), c, Nat.add(c, 0n), Equal.sym(Nat, Nat.add(c, 0n), c, add_zero_r(c))))
    case 1n+p:
      Equal.trans(Nat, T5(Nat.add(a, 1n+p), b, c, d, e), T5(1n+Nat.add(a, p), b, c, d, e), T5(a, b, Nat.add(c, 1n+p), d, e), Equal.cong(Nat, Nat, z => T5(z, b, c, d, e), Nat.add(a, 1n+p), 1n+Nat.add(a, p), add_sr(a, p)), Equal.trans(Nat, T5(1n+Nat.add(a, p), b, c, d, e), T5(a, b, 1n+Nat.add(c, p), d, e), T5(a, b, Nat.add(c, 1n+p), d, e), Equal.trans(Nat, T5(1n+Nat.add(a, p), b, c, d, e), 1n+T5(a, b, Nat.add(c, p), d, e), T5(a, b, 1n+Nat.add(c, p), d, e), Equal.cong(Nat, Nat, z => 1n+z, T5(Nat.add(a, p), b, c, d, e), T5(a, b, Nat.add(c, p), d, e), mv13(a, p, b, c, d, e)), Equal.sym(Nat, T5(a, b, 1n+Nat.add(c, p), d, e), 1n+T5(a, b, Nat.add(c, p), d, e), t3(a, b, Nat.add(c, p), d, e))), Equal.cong(Nat, Nat, z => T5(a, b, z, d, e), 1n+Nat.add(c, p), Nat.add(c, 1n+p), Equal.sym(Nat, Nat.add(c, 1n+p), 1n+Nat.add(c, p), add_sr(c, p)))))

law mv15:
  for +a: Nat
  for +m: Nat
  for +b: Nat
  for +c: Nat
  for +d: Nat
  for +e: Nat
  {T5(Nat.add(a, m), b, c, d, e) == T5(a, b, c, d, Nat.add(e, m)) : Nat}

def mv15(a, m, b, c, d, e):
  match m:
    case 0n:
      Equal.trans(Nat, T5(Nat.add(a, 0n), b, c, d, e), T5(a, b, c, d, e), T5(a, b, c, d, Nat.add(e, 0n)), Equal.cong(Nat, Nat, z => T5(z, b, c, d, e), Nat.add(a, 0n), a, add_zero_r(a)), Equal.cong(Nat, Nat, z => T5(a, b, c, d, z), e, Nat.add(e, 0n), Equal.sym(Nat, Nat.add(e, 0n), e, add_zero_r(e))))
    case 1n+p:
      Equal.trans(Nat, T5(Nat.add(a, 1n+p), b, c, d, e), T5(1n+Nat.add(a, p), b, c, d, e), T5(a, b, c, d, Nat.add(e, 1n+p)), Equal.cong(Nat, Nat, z => T5(z, b, c, d, e), Nat.add(a, 1n+p), 1n+Nat.add(a, p), add_sr(a, p)), Equal.trans(Nat, T5(1n+Nat.add(a, p), b, c, d, e), T5(a, b, c, d, 1n+Nat.add(e, p)), T5(a, b, c, d, Nat.add(e, 1n+p)), Equal.trans(Nat, T5(1n+Nat.add(a, p), b, c, d, e), 1n+T5(a, b, c, d, Nat.add(e, p)), T5(a, b, c, d, 1n+Nat.add(e, p)), Equal.cong(Nat, Nat, z => 1n+z, T5(Nat.add(a, p), b, c, d, e), T5(a, b, c, d, Nat.add(e, p)), mv15(a, p, b, c, d, e)), Equal.sym(Nat, T5(a, b, c, d, 1n+Nat.add(e, p)), 1n+T5(a, b, c, d, Nat.add(e, p)), t5(a, b, c, d, Nat.add(e, p)))), Equal.cong(Nat, Nat, z => T5(a, b, c, d, z), 1n+Nat.add(e, p), Nat.add(e, 1n+p), Equal.sym(Nat, Nat.add(e, 1n+p), 1n+Nat.add(e, p), add_sr(e, p)))))

law mv35:
  for +c: Nat
  for +m: Nat
  for +a: Nat
  for +b: Nat
  for +d: Nat
  for +e: Nat
  {T5(a, b, Nat.add(c, m), d, e) == T5(a, b, c, d, Nat.add(e, m)) : Nat}

def mv35(c, m, a, b, d, e):
  match m:
    case 0n:
      Equal.trans(Nat, T5(a, b, Nat.add(c, 0n), d, e), T5(a, b, c, d, e), T5(a, b, c, d, Nat.add(e, 0n)), Equal.cong(Nat, Nat, z => T5(a, b, z, d, e), Nat.add(c, 0n), c, add_zero_r(c)), Equal.cong(Nat, Nat, z => T5(a, b, c, d, z), e, Nat.add(e, 0n), Equal.sym(Nat, Nat.add(e, 0n), e, add_zero_r(e))))
    case 1n+p:
      Equal.trans(Nat, T5(a, b, Nat.add(c, 1n+p), d, e), T5(a, b, 1n+Nat.add(c, p), d, e), T5(a, b, c, d, Nat.add(e, 1n+p)), Equal.cong(Nat, Nat, z => T5(a, b, z, d, e), Nat.add(c, 1n+p), 1n+Nat.add(c, p), add_sr(c, p)), Equal.trans(Nat, T5(a, b, 1n+Nat.add(c, p), d, e), T5(a, b, c, d, 1n+Nat.add(e, p)), T5(a, b, c, d, Nat.add(e, 1n+p)), Equal.trans(Nat, T5(a, b, 1n+Nat.add(c, p), d, e), 1n+T5(a, b, Nat.add(c, p), d, e), T5(a, b, c, d, 1n+Nat.add(e, p)), t3(a, b, Nat.add(c, p), d, e), Equal.trans(Nat, 1n+T5(a, b, Nat.add(c, p), d, e), 1n+T5(a, b, c, d, Nat.add(e, p)), T5(a, b, c, d, 1n+Nat.add(e, p)), Equal.cong(Nat, Nat, z => 1n+z, T5(a, b, Nat.add(c, p), d, e), T5(a, b, c, d, Nat.add(e, p)), mv35(c, p, a, b, d, e)), Equal.sym(Nat, T5(a, b, c, d, 1n+Nat.add(e, p)), 1n+T5(a, b, c, d, Nat.add(e, p)), t5(a, b, c, d, Nat.add(e, p))))), Equal.cong(Nat, Nat, z => T5(a, b, c, d, z), 1n+Nat.add(e, p), Nat.add(e, 1n+p), Equal.sym(Nat, Nat.add(e, 1n+p), 1n+Nat.add(e, p), add_sr(e, p)))))

law len_snoc:
  for +xs: +List<Nat>
  for +x: Nat
  {S.len(M.snoc(xs, x)) == 1n+S.len(xs) : Nat}

def len_snoc(xs, x):
  match xs:
    case Nil{}:
      {==}
    case Con{+y, rest}:
      Equal.cong(Nat, Nat, z => 1n+z, S.len(M.snoc(rest, x)), 1n+S.len(rest), len_snoc(rest, x))

law len_app:
  for +a: +List<Nat>
  for +b: +List<Nat>
  {S.len(List.append(&2, Nat, a, b)) == Nat.add(S.len(a), S.len(b)) : Nat}

def len_app(a, b):
  match a:
    case Nil{}:
      {==}
    case Con{+y, rest}:
      Equal.cong(Nat, Nat, z => 1n+z, S.len(List.append(&2, Nat, rest, b)), Nat.add(S.len(rest), S.len(b)), len_app(rest, b))

law la_step:
  for b: Bool
  for +y: Nat
  for +tl: +List<Nat>
  for +rest: +List<Nat>
  for +hb: {b == False{} : Bool}
  for +ih: {S.len(tl) == S.len(rest) : Nat}
  {S.len(M.remove_step(b, y, tl)) == 1n+S.len(rest) : Nat}

def la_step(b, y, tl, rest, hb, ih):
  match b:
    case True{}:
      Empty.absurd({S.len(M.remove_step(True{}, y, tl)) == 1n+S.len(rest) : Nat}, tt_ff(hb))
    case False{}:
      Equal.cong(Nat, Nat, z => 1n+z, S.len(tl), S.len(rest), ih)

law len_abs:
  for +x: Nat
  for +xs: +List<Nat>
  for +h: {Bool.not(M.mem(x, xs)) == True{} : Bool}
  {S.len(M.remove(x, xs)) == S.len(xs) : Nat}

def len_abs(x, xs, h):
  match xs:
    case Nil{}:
      {==}
    case Con{+y, rest}:
      la_step(Nat.is_eq(x, y), y, M.remove(x, rest), rest, nor_l(Nat.is_eq(x, y), M.mem(x, rest), h), len_abs(x, rest, nor_r(Nat.is_eq(x, y), M.mem(x, rest), h)))

law abs_of_eq:
  for +x: Nat
  for +y: Nat
  for +rest: +List<Nat>
  for +k: {Nat.is_eq(x, y) == True{} : Bool}
  for +hn: {Bool.not(M.mem(y, rest)) == True{} : Bool}
  {Bool.not(M.mem(x, rest)) == True{} : Bool}

def abs_of_eq(x, y, rest, k, hn):
  %Equal.sym(Nat, x, y, eq_sound(x, y, k)) : {Bool.not(M.mem(_, rest)) == True{} : Bool}
  hn

law lr_step:
  for b: Bool
  for +x: Nat
  for +y: Nat
  for +rest: +List<Nat>
  for +hb: {Nat.is_eq(x, y) == b : Bool}
  for +hn: {Bool.not(M.mem(y, rest)) == True{} : Bool}
  for +m: {Bool.or(b, M.mem(x, rest)) == True{} : Bool}
  for ih: @k: {M.mem(x, rest) == True{} : Bool} -> {S.len(rest) == 1n+S.len(M.remove(x, rest)) : Nat}
  {S.len(y <> rest) == 1n+S.len(M.remove_step(b, y, M.remove(x, rest))) : Nat}

def lr_step(b, x, y, rest, hb, hn, m, ih):
  match b:
    case True{}:
      Equal.cong(Nat, Nat, z => 1n+z, S.len(rest), S.len(M.remove(x, rest)), Equal.sym(Nat, S.len(M.remove(x, rest)), S.len(rest), len_abs(x, rest, abs_of_eq(x, y, rest, hb, hn))))
    case False{}:
      Equal.cong(Nat, Nat, z => 1n+z, S.len(rest), 1n+S.len(M.remove(x, rest)), ih(m))

law len_remove:
  for +x: Nat
  for +xs: +List<Nat>
  for +nd: {S.nodup(xs) == True{} : Bool}
  for +m: {M.mem(x, xs) == True{} : Bool}
  {S.len(xs) == 1n+S.len(M.remove(x, xs)) : Nat}

def len_remove(x, xs, nd, m):
  match xs:
    case Nil{}:
      Empty.absurd({0n == 1n+0n : Nat}, ff_tt(m))
    case Con{+y, rest}:
      lr_step(Nat.is_eq(x, y), x, y, rest, {==}, and_l(Bool.not(M.mem(y, rest)), S.nodup(rest), nd), m, k => len_remove(x, rest, and_r(Bool.not(M.mem(y, rest)), S.nodup(rest), nd), k))

law kd_step:
  for b: Bool
  for +x: Nat
  for +kk: +List<Nat>
  for +dd: +List<Nat>
  for +rest: +List<Nat>
  for +ih: {Nat.add(S.len(kk), S.len(dd)) == S.len(rest) : Nat}
  {Nat.add(S.len(M.keep_step(b, x, kk)), S.len(M.drop_step(b, x, dd))) == 1n+S.len(rest) : Nat}

def kd_step(b, x, kk, dd, rest, ih):
  match b:
    case True{}:
      Equal.cong(Nat, Nat, z => 1n+z, Nat.add(S.len(kk), S.len(dd)), S.len(rest), ih)
    case False{}:
      Equal.trans(Nat, Nat.add(S.len(kk), 1n+S.len(dd)), 1n+Nat.add(S.len(kk), S.len(dd)), 1n+S.len(rest), add_sr(S.len(kk), S.len(dd)), Equal.cong(Nat, Nat, z => 1n+z, Nat.add(S.len(kk), S.len(dd)), S.len(rest), ih))

law len_kd:
  for +q: +List<Nat>
  for +u: +List<Nat>
  {Nat.add(S.len(M.keep_users(q, u)), S.len(M.drop_users(q, u))) == S.len(q) : Nat}

def len_kd(q, u):
  match q:
    case Nil{}:
      {==}
    case Con{+x, rest}:
      kd_step(M.mem(x, u), x, M.keep_users(rest, u), M.drop_users(rest, u), rest, len_kd(rest, u))

law t_ex:
  for +a: Nat
  for +b: Nat
  for +c: Nat
  for +d: Nat
  for +e: Nat
  {T5(a, b, c, d, e) == T5(0n, b, 0n, d, Nat.add(e, Nat.add(a, c))) : Nat}

def t_ex(a, b, c, d, e):
  Equal.trans(Nat, T5(a, b, c, d, e), T5(0n, b, c, d, Nat.add(e, a)), T5(0n, b, 0n, d, Nat.add(e, Nat.add(a, c))), mv15(0n, a, b, c, d, e), Equal.trans(Nat, T5(0n, b, c, d, Nat.add(e, a)), T5(0n, b, 0n, d, Nat.add(Nat.add(e, a), c)), T5(0n, b, 0n, d, Nat.add(e, Nat.add(a, c))), mv35(0n, c, 0n, b, d, Nat.add(e, a)), Equal.cong(Nat, Nat, z => T5(0n, b, 0n, d, z), Nat.add(Nat.add(e, a), c), Nat.add(e, Nat.add(a, c)), add_assoc(e, a, c))))

def S6(+q: +List<Nat>, +h: +List<Nat>, +k: +List<Nat>, +t: +List<Nat>, +d: +List<Nat>, +n: Nat) -> Data:
  {Nat.add(1n, T5(S.len(q), S.len(h), S.len(k), S.len(t), S.len(d))) == n : Nat}

def S6W(s: M.World) -> Data:
  match s:
    case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
      S6(q, haf, htk, t, d, n)

law s6_to:
  for +q: +List<Nat>
  for +h: +List<Nat>
  for +k: +List<Nat>
  for +t: +List<Nat>
  for +d: +List<Nat>
  for +n: Nat
  for +q2: +List<Nat>
  for +h2: +List<Nat>
  for +k2: +List<Nat>
  for +t2: +List<Nat>
  for +d2: +List<Nat>
  for +eq: {T5(S.len(q2), S.len(h2), S.len(k2), S.len(t2), S.len(d2)) == T5(S.len(q), S.len(h), S.len(k), S.len(t), S.len(d)) : Nat}
  for +Z: S6(q, h, k, t, d, n)
  S6(q2, h2, k2, t2, d2, n)

def s6_to(q, h, k, t, d, n, q2, h2, k2, t2, d2, eq, Z):
  Equal.trans(Nat, 1n+T5(S.len(q2), S.len(h2), S.len(k2), S.len(t2), S.len(d2)), 1n+T5(S.len(q), S.len(h), S.len(k), S.len(t), S.len(d)), n, Equal.cong(Nat, Nat, z => 1n+z, T5(S.len(q2), S.len(h2), S.len(k2), S.len(t2), S.len(d2)), T5(S.len(q), S.len(h), S.len(k), S.len(t), S.len(d)), eq), Z)

law s6_up:
  for +q: +List<Nat>
  for +h: +List<Nat>
  for +k: +List<Nat>
  for +t: +List<Nat>
  for +d: +List<Nat>
  for +n: Nat
  for +q2: +List<Nat>
  for +h2: +List<Nat>
  for +k2: +List<Nat>
  for +t2: +List<Nat>
  for +d2: +List<Nat>
  for +eq: {T5(S.len(q2), S.len(h2), S.len(k2), S.len(t2), S.len(d2)) == 1n+T5(S.len(q), S.len(h), S.len(k), S.len(t), S.len(d)) : Nat}
  for +Z: S6(q, h, k, t, d, n)
  S6(q2, h2, k2, t2, d2, 1n+n)

def s6_up(q, h, k, t, d, n, q2, h2, k2, t2, d2, eq, Z):
  Equal.trans(Nat, 1n+T5(S.len(q2), S.len(h2), S.len(k2), S.len(t2), S.len(d2)), 1n+(1n+T5(S.len(q), S.len(h), S.len(k), S.len(t), S.len(d))), 1n+n, Equal.cong(Nat, Nat, z => 1n+z, T5(S.len(q2), S.len(h2), S.len(k2), S.len(t2), S.len(d2)), 1n+T5(S.len(q), S.len(h), S.len(k), S.len(t), S.len(d)), eq), Equal.cong(Nat, Nat, z => 1n+z, 1n+T5(S.len(q), S.len(h), S.len(k), S.len(t), S.len(d)), n, Z))

law s6_init:
  S6W(M.init())

def s6_init():
  {==}

law s6_flush_c:
  for +x: Nat
  for +rest: +List<Nat>
  for +h: +List<Nat>
  for +k: +List<Nat>
  for +t: +List<Nat>
  for +d: +List<Nat>
  for +n: Nat
  for +Z: S6(x <> rest, h, k, t, d, n)
  S6(rest, h, k, M.snoc(t, x), d, n)

def s6_flush_c(x, rest, h, k, t, d, n, Z):
  s6_to(x <> rest, h, k, t, d, n, rest, h, k, M.snoc(t, x), d, Equal.trans(Nat, T5(S.len(rest), S.len(h), S.len(k), S.len(M.snoc(t, x)), S.len(d)), T5(S.len(rest), S.len(h), S.len(k), 1n+S.len(t), S.len(d)), T5(S.len(x <> rest), S.len(h), S.len(k), S.len(t), S.len(d)), Equal.cong(Nat, Nat, z => T5(S.len(rest), S.len(h), S.len(k), z, S.len(d)), S.len(M.snoc(t, x)), 1n+S.len(t), len_snoc(t, x)), t4(S.len(rest), S.len(h), S.len(k), S.len(t), S.len(d))), Z)

law s6_flush:
  for +s: M.World
  for +Z: S6W(s)
  S6W(M.flush(s))

def s6_flush(s, Z):
  match s:
    case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
      match q i:
        case Nil{} False{}:
          Z
        case Nil{} True{}:
          Z
        case Con{+x, rest} False{}:
          Z
        case Con{+x, rest} True{}:
          s6_flush_c(x, rest, haf, htk, t, d, n, Z)

law s6_snoc_q:
  for +q: +List<Nat>
  for +h: +List<Nat>
  for +k: +List<Nat>
  for +t: +List<Nat>
  for +d: +List<Nat>
  for +n: Nat
  for +Z: S6(q, h, k, t, d, n)
  S6(M.snoc(q, n), h, k, t, d, 1n+n)

def s6_snoc_q(q, h, k, t, d, n, Z):
  s6_up(q, h, k, t, d, n, M.snoc(q, n), h, k, t, d, Equal.cong(Nat, Nat, z => T5(z, S.len(h), S.len(k), S.len(t), S.len(d)), S.len(M.snoc(q, n)), 1n+S.len(q), len_snoc(q, n)), Z)

law s6_snoc_h:
  for +q: +List<Nat>
  for +h: +List<Nat>
  for +k: +List<Nat>
  for +t: +List<Nat>
  for +d: +List<Nat>
  for +n: Nat
  for +Z: S6(q, h, k, t, d, n)
  S6(q, M.snoc(h, n), k, t, d, 1n+n)

def s6_snoc_h(q, h, k, t, d, n, Z):
  s6_up(q, h, k, t, d, n, q, M.snoc(h, n), k, t, d, Equal.trans(Nat, T5(S.len(q), S.len(M.snoc(h, n)), S.len(k), S.len(t), S.len(d)), T5(S.len(q), 1n+S.len(h), S.len(k), S.len(t), S.len(d)), 1n+T5(S.len(q), S.len(h), S.len(k), S.len(t), S.len(d)), Equal.cong(Nat, Nat, z => T5(S.len(q), z, S.len(k), S.len(t), S.len(d)), S.len(M.snoc(h, n)), 1n+S.len(h), len_snoc(h, n)), t2(S.len(q), S.len(h), S.len(k), S.len(t), S.len(d))), Z)

law s6_snoc_k:
  for +q: +List<Nat>
  for +h: +List<Nat>
  for +k: +List<Nat>
  for +t: +List<Nat>
  for +d: +List<Nat>
  for +n: Nat
  for +Z: S6(q, h, k, t, d, n)
  S6(q, h, M.snoc(k, n), t, d, 1n+n)

def s6_snoc_k(q, h, k, t, d, n, Z):
  s6_up(q, h, k, t, d, n, q, h, M.snoc(k, n), t, d, Equal.trans(Nat, T5(S.len(q), S.len(h), S.len(M.snoc(k, n)), S.len(t), S.len(d)), T5(S.len(q), S.len(h), 1n+S.len(k), S.len(t), S.len(d)), 1n+T5(S.len(q), S.len(h), S.len(k), S.len(t), S.len(d)), Equal.cong(Nat, Nat, z => T5(S.len(q), S.len(h), z, S.len(t), S.len(d)), S.len(M.snoc(k, n)), 1n+S.len(k), len_snoc(k, n)), t3(S.len(q), S.len(h), S.len(k), S.len(t), S.len(d))), Z)

law s6_snoc_d:
  for +q: +List<Nat>
  for +h: +List<Nat>
  for +k: +List<Nat>
  for +t: +List<Nat>
  for +d: +List<Nat>
  for +n: Nat
  for +Z: S6(q, h, k, t, d, n)
  S6(q, h, k, t, M.snoc(d, n), 1n+n)

def s6_snoc_d(q, h, k, t, d, n, Z):
  s6_up(q, h, k, t, d, n, q, h, k, t, M.snoc(d, n), Equal.trans(Nat, T5(S.len(q), S.len(h), S.len(k), S.len(t), S.len(M.snoc(d, n))), T5(S.len(q), S.len(h), S.len(k), S.len(t), 1n+S.len(d)), 1n+T5(S.len(q), S.len(h), S.len(k), S.len(t), S.len(d)), Equal.cong(Nat, Nat, z => T5(S.len(q), S.len(h), S.len(k), S.len(t), z), S.len(M.snoc(d, n)), 1n+S.len(d), len_snoc(d, n)), t5(S.len(q), S.len(h), S.len(k), S.len(t), S.len(d))), Z)

law s6_send_route:
  for fu: Bool
  for tk: Bool
  for +s: M.World
  for +Z: S6W(s)
  S6W(M.send_route(fu, tk, s))

def s6_send_route(fu, tk, s, Z):
  match fu:
    case True{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          s6_flush(M.W{M.snoc(q, n), haf, htk, t, d, M.snoc(u, n), tt, rl, 1n+n, tk, i, g}, s6_snoc_q(q, haf, htk, t, d, n, Z))
    case False{}:
      match tk:
        case True{}:
          match s:
            case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
              s6_snoc_k(q, haf, htk, t, d, n, Z)
        case False{}:
          match s:
            case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
              s6_flush(M.W{M.snoc(q, n), haf, htk, t, d, u, tt, rl, 1n+n, tk, i, g}, s6_snoc_q(q, haf, htk, t, d, n, Z))

law s6_send_auto:
  for fu: Bool
  for +s: M.World
  for +Z: S6W(s)
  S6W(M.send_auto(fu, s))

def s6_send_auto(fu, s, Z):
  match s:
    case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
      s6_send_route(fu, tk, M.W{q, haf, htk, t, d, u, tt, rl, n, tk, i, g}, Z)

law s6_exit:
  for +q: +List<Nat>
  for +h: +List<Nat>
  for +k: +List<Nat>
  for +t: +List<Nat>
  for +d: +List<Nat>
  for +n: Nat
  for +Z: S6(q, h, k, t, d, n)
  S6([], h, [], t, List.append(&2, Nat, d, List.append(&2, Nat, q, k)), n)

def s6_exit(q, h, k, t, d, n, Z):
  s6_to(q, h, k, t, d, n, [], h, [], t, List.append(&2, Nat, d, List.append(&2, Nat, q, k)), Equal.trans(Nat, T5(0n, S.len(h), 0n, S.len(t), S.len(List.append(&2, Nat, d, List.append(&2, Nat, q, k)))), T5(0n, S.len(h), 0n, S.len(t), Nat.add(S.len(d), Nat.add(S.len(q), S.len(k)))), T5(S.len(q), S.len(h), S.len(k), S.len(t), S.len(d)), Equal.cong(Nat, Nat, z => T5(0n, S.len(h), 0n, S.len(t), z), S.len(List.append(&2, Nat, d, List.append(&2, Nat, q, k))), Nat.add(S.len(d), Nat.add(S.len(q), S.len(k))), Equal.trans(Nat, S.len(List.append(&2, Nat, d, List.append(&2, Nat, q, k))), Nat.add(S.len(d), S.len(List.append(&2, Nat, q, k))), Nat.add(S.len(d), Nat.add(S.len(q), S.len(k))), len_app(d, List.append(&2, Nat, q, k)), Equal.cong(Nat, Nat, z => Nat.add(S.len(d), z), S.len(List.append(&2, Nat, q, k)), Nat.add(S.len(q), S.len(k)), len_app(q, k)))), Equal.sym(Nat, T5(S.len(q), S.len(h), S.len(k), S.len(t), S.len(d)), T5(0n, S.len(h), 0n, S.len(t), Nat.add(S.len(d), Nat.add(S.len(q), S.len(k)))), t_ex(S.len(q), S.len(h), S.len(k), S.len(t), S.len(d)))), Z)

law s6_tb:
  for +q: +List<Nat>
  for +h: +List<Nat>
  for +k: +List<Nat>
  for +t: +List<Nat>
  for +d: +List<Nat>
  for +n: Nat
  for +u: +List<Nat>
  for +Z: S6(q, h, k, t, d, n)
  S6(M.keep_users(q, u), h, List.append(&2, Nat, k, M.drop_users(q, u)), t, d, n)

def s6_tb(q, h, k, t, d, n, u, Z):
  s6_to(q, h, k, t, d, n, M.keep_users(q, u), h, List.append(&2, Nat, k, M.drop_users(q, u)), t, d, Equal.trans(Nat, T5(S.len(M.keep_users(q, u)), S.len(h), S.len(List.append(&2, Nat, k, M.drop_users(q, u))), S.len(t), S.len(d)), T5(Nat.add(S.len(M.keep_users(q, u)), S.len(M.drop_users(q, u))), S.len(h), S.len(k), S.len(t), S.len(d)), T5(S.len(q), S.len(h), S.len(k), S.len(t), S.len(d)), Equal.trans(Nat, T5(S.len(M.keep_users(q, u)), S.len(h), S.len(List.append(&2, Nat, k, M.drop_users(q, u))), S.len(t), S.len(d)), T5(S.len(M.keep_users(q, u)), S.len(h), Nat.add(S.len(k), S.len(M.drop_users(q, u))), S.len(t), S.len(d)), T5(Nat.add(S.len(M.keep_users(q, u)), S.len(M.drop_users(q, u))), S.len(h), S.len(k), S.len(t), S.len(d)), Equal.cong(Nat, Nat, z => T5(S.len(M.keep_users(q, u)), S.len(h), z, S.len(t), S.len(d)), S.len(List.append(&2, Nat, k, M.drop_users(q, u))), Nat.add(S.len(k), S.len(M.drop_users(q, u))), len_app(k, M.drop_users(q, u))), Equal.sym(Nat, T5(Nat.add(S.len(M.keep_users(q, u)), S.len(M.drop_users(q, u))), S.len(h), S.len(k), S.len(t), S.len(d)), T5(S.len(M.keep_users(q, u)), S.len(h), Nat.add(S.len(k), S.len(M.drop_users(q, u))), S.len(t), S.len(d)), mv13(S.len(M.keep_users(q, u)), S.len(M.drop_users(q, u)), S.len(h), S.len(k), S.len(t), S.len(d)))), Equal.cong(Nat, Nat, z => T5(z, S.len(h), S.len(k), S.len(t), S.len(d)), Nat.add(S.len(M.keep_users(q, u)), S.len(M.drop_users(q, u))), S.len(q), len_kd(q, u))), Z)

law s6_te:
  for +q: +List<Nat>
  for +h: +List<Nat>
  for +k: +List<Nat>
  for +t: +List<Nat>
  for +d: +List<Nat>
  for +n: Nat
  for +Z: S6(q, h, k, t, d, n)
  S6(List.append(&2, Nat, q, k), h, [], t, d, n)

def s6_te(q, h, k, t, d, n, Z):
  s6_to(q, h, k, t, d, n, List.append(&2, Nat, q, k), h, [], t, d, Equal.trans(Nat, T5(S.len(List.append(&2, Nat, q, k)), S.len(h), 0n, S.len(t), S.len(d)), T5(Nat.add(S.len(q), S.len(k)), S.len(h), 0n, S.len(t), S.len(d)), T5(S.len(q), S.len(h), S.len(k), S.len(t), S.len(d)), Equal.cong(Nat, Nat, z => T5(z, S.len(h), 0n, S.len(t), S.len(d)), S.len(List.append(&2, Nat, q, k)), Nat.add(S.len(q), S.len(k)), len_app(q, k)), mv13(S.len(q), S.len(k), S.len(h), 0n, S.len(t), S.len(d))), Z)

law s6_dif:
  for +q: +List<Nat>
  for +h: +List<Nat>
  for +k: +List<Nat>
  for +t: +List<Nat>
  for +d: +List<Nat>
  for +n: Nat
  for +id: Nat
  for +nd: {S.nodup(h) == True{} : Bool}
  for +m: {M.mem(id, h) == True{} : Bool}
  for +Z: S6(q, h, k, t, d, n)
  S6(M.snoc(q, id), M.remove(id, h), k, t, d, n)

def s6_dif(q, h, k, t, d, n, id, nd, m, Z):
  s6_to(q, h, k, t, d, n, M.snoc(q, id), M.remove(id, h), k, t, d, Equal.trans(Nat, T5(S.len(M.snoc(q, id)), S.len(M.remove(id, h)), S.len(k), S.len(t), S.len(d)), T5(1n+S.len(q), S.len(M.remove(id, h)), S.len(k), S.len(t), S.len(d)), T5(S.len(q), S.len(h), S.len(k), S.len(t), S.len(d)), Equal.cong(Nat, Nat, z => T5(z, S.len(M.remove(id, h)), S.len(k), S.len(t), S.len(d)), S.len(M.snoc(q, id)), 1n+S.len(q), len_snoc(q, id)), Equal.sym(Nat, T5(S.len(q), S.len(h), S.len(k), S.len(t), S.len(d)), T5(1n+S.len(q), S.len(M.remove(id, h)), S.len(k), S.len(t), S.len(d)), Equal.trans(Nat, T5(S.len(q), S.len(h), S.len(k), S.len(t), S.len(d)), T5(S.len(q), 1n+S.len(M.remove(id, h)), S.len(k), S.len(t), S.len(d)), T5(1n+S.len(q), S.len(M.remove(id, h)), S.len(k), S.len(t), S.len(d)), Equal.cong(Nat, Nat, z => T5(S.len(q), z, S.len(k), S.len(t), S.len(d)), S.len(h), 1n+S.len(M.remove(id, h)), len_remove(id, h, nd, m)), t2(S.len(q), S.len(M.remove(id, h)), S.len(k), S.len(t), S.len(d))))), Z)

law s6_dtk:
  for +q: +List<Nat>
  for +h: +List<Nat>
  for +k: +List<Nat>
  for +t: +List<Nat>
  for +d: +List<Nat>
  for +n: Nat
  for +id: Nat
  for +nd: {S.nodup(k) == True{} : Bool}
  for +m: {M.mem(id, k) == True{} : Bool}
  for +Z: S6(q, h, k, t, d, n)
  S6(M.snoc(q, id), h, M.remove(id, k), t, d, n)

def s6_dtk(q, h, k, t, d, n, id, nd, m, Z):
  s6_to(q, h, k, t, d, n, M.snoc(q, id), h, M.remove(id, k), t, d, Equal.trans(Nat, T5(S.len(M.snoc(q, id)), S.len(h), S.len(M.remove(id, k)), S.len(t), S.len(d)), T5(1n+S.len(q), S.len(h), S.len(M.remove(id, k)), S.len(t), S.len(d)), T5(S.len(q), S.len(h), S.len(k), S.len(t), S.len(d)), Equal.cong(Nat, Nat, z => T5(z, S.len(h), S.len(M.remove(id, k)), S.len(t), S.len(d)), S.len(M.snoc(q, id)), 1n+S.len(q), len_snoc(q, id)), Equal.sym(Nat, T5(S.len(q), S.len(h), S.len(k), S.len(t), S.len(d)), T5(1n+S.len(q), S.len(h), S.len(M.remove(id, k)), S.len(t), S.len(d)), Equal.trans(Nat, T5(S.len(q), S.len(h), S.len(k), S.len(t), S.len(d)), T5(S.len(q), S.len(h), 1n+S.len(M.remove(id, k)), S.len(t), S.len(d)), T5(1n+S.len(q), S.len(h), S.len(M.remove(id, k)), S.len(t), S.len(d)), Equal.cong(Nat, Nat, z => T5(S.len(q), S.len(h), z, S.len(t), S.len(d)), S.len(k), 1n+S.len(M.remove(id, k)), len_remove(id, k, nd, m)), t3(S.len(q), S.len(h), S.len(M.remove(id, k)), S.len(t), S.len(d))))), Z)

law s6_drh:
  for +q: +List<Nat>
  for +h: +List<Nat>
  for +k: +List<Nat>
  for +t: +List<Nat>
  for +d: +List<Nat>
  for +n: Nat
  for +id: Nat
  for +nd: {S.nodup(h) == True{} : Bool}
  for +m: {M.mem(id, h) == True{} : Bool}
  for +Z: S6(q, h, k, t, d, n)
  S6(q, M.remove(id, h), k, t, M.snoc(d, id), n)

def s6_drh(q, h, k, t, d, n, id, nd, m, Z):
  s6_to(q, h, k, t, d, n, q, M.remove(id, h), k, t, M.snoc(d, id), Equal.trans(Nat, T5(S.len(q), S.len(M.remove(id, h)), S.len(k), S.len(t), S.len(M.snoc(d, id))), 1n+T5(S.len(q), S.len(M.remove(id, h)), S.len(k), S.len(t), S.len(d)), T5(S.len(q), S.len(h), S.len(k), S.len(t), S.len(d)), Equal.trans(Nat, T5(S.len(q), S.len(M.remove(id, h)), S.len(k), S.len(t), S.len(M.snoc(d, id))), T5(S.len(q), S.len(M.remove(id, h)), S.len(k), S.len(t), 1n+S.len(d)), 1n+T5(S.len(q), S.len(M.remove(id, h)), S.len(k), S.len(t), S.len(d)), Equal.cong(Nat, Nat, z => T5(S.len(q), S.len(M.remove(id, h)), S.len(k), S.len(t), z), S.len(M.snoc(d, id)), 1n+S.len(d), len_snoc(d, id)), t5(S.len(q), S.len(M.remove(id, h)), S.len(k), S.len(t), S.len(d))), Equal.sym(Nat, T5(S.len(q), S.len(h), S.len(k), S.len(t), S.len(d)), 1n+T5(S.len(q), S.len(M.remove(id, h)), S.len(k), S.len(t), S.len(d)), Equal.trans(Nat, T5(S.len(q), S.len(h), S.len(k), S.len(t), S.len(d)), T5(S.len(q), 1n+S.len(M.remove(id, h)), S.len(k), S.len(t), S.len(d)), 1n+T5(S.len(q), S.len(M.remove(id, h)), S.len(k), S.len(t), S.len(d)), Equal.cong(Nat, Nat, z => T5(S.len(q), z, S.len(k), S.len(t), S.len(d)), S.len(h), 1n+S.len(M.remove(id, h)), len_remove(id, h, nd, m)), t2(S.len(q), S.len(M.remove(id, h)), S.len(k), S.len(t), S.len(d))))), Z)

law s6_drk:
  for +q: +List<Nat>
  for +h: +List<Nat>
  for +k: +List<Nat>
  for +t: +List<Nat>
  for +d: +List<Nat>
  for +n: Nat
  for +id: Nat
  for +nd: {S.nodup(k) == True{} : Bool}
  for +m: {M.mem(id, k) == True{} : Bool}
  for +Z: S6(q, h, k, t, d, n)
  S6(q, h, M.remove(id, k), t, M.snoc(d, id), n)

def s6_drk(q, h, k, t, d, n, id, nd, m, Z):
  s6_to(q, h, k, t, d, n, q, h, M.remove(id, k), t, M.snoc(d, id), Equal.trans(Nat, T5(S.len(q), S.len(h), S.len(M.remove(id, k)), S.len(t), S.len(M.snoc(d, id))), 1n+T5(S.len(q), S.len(h), S.len(M.remove(id, k)), S.len(t), S.len(d)), T5(S.len(q), S.len(h), S.len(k), S.len(t), S.len(d)), Equal.trans(Nat, T5(S.len(q), S.len(h), S.len(M.remove(id, k)), S.len(t), S.len(M.snoc(d, id))), T5(S.len(q), S.len(h), S.len(M.remove(id, k)), S.len(t), 1n+S.len(d)), 1n+T5(S.len(q), S.len(h), S.len(M.remove(id, k)), S.len(t), S.len(d)), Equal.cong(Nat, Nat, z => T5(S.len(q), S.len(h), S.len(M.remove(id, k)), S.len(t), z), S.len(M.snoc(d, id)), 1n+S.len(d), len_snoc(d, id)), t5(S.len(q), S.len(h), S.len(M.remove(id, k)), S.len(t), S.len(d))), Equal.sym(Nat, T5(S.len(q), S.len(h), S.len(k), S.len(t), S.len(d)), 1n+T5(S.len(q), S.len(h), S.len(M.remove(id, k)), S.len(t), S.len(d)), Equal.trans(Nat, T5(S.len(q), S.len(h), S.len(k), S.len(t), S.len(d)), T5(S.len(q), S.len(h), 1n+S.len(M.remove(id, k)), S.len(t), S.len(d)), 1n+T5(S.len(q), S.len(h), S.len(M.remove(id, k)), S.len(t), S.len(d)), Equal.cong(Nat, Nat, z => T5(S.len(q), S.len(h), z, S.len(t), S.len(d)), S.len(k), 1n+S.len(M.remove(id, k)), len_remove(id, k, nd, m)), t3(S.len(q), S.len(h), S.len(M.remove(id, k)), S.len(t), S.len(d))))), Z)

law s6_deliver_if:
  for +id: Nat
  for +s: M.World
  for +I: InvW(s)
  for +Z: S6W(s)
  for +m: {M.mem(id, S.held_af(s)) == True{} : Bool}
  S6W(M.deliver_if(True{}, id, s))

def s6_deliver_if(id, s, I, Z, m):
  match s:
    case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
      s6_flush(M.W{M.snoc(q, id), M.remove(id, haf), htk, t, d, u, tt, M.snoc(rl, id), n, tk, i, g}, s6_dif(q, haf, htk, t, d, n, id, pnh(q, haf, htk, t, d, n, I), m, Z))

law s6_deliver_tk:
  for b: Bool
  for +id: Nat
  for +s: M.World
  for +I: InvW(s)
  for +Z: S6W(s)
  for hb: @k: {b == True{} : Bool} -> {M.mem(id, S.held_tk(s)) == True{} : Bool}
  S6W(M.deliver_tk(b, id, s))

def s6_deliver_tk(b, id, s, I, Z, hb):
  match b:
    case False{}:
      Z
    case True{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          s6_flush(M.W{M.snoc(q, id), haf, M.remove(id, htk), t, d, u, tt, M.snoc(rl, id), n, tk, i, g}, s6_dtk(q, haf, htk, t, d, n, id, pnk(q, haf, htk, t, d, n, I), hb({==}), Z))

law s6_deliver_pick:
  for a: Bool
  for b: Bool
  for +id: Nat
  for +s: M.World
  for +I: InvW(s)
  for +Z: S6W(s)
  for ha: @k: {a == True{} : Bool} -> {M.mem(id, S.held_af(s)) == True{} : Bool}
  for hb: @k: {b == True{} : Bool} -> {M.mem(id, S.held_tk(s)) == True{} : Bool}
  S6W(M.deliver_pick(a, b, id, s))

def s6_deliver_pick(a, b, id, s, I, Z, ha, hb):
  match a:
    case True{}:
      s6_deliver_if(id, s, I, Z, ha({==}))
    case False{}:
      s6_deliver_tk(b, id, s, I, Z, hb)

law s6_drop_if:
  for +id: Nat
  for +s: M.World
  for +I: InvW(s)
  for +Z: S6W(s)
  for +m: {M.mem(id, S.held_af(s)) == True{} : Bool}
  S6W(M.drop_if(True{}, id, s))

def s6_drop_if(id, s, I, Z, m):
  match s:
    case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
      s6_drh(q, haf, htk, t, d, n, id, pnh(q, haf, htk, t, d, n, I), m, Z)

law s6_drop_tk:
  for b: Bool
  for +id: Nat
  for +s: M.World
  for +I: InvW(s)
  for +Z: S6W(s)
  for hb: @k: {b == True{} : Bool} -> {M.mem(id, S.held_tk(s)) == True{} : Bool}
  S6W(M.drop_tk(b, id, s))

def s6_drop_tk(b, id, s, I, Z, hb):
  match b:
    case False{}:
      Z
    case True{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          s6_drk(q, haf, htk, t, d, n, id, pnk(q, haf, htk, t, d, n, I), hb({==}), Z)

law s6_drop_pick:
  for a: Bool
  for b: Bool
  for +id: Nat
  for +s: M.World
  for +I: InvW(s)
  for +Z: S6W(s)
  for ha: @k: {a == True{} : Bool} -> {M.mem(id, S.held_af(s)) == True{} : Bool}
  for hb: @k: {b == True{} : Bool} -> {M.mem(id, S.held_tk(s)) == True{} : Bool}
  S6W(M.drop_pick(a, b, id, s))

def s6_drop_pick(a, b, id, s, I, Z, ha, hb):
  match a:
    case True{}:
      s6_drop_if(id, s, I, Z, ha({==}))
    case False{}:
      s6_drop_tk(b, id, s, I, Z, hb)

law s6_step:
  for e: M.Ev
  for +s: M.World
  for +I: InvW(s)
  for +Z: S6W(s)
  S6W(M.step(e, s))

def s6_step(e, s, I, Z):
  match e:
    case M.Send{fu, r}:
      match r:
        case M.Auto{}:
          match s:
            case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
              match g:
                case True{}:
                  Z
                case False{}:
                  s6_send_auto(fu, M.W{q, haf, htk, t, d, u, tt, rl, n, tk, i, False{}}, Z)
        case M.AskFirst{}:
          match s:
            case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
              match g:
                case True{}:
                  Z
                case False{}:
                  s6_snoc_h(q, haf, htk, t, d, n, Z)
        case M.RouteDrop{}:
          match s:
            case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
              match g:
                case True{}:
                  Z
                case False{}:
                  s6_snoc_d(q, haf, htk, t, d, n, Z)
    case M.Idle{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          match g:
            case True{}:
              Z
            case False{}:
              s6_flush(M.W{q, haf, htk, t, d, u, tt, rl, n, tk, True{}, False{}}, Z)
    case M.Busy{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          Z
    case M.Exit{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          s6_exit(q, haf, htk, t, d, n, Z)
    case M.TakeoverBegin{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          s6_tb(q, haf, htk, t, d, n, u, Z)
    case M.TakeoverEnd{}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          s6_flush(M.W{List.append(&2, Nat, q, htk), haf, [], t, d, u, tt, rl, n, False{}, i, g}, s6_te(q, haf, htk, t, d, n, Z))
    case M.UserDeliver{+id}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          s6_deliver_pick(Bool.and(M.mem(id, haf), Bool.not(g)), Bool.and(M.mem(id, htk), Bool.not(g)), id, M.W{q, haf, htk, t, d, u, tt, rl, n, tk, i, g}, I, Z, k => and_l(M.mem(id, haf), Bool.not(g), k), k => and_l(M.mem(id, htk), Bool.not(g), k))
    case M.UserDrop{+id}:
      match s:
        case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
          s6_drop_pick(M.mem(id, haf), M.mem(id, htk), id, M.W{q, haf, htk, t, d, u, tt, rl, n, tk, i, g}, I, Z, k => k, k => k)

law m6_of:
  for +s: M.World
  for +Z: S6W(s)
  {Nat.add(1n, Nat.add(S.len(S.queue(s)), Nat.add(S.len(S.held_af(s)), Nat.add(S.len(S.held_tk(s)), Nat.add(S.len(S.typed(s)), S.len(S.dropped(s))))))) == S.next_id(s) : Nat}

def m6_of(s, Z):
  match s:
    case M.W{+q, +haf, +htk, +t, +d, +u, +tt, +rl, +n, +tk, +i, +g}:
      Z

law m6_run:
  for +evs: +List<M.Ev>
  for +s: M.World
  for +I: InvW(s)
  for +Z: S6W(s)
  {Nat.add(1n, Nat.add(S.len(S.queue(M.run(evs, s))), Nat.add(S.len(S.held_af(M.run(evs, s))), Nat.add(S.len(S.held_tk(M.run(evs, s))), Nat.add(S.len(S.typed(M.run(evs, s))), S.len(S.dropped(M.run(evs, s)))))))) == S.next_id(M.run(evs, s)) : Nat}

def m6_run(evs, s, I, Z):
  match evs:
    case Nil{}:
      m6_of(s, Z)
    case Con{e, rest}:
      m6_run(rest, M.step(e, s), invw_step(e, s, I), s6_step(e, s, I, Z))

def Laws.m6_no_silent_loss(evs):
  m6_run(evs, M.init(), invw_init(), s6_init())
