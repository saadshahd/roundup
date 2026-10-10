import { cleanup, fireEvent, render, waitFor } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { App } from "../App";
import { rowOf } from "../rail/railFixture";
import { createFakeApp } from "../testing/fakeApp";
import { agent, door } from "../testing/nodes";
import type { EmulatorFactory } from "../terminal/emulator";
import { CHORDS, pressOf } from "./chords";

afterEach(cleanup);

/** A terminal screen with a focusable field, standing in for xterm's helper textarea (as in `u41_app.test.tsx`). */
const focusableEmulator = (): EmulatorFactory => () => {
  const field = document.createElement("textarea");

  return {
    write: () => {},
    setSize: () => {},
    setFontSize: () => {},
    reset: () => {},
    selection: () => "",
    paste: () => {},
    onInput: () => {},
    show: (host) => {
      host.replaceChildren(field);

      return { cols: 80, rows: 24 };
    },
    fit: () => ({ cols: 80, rows: 24 }),
    focus: () => field.focus(),
    isAtBottom: () => true,
    onScroll: () => {},
    scrollToBottom: () => {},
    dispose: () => {},
  };
};

const open = async () => {
  const app = createFakeApp();
  app.opened.project = { name: "p", path: "/p" };
  app.handlers["rail.tree"] = () => [
    door("w", "idle", "idle", { name: "lead" }),
    ...["a", "b", "c", "d", "e", "f", "g", "h", "i"].map((id) => agent(id, "idle", "x", { parent: "w" })),
  ];

  render(() => <App app={app} reducedMotion={() => false} clock={() => 0} createEmulator={focusableEmulator()} />);
  await waitFor(() => rowOf("a"));

  return app;
};

const help = (): HTMLElement | null => document.querySelector<HTMLElement>(".help");

const question = (target: Element): boolean => {
  const press = new KeyboardEvent("keydown", { key: "?", shiftKey: true, bubbles: true, cancelable: true });

  target.dispatchEvent(press);

  return press.defaultPrevented;
};

const MODS = [
  {},
  { metaKey: true },
  { metaKey: true, shiftKey: true },
  { ctrlKey: true },
  { altKey: true },
  { ctrlKey: true, shiftKey: true },
  { altKey: true, shiftKey: true },
];

const CODES = [..."ABCDEFGHIJKLMNOPQRSTUVWXYZ"].map((letter) => `Key${letter}`).concat([..."0123456789"].map((digit) => `Digit${digit}`));

/** Every modified press that something in the webview binds, by `preventDefault`, from a Rail row outside any Terminal. */
const boundPresses = (from: Element): string[] =>
  MODS.flatMap((mods) =>
    CODES.flatMap((code) => {
      if (Object.keys(mods).length === 0) return [];

      const key = code.startsWith("Digit") ? code.slice(5) : code.slice(3).toLowerCase();
      const press = new KeyboardEvent("keydown", { key, code, bubbles: true, cancelable: true, ...mods });

      from.dispatchEvent(press);

      return press.defaultPrevented ? [pressOf(press)] : [];
    }),
  );

describe("u54 keyboard help", () => {
  it("u54_question_mark_in_the_rail_opens_help_listing_each_chord_and_esc_returns_focus", async () => {
    await open();
    rowOf("a").focus();

    expect(question(rowOf("a"))).toBe(true);

    const panel = help();

    expect(panel).not.toBeNull();
    expect(document.activeElement).toBe(panel);

    const lines = [...panel!.querySelectorAll(".help-line")].map((line) => line.textContent?.replace(/\s+/g, " ").trim());

    expect(lines).toEqual(CHORDS.map((chord) => `${chord.keys} ${chord.what}`));
    expect(lines.slice(0, 2)).toEqual(["⌘J jump to what needs you", "⌘N new agent"]);

    fireEvent.keyDown(document.activeElement!, { key: "Escape" });

    expect(help()).toBeNull();
    expect(document.activeElement).toBe(rowOf("a"));
  });

  it("u54_question_mark_again_and_a_click_outside_close_help", async () => {
    await open();
    rowOf("a").focus();
    question(rowOf("a"));
    question(document.activeElement!);

    expect(help()).toBeNull();
    expect(document.activeElement).toBe(rowOf("a"));

    question(rowOf("a"));
    fireEvent.click(rowOf("b"));

    expect(help()).toBeNull();
  });

  it("u54_question_mark_in_a_terminal_a_text_field_or_nowhere_opens_nothing", async () => {
    await open();
    fireEvent.click(rowOf("a"));
    await waitFor(() => expect(document.querySelector(".pane-screen textarea")).not.toBeNull());

    expect(question(document.querySelector(".pane-screen textarea")!)).toBe(false);
    expect(question(document.querySelector("[data-thread-input]")!)).toBe(false);
    expect(question(document.body)).toBe(false);
    expect(help()).toBeNull();
  });

  it("u54_while_open_the_listed_chords_do_nothing_and_the_rail_does_not_reorder", async () => {
    const app = await open();
    const calls = app.calls.length;

    rowOf("a").focus();
    question(rowOf("a"));

    expect(boundPresses(document.activeElement!)).toEqual([]);
    expect(app.calls.length).toBe(calls);
    expect(help()).not.toBeNull();
  });

  it("u54_help_stays_in_step_with_the_chords_bound_in_the_webview", async () => {
    await open();
    fireEvent.click(rowOf("a"));
    await waitFor(() => expect(document.querySelector("[data-thread-input]")).not.toBeNull());

    const listed = CHORDS.flatMap((chord) => chord.presses).sort();
    const bound = [...new Set(boundPresses(document.querySelector("[data-thread-input]")!))].sort();

    expect(bound).toEqual(listed);
  });

  it("u54_every_listed_chord_has_its_effect_with_help_closed", async () => {
    await open();
    rowOf("a").focus();

    for (const chord of CHORDS)
      for (const press of chord.presses) {
        const parts = press.split("+");
        const code = parts.at(-1)!;

        const event = new KeyboardEvent("keydown", {
          code,
          key: code.startsWith("Digit") ? code.slice(5) : code.slice(3).toLowerCase(),
          ctrlKey: parts.includes("ctrl"),
          altKey: parts.includes("alt"),
          metaKey: parts.includes("meta"),
          shiftKey: parts.includes("shift"),
          bubbles: true,
          cancelable: true,
        });

        document.querySelector("[data-thread-input]")!.dispatchEvent(event);

        expect(event.defaultPrevented, `${chord.keys} ${press}`).toBe(true);
      }
  });
});
