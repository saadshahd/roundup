/** The adjacent id in `ids` from `current` by `direction` (`1` down, `-1` up), clamped at either end (no wrap); the first id when `current` is no longer in `ids`; `null` when `ids` is empty. */
export const adjacentId = (ids: readonly string[], current: string, direction: 1 | -1): string | null => {
  const first = ids[0];

  if (first === undefined) return null;

  const index = ids.indexOf(current);

  if (index === -1) return first;

  return ids[index + direction] ?? current;
};
