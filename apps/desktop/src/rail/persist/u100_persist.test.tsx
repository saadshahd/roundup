import { cleanup, fireEvent, render, screen, waitFor, within } from "@solidjs/testing-library";
import { afterEach, expect, it, vi } from "vitest";
import { App } from "../../App";
import type { RailNode } from "@contracts/agent/RailNode";
import { seedApp } from "../../testing/seeds";
import type { SeedName } from "../../testing/seeds";
import { event, NOW } from "../../testing/nodes";
import { fakeEmulators } from "../../terminal/paneHarness";
import type { EmulatorFactory } from "../../terminal/emulator";

const key = (path = "/u100") => `roundup:rail:${path}`;

const saved = (selected: string | null, collapsed: string[] = [], path = "/u100") =>
  localStorage.setItem(key(path), JSON.stringify({ selected, collapsed }));

const read = (path = "/u100") => JSON.parse(localStorage.getItem(key(path)) ?? "null");

const row = (id: string) => document.querySelector<HTMLElement>(`[role=treeitem][data-id="${id}"]`);

const click = (id: string) => fireEvent.click(row(id)!);

const collapse = (id: string) => fireEvent.click(within(row(id)!).getByRole("button", { name: "collapse" }));

const mount = (seed: SeedName = "tree-40", path = "/u100", tree?: Promise<RailNode[]>) => {
  const controls = seedApp(seed, NOW);
  controls.app.opened.project = { name: "u100", path };

  if (tree) controls.app.handlers["rail.tree"] = () => tree;
  const emulators = fakeEmulators();

  const factory: EmulatorFactory = (id) => {
    const emulator = emulators.factory(id);

    return {
      ...emulator,
      show: (host, focus = true) => {
        const field = document.createElement("input");
        field.dataset.terminal = id;
        host.replaceChildren(field);

        if (focus) field.focus();

        return emulator.fit();
      },
    };
  };

  const view = render(() => <App app={controls.app} reducedMotion={() => false} clock={() => NOW} createEmulator={factory} />);

  return { ...controls, ...view };
};

const loaded = () => waitFor(() => expect(row("agent-1")).not.toBeNull());

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  localStorage.clear();
});

it("u100_same_project_restores_selection_and_collapsed_groups", async () => {
  const first = mount();
  await loaded();
  collapse("auth");
  click("loose-shell");
  first.unmount();
  mount();
  await loaded();
  expect(row("loose-shell")?.getAttribute("aria-selected")).toBe("true");
  expect(row("auth")?.getAttribute("aria-expanded")).toBe("false");
  expect(row("tokens")).toBeNull();
  expect(document.querySelector('[data-terminal="t-loose-shell"]')).not.toBeNull();
});

it("u100_different_project_never_reads_or_changes_the_saved_project", async () => {
  saved("agent-1", ["backend"]);
  const get = vi.spyOn(Storage.prototype, "getItem");
  mount("tree-40", "/other");
  await loaded();
  expect(get.mock.calls).toEqual([[key("/other")]]);
  expect(row("agent-1")?.getAttribute("aria-selected")).toBe("false");
  expect(row("backend")?.getAttribute("aria-expanded")).toBe("true");
  click("agent-2");
  expect(read()).toEqual({ selected: "agent-1", collapsed: ["backend"] });
  expect(read("/other")).toEqual({ selected: "agent-2", collapsed: [] });
});

it("u100_removed_ids_are_ignored_and_dropped_on_the_next_write", async () => {
  saved("removed", ["gone", "backend", "agent-1"]);
  mount();
  await loaded();
  expect(document.querySelector('[aria-selected="true"]')).toBeNull();
  expect(row("backend")?.getAttribute("aria-expanded")).toBe("false");
  click("agent-1");
  expect(read()).toEqual({ selected: "agent-1", collapsed: ["backend"] });
});

it("u100_selection_under_collapsed_groups_is_revealed_and_saved", async () => {
  saved("tokens", ["backend", "auth"]);
  mount();
  await loaded();
  expect(row("tokens")?.getAttribute("aria-selected")).toBe("true");
  expect(row("backend")?.getAttribute("aria-expanded")).toBe("true");
  expect(row("auth")?.getAttribute("aria-expanded")).toBe("true");
  expect(read()).toEqual({ selected: "tokens", collapsed: [] });
});

it.each(["{", "null", '{"selected":4,"collapsed":[]}', '{"selected":"agent-1","collapsed":[2]}'])(
  "u100_unreadable_value_is_absent_and_replaced_on_change_%s", async (value) => {
    localStorage.setItem(key(), value);
    mount("agents-10");
    await loaded();
    expect(document.querySelector('[aria-selected="true"]')).toBeNull();
    click("agent-2");
    expect(read()).toEqual({ selected: "agent-2", collapsed: [] });
  },
);

it("u100_storage_read_failure_is_absent", async () => {
  const get = vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => { throw new Error("unreadable"); });
  mount("agents-10");
  await loaded();
  expect(document.querySelector('[aria-selected="true"]')).toBeNull();
  get.mockRestore();
  click("agent-2");
  expect(read()).toEqual({ selected: "agent-2", collapsed: [] });
});

it("u100_write_failure_shows_one_ink_line_and_keeps_working", async () => {
  vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => { throw new Error("storage full"); });
  mount();
  await loaded();
  click("agent-1");
  collapse("backend");
  expect(screen.getAllByText("✕ storage full")).toHaveLength(1);
  const failure = screen.getByText("✕ storage full");
  expect(failure.classList.contains("ink")).toBe(true);
  expect(failure.compareDocumentPosition(screen.getByText("+ agent")) & Node.DOCUMENT_POSITION_FOLLOWING).not.toBe(0);
  expect(row("backend")?.getAttribute("aria-expanded")).toBe("false");
  click("agent-2");
  expect(row("agent-2")?.getAttribute("aria-selected")).toBe("true");
});

it("u100_first_tree_is_restored_without_flash_scroll_focus_or_extra_rpc", async () => {
  saved("loose-shell", ["backend"]);
  const pending = Promise.withResolvers<RailNode[]>();
  const source = seedApp("tree-40", NOW);
  const tree = await source.app.rpc("rail.tree", null);
  const field = document.createElement("input");
  document.body.append(field);
  field.focus();
  const focus = vi.spyOn(HTMLElement.prototype, "focus");
  const scroll = vi.spyOn(Element.prototype, "scrollIntoView");
  const frames: string[] = [];

  const observer = new MutationObserver(() => {
    if (row("backend")) frames.push(`${row("backend")?.getAttribute("aria-expanded")}/${row("loose-shell")?.getAttribute("aria-selected")}`);
  });

  observer.observe(document.body, { childList: true, subtree: true, attributes: true });
  const { app } = mount("tree-40", "/u100", pending.promise);
  await waitFor(() => expect(app.calls.some((call) => call.method === "rail.tree")).toBe(true));
  expect(screen.queryByRole("tree")).toBeNull();
  expect(document.querySelector('[data-terminal]')).toBeNull();
  pending.resolve(tree);
  await loaded();
  observer.disconnect();
  expect(frames.length).toBeGreaterThan(0);
  expect(new Set(frames)).toEqual(new Set(["false/true"]));
  expect(focus).not.toHaveBeenCalled();
  expect(scroll).not.toHaveBeenCalled();
  expect(document.activeElement).toBe(field);
  expect(app.calls.map((call) => call.method).sort()).toEqual([
    "pad.list", "rail.tree", "terminal.list", "terminal.resize", "terminal.snapshot", "todo.list",
  ]);
  field.remove();
});

it("u100_reopening_after_daemon_exit_restores_the_last_layout", async () => {
  const first = mount();
  await loaded();
  collapse("backend");
  click("agent-2");
  first.app.exitDaemon({ code: 1 });
  await screen.findByText("✕ daemon exited 1");
  first.unmount();
  mount();
  await loaded();
  expect(row("agent-2")?.getAttribute("aria-selected")).toBe("true");
  expect(row("backend")?.getAttribute("aria-expanded")).toBe("false");
});

it("u100_later_selection_reveals_ancestors_and_saves_each_change", async () => {
  saved("agent-2", ["backend"]);
  const { app } = mount();
  await loaded();
  collapse("backend");
  click("tokens");
  expect(read()).toEqual({ selected: "tokens", collapsed: [] });
  collapse("auth");
  expect(read()).toEqual({ selected: "tokens", collapsed: ["auth"] });
  app.emit(event({ name: "agent.status", data: { id: "logins", status: { kind: "error", label: "failed", since: 0 } } }));
  fireEvent.keyDown(document, { key: "j", metaKey: true });
  expect(row("logins")?.getAttribute("aria-selected")).toBe("true");
  expect(read()).toEqual({ selected: "logins", collapsed: [] });
});

it("u100_done_lines_are_not_restored", async () => {
  const first = mount("agents-10");
  await loaded();
  fireEvent.click(screen.getByText("✓ 1 done"));
  expect(row("agent-6")).not.toBeNull();
  click("agent-2");
  expect(read()).toEqual({ selected: "agent-2", collapsed: [] });
  first.unmount();
  mount("agents-10");
  await loaded();
  expect(row("agent-6")).toBeNull();
  expect(screen.getByText("✓ 1 done")).toBeDefined();
});

it("u100_meta_agent_selection_survives_reopening", async () => {
  const first = mount();
  await loaded();
  collapse("backend");
  click("payments");
  first.unmount();
  mount();
  await waitFor(() => expect(row("payments")).not.toBeNull());
  expect(row("backend")?.getAttribute("aria-expanded")).toBe("false");
  expect(row("payments")?.getAttribute("aria-selected")).toBe("true");
});

it("u100_failed_first_tree_keeps_storage_until_a_successful_tree", async () => {
  saved("loose-shell", ["backend"]);
  const pending = Promise.withResolvers<RailNode[]>();
  const source = seedApp("tree-40", NOW);
  const tree = await source.app.rpc("rail.tree", null);
  const get = vi.spyOn(Storage.prototype, "getItem");
  const { app } = mount("tree-40", "/u100", pending.promise);
  await waitFor(() => expect(app.calls.some((call) => call.method === "rail.tree")).toBe(true));
  pending.reject(new Error("tree unavailable"));
  await screen.findByText("✕ tree unavailable");
  expect(get).not.toHaveBeenCalled();
  expect(document.querySelector('[aria-selected="true"]')).toBeNull();
  app.handlers["rail.tree"] = () => tree;
  app.emit(event({ name: "rail.changed" }));
  await loaded();
  expect(row("loose-shell")?.getAttribute("aria-selected")).toBe("true");
  expect(row("backend")?.getAttribute("aria-expanded")).toBe("false");
});

it("u100_storage_failure_clears_on_the_next_click_and_can_be_reported_again", async () => {
  vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => { throw new Error("storage full"); });
  mount();
  await loaded();
  click("agent-2");
  expect(screen.getByText("✕ storage full")).toBeDefined();
  fireEvent.click(screen.getByRole("tree"));
  expect(screen.queryByText("✕ storage full")).toBeNull();
  collapse("backend");
  expect(screen.getAllByText("✕ storage full")).toHaveLength(1);
});
