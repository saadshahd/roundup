/** The adjacent id in `ids` from `current` by `direction` (`1` down, `-1` up), clamped at either end (no wrap). `current` is always a member of `ids`: it comes from a rendered row's own `data-id`. */
export const adjacentId = (ids: readonly string[], current: string, direction: 1 | -1): string => {
  const index = ids.indexOf(current);

  return ids[index + direction] ?? current;
};
