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

