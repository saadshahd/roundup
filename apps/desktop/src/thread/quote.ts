const QUOTE_CHARS = 2000;

export type Quote = { text: string; source: string };

/** U110: the selected text up to 2000 characters; a longer one is cut and marked `[cut]`. */
export const quoteOf = (selected: string, source: string): Quote => ({
  text: selected.length > QUOTE_CHARS ? `${selected.slice(0, QUOTE_CHARS)} [cut]` : selected,
  source,
});

/** What a Quote adds to the body of the Message the Thread's input sends. */
export const quotedLines = (quote: Quote): string => `> ${quote.source}\n${quote.text.split("\n").map((line) => `> ${line}`).join("\n")}`;
