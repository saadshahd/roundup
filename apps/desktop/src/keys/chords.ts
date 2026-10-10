/** One line of Help (U54): the chord as shown, what it does, and every key press it stands for as `<mods>+<code>`, mods in the order `ctrl+alt+meta+shift`. */
export type Chord = { keys: string; what: string; presses: readonly string[] };

const digits = (from: number, to: number): string[] =>
  Array.from({ length: to - from + 1 }, (_, offset) => `alt+Digit${from + offset}`);

/** Every chord bound in the webview outside a Terminal screen, in Help's order (U54; U106 and U107 follow the scenario's list). `⌘K` joins when U51 binds it. The Terminal's own `⌘C`, `⌘V`, `⌘=`, `⌘-` and `⌘0` act only in a Terminal screen, where `?` never opens Help. */
export const CHORDS: readonly Chord[] = [
  { keys: "⌘J", what: "jump to what needs you", presses: ["meta+KeyJ"] },
  { keys: "⌘N", what: "new agent", presses: ["meta+KeyN"] },
  { keys: "⌘T", what: "new terminal", presses: ["meta+KeyT"] },
  { keys: "⇧⌘N", what: "new agent with a prompt", presses: ["meta+shift+KeyN"] },
  { keys: "⌘1", what: "focus the Rail", presses: ["meta+Digit1"] },
  { keys: "⌘2", what: "focus the pane", presses: ["meta+Digit2"] },
  { keys: "⌃O", what: "expand or fold the thread", presses: ["ctrl+KeyO"] },
  { keys: "⌥1–⌥9", what: "take over the n-th agent", presses: digits(1, 9) },
  { keys: "⌥0", what: "end the takeover", presses: digits(0, 0) },
];

/** The `<mods>+<code>` of a key press, as `Chord.presses` writes it. */
export const pressOf = (press: Pick<KeyboardEvent, "ctrlKey" | "altKey" | "metaKey" | "shiftKey" | "code">): string =>
  [press.ctrlKey && "ctrl", press.altKey && "alt", press.metaKey && "meta", press.shiftKey && "shift", press.code].filter(Boolean).join("+");
