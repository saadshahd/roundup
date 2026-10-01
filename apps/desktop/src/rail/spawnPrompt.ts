/** `agent.spawn`'s `prompt`: blank or whitespace-only text means none. */
export const promptOf = (text: string): string | null => {
  const trimmed = text.trim();

  return trimmed === "" ? null : trimmed;
};
