import type { Kind } from "@contracts/Kind";

type Tone = "amber" | "red" | "grey" | "light" | "lightest";

type Glyph = { mark: string; tone: Tone };

const GLYPHS: Record<Kind, Glyph> = {
  "needs-you": { mark: "●", tone: "amber" },
  error: { mark: "✕", tone: "red" },
  blocked: { mark: "⏸", tone: "grey" },
  working: { mark: "○", tone: "light" },
  idle: { mark: "·", tone: "light" },
  done: { mark: "✓", tone: "lightest" },
};

/** Most urgent first, as listed under Kind in CONTEXT.md; the type forces every Kind to have a rank. */
const URGENCY_RANK: Record<Kind, number> = {
  error: 0,
  "needs-you": 1,
  blocked: 2,
  working: 3,
  idle: 4,
  done: 5,
};

export const glyphOf = (kind: Kind): Glyph => GLYPHS[kind];

export const hasInk = (glyph: Glyph): boolean => glyph.tone === "amber" || glyph.tone === "red";

export const mostUrgent = (kinds: readonly Kind[]): Kind | null =>
  kinds.reduce<Kind | null>(
    (best, kind) => (best === null || URGENCY_RANK[kind] < URGENCY_RANK[best] ? kind : best),
    null,
  );
