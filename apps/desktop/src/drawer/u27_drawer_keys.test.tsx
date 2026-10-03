import { cleanup, fireEvent, render, screen } from "@solidjs/testing-library";
import { createSignal } from "solid-js";
import { afterEach, describe, expect, it, vi } from "vitest";
import { createFakeApp } from "../testing/fakeApp";
import { connectProject, ConnectedProjectContext } from "../state/connectedProject";
import { Pane } from "../terminal/Pane";
import type { Emulator } from "../terminal/emulator";
import { info, node } from "../testing/nodes";
import styles from "../styles.css?inline";
import { createDrawer } from "./drawer";
import { DrawerHost } from "./DrawerHost";

afterEach(cleanup);

const mountDrawer = () => {
  const drawer = createDrawer();
  const [reduced] = createSignal(true);
  render(() => <DrawerHost drawer={drawer} reducedMotion={reduced} />);

  return drawer;
};

const mountPaneWithDrawer = async (selectAgent: boolean) => {
  const app = createFakeApp();

  app.handlers["rail.tree"] = () => [node("a")];
  app.handlers["terminal.list"] = () => [info("t-a")];
  app.handlers["terminal.write"] = () => null;
  app.handlers["terminal.resize"] = () => null;

  const connected = await connectProject(app, { name: "p", path: "/p" }, () => true, () => 0);
  const focused: string[] = [];

  const emulator: Emulator = {
    write: () => {},
    setSize: () => {},
    reset: () => {},
    onInput: () => {},
    show: () => ({ cols: 80, rows: 24 }),
    fit: () => ({ cols: 80, rows: 24 }),
    focus: () => focused.push("t-a"),
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

  if (selectAgent) connected.rail.select("a");

  connected.drawer.open(() => <input aria-label="field" />);
  focused.length = 0;

  return { connected, focused };
};

const mountPaneWithFocusedTerminal = async () => {
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

  render(() => (
    <ConnectedProjectContext.Provider value={connected}>
      <Pane createEmulator={() => emulator} />
      <DrawerHost drawer={connected.drawer} reducedMotion={connected.reducedMotion} />
    </ConnectedProjectContext.Provider>
  ));

  connected.rail.select("a");

  return { connected, field };
};

describe("u27 focus returns", () => {
  it("u27_closing_the_drawer_gives_the_keyboard_back_to_the_shown_terminal", async () => {
    const { connected, field } = await mountPaneWithFocusedTerminal();
    field.focus();

    connected.drawer.open(() => <input aria-label="field" />);
    await vi.waitFor(() => expect(screen.getByLabelText("drawer").contains(document.activeElement)).toBe(true));
    fireEvent.click(screen.getByText("close"));

    await vi.waitFor(() => expect(document.activeElement).toBe(field));
    expect(connected.drawer.content()).toBeNull();
  });

  it("u27_with_no_terminal_shown_closing_the_drawer_focuses_nothing", async () => {
    const { focused } = await mountPaneWithDrawer(false);

    fireEvent.click(screen.getByText("close"));

    await Promise.resolve();
    expect(focused).toEqual([]);
  });

  it("u27_focus_already_in_another_field_is_left_alone", async () => {
    const { connected, focused } = await mountPaneWithDrawer(true);
    const other = document.body.appendChild(document.createElement("input"));

    other.focus();
    connected.drawer.close();

    await Promise.resolve();
    expect(focused).toEqual([]);
    other.remove();
  });
});

describe("u28 Esc closes", () => {
  it("u28_esc_closes_the_open_drawer", () => {
    const drawer = mountDrawer();
    drawer.open(() => <p>detail</p>);

    fireEvent.keyDown(document.body, { key: "Escape" });

    expect(drawer.content()).toBeNull();
  });

  it("u28_esc_inside_a_field_leaves_the_drawer_open", () => {
    const drawer = mountDrawer();
    drawer.open(() => <input aria-label="field" />);

    fireEvent.keyDown(screen.getByLabelText("field"), { key: "Escape" });

    expect(drawer.content()).not.toBeNull();
  });

  it("u28_esc_inside_a_pad_text_field_leaves_the_drawer_open", () => {
    const drawer = mountDrawer();
    drawer.open(() => <textarea aria-label="text" />);

    fireEvent.keyDown(screen.getByLabelText("text"), { key: "Escape" });

    expect(drawer.content()).not.toBeNull();
  });
});

describe("u27 hairline edge", () => {
  it("u27_the_open_drawer_has_a_one_pixel_lightest_left_edge", () => {
    const sheet = document.head.appendChild(document.createElement("style"));
    sheet.textContent = styles;
    const drawer = mountDrawer();
    drawer.open(() => <p>detail</p>);
    const edge = getComputedStyle(screen.getByLabelText("drawer"));
    const seen = [edge.borderLeftWidth, edge.borderLeftStyle, edge.borderLeftColor];
    sheet.remove();

    expect(seen).toEqual(["1px", "solid", "var(--lightest)"]);
  });
});
