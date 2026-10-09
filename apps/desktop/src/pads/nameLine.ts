import type { JSX } from "solid-js";

/** A flex line, so an owner mark and a long name cannot land on separate lines. */
export const markedLine: JSX.CSSProperties = { display: "flex", gap: "1ch" };

/** The name gives way first: one line, cut with `…`; its full text goes in the `title`. */
export const cutName: JSX.CSSProperties = { "min-width": "0", overflow: "hidden", "text-overflow": "ellipsis", "white-space": "nowrap" };

/** A line's words that must stay whole beside a cut name. */
export const wholeWord: JSX.CSSProperties = { "flex-shrink": "0" };
