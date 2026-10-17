import { fireEvent } from "@solidjs/testing-library";
import { createRoot, onCleanup } from "solid-js";
import { expect, it } from "vitest";
import { createShelfFrame } from "./frame";
import { shelfKeys } from "./keys";

const toggleChord = () => fireEvent.keyDown(document.activeElement ?? document.body, { key: "Escape", code: "Escape" });

it("u491a_shortcut_scope", () => {
  let frame!: ReturnType<typeof createShelfFrame>;

  const dispose = createRoot((stop) => {
    frame = createShelfFrame(() => "/u491a", () => true, () => {});
    const shelf = document.createElement("div");
    shelf.append(document.createElement("button"));
    document.body.append(shelf);
    const keys = shelfKeys({ frame, shelf: () => shelf });
    shelf.querySelector("button")!.focus();
    onCleanup(() => { keys.stop(); shelf.remove(); });

    return stop;
  });

  expect(frame.open()).toBe(true);
  toggleChord();
  expect(frame.open()).toBe(false);
  frame.toggle();
  expect(frame.open()).toBe(true);

  dispose();
  toggleChord();
  expect(frame.open()).toBe(true);
});
