import Base
import ./model.bend as M
import ./spec.bend as S

# Laws of crates/messages for one receiver, stated over every schedule (a List<Ev>) of model.bend's pure `step`.
# Human-written. A Builder never edits this file; a law may only become stronger.
# Not stated yet: M8 (escalation terminates) and M9 (restart is a replay) need the escalation chain and the store in the model.
# Stated in part only: M4 (the typed list only grows at its end; B7's ties-by-id and held-never-blocks are not), M5 (disjointness;
# that ids never change status and rise by one is only counted by M6).

# M1: a Message is typed at most once.
law m1_final_and_at_most_once:
  for evs: +List<M.Ev>
  {S.nodup(S.typed(S.world(evs))) == True{} : Bool}

# M2: a Message held for ask-first leaves `held` only by the user's deliver or drop.
law m2_held_leaves_only_by_its_owner:
  for evs: +List<M.Ev>
  for e: M.Ev
  {S.implies(Bool.not(S.is_user_call(e)), S.all_in(S.held_af(S.world(evs)), S.held_af(M.step(e, S.world(evs))))) == True{} : Bool}

# M3: nothing typed during a Takeover came from an Actor but the user, unless the user delivered it by hand (B6: the user's call wins).
law m3_takeover_isolates:
  for evs: +List<M.Ev>
  {S.all_in(S.tk_typed(S.world(evs)), List.append(&2, Nat, S.users(S.world(evs)), S.released(S.world(evs)))) == True{} : Bool}

# M4 (the part stated so far): the typed list only grows at its end, so no Message is typed out of order.
law m4_typed_only_grows_at_the_end:
  for evs: +List<M.Ev>
  for e: M.Ev
  {S.is_prefix(S.typed(S.world(evs)), S.typed(M.step(e, S.world(evs)))) == True{} : Bool}

# M5: delivered and dropped Messages are disjoint, and no id is both typed and still waiting.
law m5_terminal_states_are_absorbing:
  for evs: +List<M.Ev>
  {Bool.and(S.disjoint(S.typed(S.world(evs)), S.dropped(S.world(evs))), Bool.and(S.disjoint(S.typed(S.world(evs)), S.queue(S.world(evs))), Bool.and(S.disjoint(S.typed(S.world(evs)), S.held_af(S.world(evs))), S.disjoint(S.typed(S.world(evs)), S.held_tk(S.world(evs)))))) == True{} : Bool}

# M6: every id handed out is in exactly one of pending, held, delivered or dropped.
law m6_no_silent_loss:
  for evs: +List<M.Ev>
  {Nat.add(1n, Nat.add(S.len(S.queue(S.world(evs))), Nat.add(S.len(S.held_af(S.world(evs))), Nat.add(S.len(S.held_tk(S.world(evs))), Nat.add(S.len(S.typed(S.world(evs))), S.len(S.dropped(S.world(evs)))))))) == S.next_id(S.world(evs)) : Nat}

# M7: once the receiver is gone, nothing is pending and nothing waits on a Takeover, and a later send changes nothing (CONFLICT).
law m7_receiver_gone:
  for evs: +List<M.Ev>
  for fu: Bool
  for r: M.Route
  {Bool.and(S.implies(S.gone(S.world(evs)), Bool.and(S.is_nil(S.queue(S.world(evs))), S.is_nil(S.held_tk(S.world(evs))))), S.implies(S.gone(S.world(evs)), Nat.is_eq(S.next_id(M.step(M.Send{fu, r}, S.world(evs))), S.next_id(S.world(evs))))) == True{} : Bool}

# M10: under the fairness assumption that a live receiver turns idle again (cycle), every pending Message is typed.
# Without the assumption nothing is typed from a receiver that stays busy.
law m10_pending_to_idle_is_eventually_typed:
  for evs: +List<M.Ev>
  {S.implies(Bool.not(S.gone(S.world(evs))), S.is_nil(S.queue(M.run(S.cycle(S.len(S.queue(S.world(evs)))), S.world(evs))))) == True{} : Bool}
