Architect. `anchor to state each law so it cannot be read two ways`

Write `proofs/messages/LAWS.bend` for V3 in `scenarios/proofs.md`, over a minimal `model.bend` skeleton named as `docs/messages.md` names its transitions; only `LAWS.bend` and `mutants/`.

- Every law stays open (no proof), and none is true of every state.
- One mutant per law drops exactly the guard that law needs.
- Use `bend base <Name>` for library names; they change between versions.
- The PR quotes each law in English beside its Bend; another Architect approves.
