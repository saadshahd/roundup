import { cleanup, fireEvent, render, waitFor } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { App } from "../App";
import { rowOf } from "../rail/railFixture";
import { createFakeApp } from "../testing/fakeApp";
import { agent } from "../testing/nodes";
import type { EmulatorFactory } from "../terminal/emulator";

afterEach(cleanup);

const wait = (ms: number): Promise<void> => new Promise((resolve) => setTimeout(resolve, ms));

/** A terminal screen that is actually focusable, standing in for xterm's real helper textarea (as in `u41_rail_keys.test.tsx`). */
const focusableEmulator = (): EmulatorFactory => () => {
  const field = document.createElement("textarea");

  return {
    write: () => {},
    onInput: () => {},
    show: (host) => {
      host.replaceChildren(field);
      field.focus();

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

describe("u41 the App mounts Keys", () => {
  it("u41_cmd_1_focuses_the_rail_in_the_real_app", async () => {
    const app = createFakeApp();
    app.opened.project = { name: "p", path: "/p" };
    app.handlers["rail.tree"] = () => [agent("a", "idle", "x")];

    render(() => <App app={app} reducedMotion={() => false} clock={() => 0} />);

    await waitFor(() => rowOf("a"));

    fireEvent.keyDown(document, { key: "1", metaKey: true });

    expect(document.activeElement).toBe(rowOf("a"));
  });

  it("u41_cmd_1_on_an_already_selected_row_keeps_focus_in_the_rail", async () => {
    const app = createFakeApp();
    app.opened.project = { name: "p", path: "/p" };
    app.handlers["rail.tree"] = () => [agent("a", "idle", "x")];

    render(() => <App app={app} reducedMotion={() => false} clock={() => 0} createEmulator={focusableEmulator()} />);

    await waitFor(() => rowOf("a"));

    fireEvent.click(rowOf("a"));

    fireEvent.keyDown(document, { key: "1", metaKey: true });

    expect(document.activeElement).toBe(rowOf("a"));

    await wait(300);

    expect(document.activeElement).toBe(rowOf("a"));
  });
});
