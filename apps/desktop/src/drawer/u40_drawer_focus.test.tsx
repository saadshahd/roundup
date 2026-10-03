import { cleanup, fireEvent, render, screen } from "@solidjs/testing-library";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ConnectedProjectContext, connectProject } from "../state/connectedProject";
import { createFakeApp } from "../testing/fakeApp";
import { agent, event, info, node } from "../testing/nodes";
import { Pane } from "../terminal/Pane";
import type { Emulator } from "../terminal/emulator";
import { mountRail, rowOf as railRowOf } from "../rail/railFixture";
import { mountTodos, todo, todoRowOf as shelfRowOf } from "../todos/testHarness";
import { DrawerHost } from "./DrawerHost";

afterEach(cleanup);

/** A Pane whose Terminal is a real focusable field, standing in for xterm's hidden textarea (as `u31_rail_keyboard.test.tsx` does). */
const mountPaneWithDrawer = async () => {
  const app = createFakeApp();
  app.handlers["rail.tree"] = () => [node("a")];
  app.handlers["terminal.list"] = () => [info("t-a")];
  app.handlers["terminal.write"] = () => null;
  app.handlers["terminal.resize"] = () => null;

  const connected = await connectProject(app, { name: "p", path: "/p" }, () => true, () => 0);
  const field = document.createElement("textarea");

  const emulator: Emulator = {
    write: () => {},
    setSize: () => {},
    reset: () => {},
    onInput: (listener) => field.addEventListener("input", () => listener(new TextEncoder().encode(field.value))),
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

  render(() => (
    <ConnectedProjectContext.Provider value={connected}>
      <Pane createEmulator={() => emulator} />
      <DrawerHost drawer={connected.drawer} reducedMotion={connected.reducedMotion} />
    </ConnectedProjectContext.Provider>
  ));

  connected.rail.select("a");

  return { connected, field, app };
};

describe("u40 the Drawer takes and gives back focus", () => {
  it("u40_opening_the_drawer_moves_keyboard_focus_into_it", async () => {
    const { connected } = await mountTodos([todo(1)]);

    connected.drawer.open(() => <input aria-label="field" />);

    await vi.waitFor(() => expect(screen.getByLabelText("drawer").contains(document.activeElement)).toBe(true));
  });

  it("u40_while_open_a_keystroke_never_reaches_the_terminal", async () => {
    const { connected, field, app } = await mountPaneWithDrawer();
    field.focus();
    const before = app.calls.length;

    connected.drawer.open(() => <input aria-label="field" />);
    await vi.waitFor(() => expect(screen.getByLabelText("drawer").contains(document.activeElement)).toBe(true));
    // Fired at whatever holds focus now: if focus had wrongly stayed on `field`, this is the same
    // `input` event that drives `field`'s own listener into a `terminal.write` (screens.ts onInput).
    fireEvent.input(document.activeElement ?? document.body, { target: { value: "x" } });

    expect(app.calls.slice(before).some((call) => call.method === "terminal.write")).toBe(false);
  });

  it("u40_a_keystroke_typed_into_the_pane_before_the_drawer_opened_is_not_lost", async () => {
    const { connected, field, app } = await mountPaneWithDrawer();
    field.focus();
    fireEvent.input(field, { target: { value: "a" } });
    await vi.waitFor(() => expect(app.calls.some((call) => call.method === "terminal.write")).toBe(true));

    connected.drawer.open(() => <input aria-label="field" />);
    await vi.waitFor(() => expect(screen.getByLabelText("drawer").contains(document.activeElement)).toBe(true));
    connected.drawer.close();
    await vi.waitFor(() => expect(document.activeElement).toBe(field));

    expect(field.value).toBe("a");
  });

  it("u40_closing_gives_focus_back_to_the_terminal_that_held_it", async () => {
    const { connected, field } = await mountPaneWithDrawer();
    field.focus();

    connected.drawer.open(() => <input aria-label="field" />);
    await vi.waitFor(() => expect(screen.getByLabelText("drawer").contains(document.activeElement)).toBe(true));
    fireEvent.click(screen.getByText("close"));

    await vi.waitFor(() => expect(document.activeElement).toBe(field));
  });

  it("u40_closing_gives_focus_back_to_the_rail_row_that_held_it", async () => {
    const { connected } = await mountRail([agent("a", "idle", "x")], [], undefined, undefined, { drawer: true });
    railRowOf("a").focus();

    connected.drawer.open(() => <input aria-label="field" />);
    await vi.waitFor(() => expect(screen.getByLabelText("drawer").contains(document.activeElement)).toBe(true));
    fireEvent.click(screen.getByText("close"));

    await vi.waitFor(() => expect(document.activeElement).toBe(railRowOf("a")));
  });

  it("u40_closing_gives_focus_back_to_the_shelf_row_that_held_it", async () => {
    const { connected } = await mountTodos([todo(1)]);
    await screen.findByText(/todo 1/);
    const button = shelfRowOf(1).querySelector("button");

    if (!button) throw new Error("no row button");

    button.focus();

    connected.drawer.open(() => <input aria-label="field" />);
    await vi.waitFor(() => expect(screen.getByLabelText("drawer").contains(document.activeElement)).toBe(true));
    fireEvent.click(screen.getByText("close"));

    await vi.waitFor(() => expect(document.activeElement).toBe(button));
  });

  it("u40_focus_moved_elsewhere_while_open_is_left_alone", async () => {
    const { connected } = await mountRail([agent("a", "idle", "x")], [], undefined, undefined, { drawer: true });
    railRowOf("a").focus();
    connected.drawer.open(() => <input aria-label="field" />);
    await vi.waitFor(() => expect(screen.getByLabelText("drawer").contains(document.activeElement)).toBe(true));
    const other = document.body.appendChild(document.createElement("input"));
    other.focus();

    connected.drawer.close();

    await Promise.resolve();
    expect(document.activeElement).toBe(other);
    other.remove();
  });

  it("u40_replacing_the_open_drawers_content_keeps_the_original_held_focus", async () => {
    const { connected } = await mountRail([agent("a", "idle", "x")], [], undefined, undefined, { drawer: true });
    railRowOf("a").focus();
    connected.drawer.open(() => <input aria-label="first" />);
    await vi.waitFor(() => expect(screen.getByLabelText("drawer").contains(document.activeElement)).toBe(true));

    connected.drawer.open(() => <input aria-label="second" />);
    await Promise.resolve();
    expect(screen.getByLabelText("drawer").contains(document.activeElement)).toBe(true);
    fireEvent.click(screen.getByText("close"));

    await vi.waitFor(() => expect(document.activeElement).toBe(railRowOf("a")));
  });

  it("u40_a_held_row_removed_from_the_rail_while_open_is_not_refocused", async () => {
    const { connected, app } = await mountRail([agent("a", "idle", "x")], [], undefined, undefined, { drawer: true });
    railRowOf("a").focus();
    connected.drawer.open(() => <input aria-label="field" />);
    await vi.waitFor(() => expect(screen.getByLabelText("drawer").contains(document.activeElement)).toBe(true));
    const drawer = screen.getByLabelText("drawer");

    app.handlers["rail.tree"] = () => [];
    app.emit(event({ name: "rail.changed" }));
    await connected.rail.settled();

    fireEvent.click(screen.getByText("close"));

    await Promise.resolve();
    expect(drawer.contains(document.activeElement)).toBe(true);
  });
});
