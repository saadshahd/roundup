import type { Message } from "@contracts/message/Message";

const FOLDED_CHARS = 60;

/** U106: a Message between two Agents; one to or from the user, or to a node the Rail does not show, is never folded. */
export const isBetweenAgents = (message: Message, nodeIds: ReadonlySet<string>): boolean =>
  message.from.kind === "agent" && nodeIds.has(message.to);

/** `<from> → <to> <kind> <first 60 characters>…`; a line break in the body reads as a space so the line stays one line. */
export const foldedLine = (from: string, to: string, message: Message): string => {
  const head = message.body.replace(/\s/g, " ").slice(0, FOLDED_CHARS);

  return `${from} → ${to} ${message.kind} ${head}…`;
};

export const fullLine = (from: string, to: string, message: Message): string => `${from} → ${to} ${message.kind} ${message.body}`;
