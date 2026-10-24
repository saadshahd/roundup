import { questionDialog, answerQuestion } from "../shelf/bulkQuestionHarness";
import { cleanup, fireEvent, waitFor, within } from "@solidjs/testing-library";
import { afterEach, expect, it } from "vitest";
import { createSignal } from "solid-js";
import { CHORDS, LEADER } from "../keys/chords";
import { leaderKey } from "../keys/leaderFixture";
import { openTabs, tab } from "../header/stripFixture";
import { agent, info, workstream } from "../testing/nodes";
import { field, mountSwitcher, openSwitcher, switcherKey as press } from "./switcherFixture";

afterEach(() => { cleanup(); localStorage.clear(); sessionStorage.clear(); });

it("u2572a_searches_destinations_then_grouped_actions", async () => {
  await mountSwitcher("ws", [
    workstream("ws", { name: "new-onboarding" }),
    agent("a", "idle", "idle", { parent: "ws", name: "renew-token" }),
    agent("b", "idle", "idle", { parent: "ws", name: "New Agent" }),
  ], [info("7", { owner: "ws", title: "new-shell" })]);
  await show();
  fireEvent.input(field()!, { target: { value: " NeW " } });
  await waitFor(() => expect([...document.querySelectorAll('.switcher [role="option"]')].map((row) => row.querySelector('.switcher-name')?.textContent)).toEqual([
    "new-onboarding", "renew-token", "New Agent", "new-shell", "New Agent", "New Agent with a prompt", "New Terminal",
  ]));
  const groups = within(document.querySelector(".switcher")!).getAllByRole("group", { name: "Actions" });

  expect(groups).toHaveLength(1);
  expect(within(groups[0]!).getAllByRole("option").map((row) => row.getAttribute("data-value"))).toEqual([
    "action:chord:⌘N", "action:chord:⇧⌘N", "action:chord:⌘T",
  ]);
  expect([...document.querySelectorAll<HTMLElement>('.switcher [role="option"]')].map((row) => row.dataset.value)).toEqual([
    "ws", "a", "b", "terminal:7", "action:chord:⌘N", "action:chord:⇧⌘N", "action:chord:⌘T",
  ]);
});

const options = (): HTMLElement[] => [...document.querySelectorAll<HTMLElement>('.switcher [role="option"]')];

const actionRows = (): HTMLElement[] => options().filter((row) => row.dataset.value?.startsWith("action:"));

const actionNames = (): (string | null | undefined)[] => actionRows().map((row) => row.querySelector(".switcher-name")?.textContent);

const search = async (text: string) => {
  fireEvent.input(field()!, { target: { value: text } });
  await waitFor(() => expect(field()!.value).toBe(text));
};

const show = async () => {
  openSwitcher();
  await waitFor(() => expect(field()).not.toBeNull());
  await waitFor(() => expect(document.activeElement).toBe(field()));
};

const chooseAction = async (label: string, how: "enter" | "pointer") => {
  await search(label);
  const option = actionRows().find((row) => row.querySelector(".switcher-name")?.textContent === label)!;
  expect(option).toBeDefined();

  if (how === "pointer") {
    fireEvent.pointerDown(option, { button: 0, pointerType: "mouse" });
    fireEvent.pointerUp(option, { button: 0, pointerType: "mouse" });
    fireEvent.click(option);
  } else press("Enter", {}, field()!);
  await waitFor(() => expect(field()).toBeNull());
};

it.each(["New Agent with a prompt", "zzzz-no-match", ""])("u2572a_action_only_empty_and_unfiltered_results: %s", async (query) => {
  await mountSwitcher("ops");
  await show();
  await search(query);

  if (query === "zzzz-no-match") {
    expect(options()).toHaveLength(0);
    expect(document.querySelector('.switcher [role="group"]')).toBeNull();
    expect(document.querySelector(".switcher")?.textContent).toContain("No match");
  } else {
    const names = actionNames();
    expect(names).toEqual(query ? ["New Agent with a prompt"] : ["New Agent", "New Agent with a prompt", "New Terminal", "Ask bar", "Jump to what needs you"]);
  }
});

it.each(["enter", "pointer"] as const)("u2572b_actions_share_key_behavior_in_the_opening_view: creation by %s", async (how) => {
  const { app, connected } = await mountSwitcher("ops");
  const outcomes: unknown[] = [];
  app.handlers["agent.spawn"] = (params) => {
    outcomes.push({ params, closed: field() === null, selected: connected.rail.selected() });

    return agent("created", "idle", "idle", { terminal_id: "created-terminal" });
  };

  press("n", { metaKey: true, code: "KeyN" });
  await waitFor(() => expect(outcomes).toHaveLength(1));
  const keyOutcome = outcomes[0];
  await show();
  await chooseAction("New Agent", how);
  await waitFor(() => expect(outcomes).toEqual([keyOutcome, keyOutcome]));
  expect(app.calls.filter((call) => call.method === "terminal.write")).toEqual([]);
});

it("u2572b_action_choice_does_not_select_a_highlighted_destination", async () => {
  const { app, connected } = await mountSwitcher("ops");
  app.handlers["agent.spawn"] = () => (agent("created", "idle", "idle", { terminal_id: "created-terminal" }));
  await show();
  await search("Ann");
  press("ArrowDown", {}, field()!);
  expect(connected.rail.selected()).toBe("ops");
  await chooseAction("New Agent", "enter");
  expect(app.calls.filter((call) => call.method === "agent.spawn").map((call) => call.params)).toEqual([
    { cwd: "/p", prompt: null, parent: "ops", model: null, effort: null },
  ]);
});

it.each([false, true])("u2572b_actions_share_key_behavior_in_the_opening_view: prompted creation %s", async (byAction) => {
  const { app } = await mountSwitcher("ops");
  app.handlers["agent.spawn"] = () => (agent("created", "idle", "idle", { terminal_id: "created-terminal" }));

  if (byAction) { await show(); await chooseAction("New Agent with a prompt", "enter"); }
  else press("N", { metaKey: true, shiftKey: true, code: "KeyN" });
  await waitFor(() => expect(document.activeElement?.matches(".spawn-prompt-field")).toBe(true));
  const prompt = document.querySelector<HTMLTextAreaElement>(".spawn-prompt-field")!;
  fireEvent.input(prompt, { target: { value: "make this work" } });
  press("Enter", {}, prompt);
  await waitFor(() => expect(app.calls.filter((call) => call.method === "agent.spawn").map((call) => call.params)).toEqual([
    { cwd: "/p", prompt: "make this work", parent: "ops", model: null, effort: null },
  ]));
});

it.each([false, true])("u2572b_actions_share_key_behavior_in_the_opening_view: rejected Terminal %s", async (byAction) => {
  const { app, connected } = await mountSwitcher("ops");
  app.handlers["terminal.open"] = () => { throw new Error("Terminal creation rejected"); };

  if (byAction) { await show(); await chooseAction("New Terminal", "enter"); }
  else press("t", { metaKey: true, code: "KeyT" });
  await waitFor(() => expect(document.querySelector('[role="alert"]')?.textContent).toContain("Terminal creation rejected"));
  expect(app.calls.filter((call) => call.method === "terminal.open").map((call) => call.params)).toEqual([{ workstream: "ops" }]);
  expect(connected.rail.selected()).toBe("ops");
});

it.each([false, true])("u2572b_actions_share_key_behavior_in_the_opening_view: Ask %s", async (byAction) => {
  await mountSwitcher("ops");

  if (byAction) { await show(); await chooseAction("Ask bar", "pointer"); }
  else press("/", { metaKey: true, code: "Slash" });
  await waitFor(() => expect(document.activeElement?.closest(".ask-bar")).not.toBeNull());
});

it.each([false, true])("u2572b_actions_share_key_behavior_in_the_opening_view: Remove cancellation %s", async (byAction) => {
  const { app, connected } = await mountSwitcher("ann");
  const row = document.querySelector<HTMLElement>('[data-id="ann"][role="treeitem"]')!;
  row.focus();

  if (byAction) { await show(); await chooseAction("Remove", "enter"); }
  else press("Backspace", { metaKey: true }, row);
  await questionDialog();
  await answerQuestion(false);
  expect(app.calls.filter((call) => call.method === "rail.remove")).toEqual([]);
  expect(connected.rail.selected()).toBe("ann");
});

it.each([false, true])("u2572b_actions_share_key_behavior_in_the_opening_view: attention %s", async (byAction) => {
  const { connected } = await mountSwitcher("ops");

  if (byAction) { await show(); await chooseAction("Jump to what needs you", "enter"); }
  else leaderKey("KeyJ");
  expect(connected.rail.selected()).toBe("cy");
});

it.each(["M", "W"])("u2572b_actions_share_key_behavior_in_the_opening_view: Bubble %s", async (key) => {
  const results: unknown[] = [];

  for (const byAction of [false, true]) {
    const { app, emulators } = await mountSwitcher("ops");
    await waitFor(() => expect(document.querySelector<HTMLElement>('.bubble[data-bubble="7"]')?.hidden).toBe(false));
    emulators.made.get("7")!.focus();
    await waitFor(() => expect(document.activeElement?.closest("[data-bubble]")?.getAttribute("data-bubble")).toBe("7"));

    if (byAction) { await show(); await chooseAction(key === "M" ? "Minimise or restore the Bubble" : "Close the Bubble", "enter"); }
    else leaderKey(`Key${key}`, {}, document.activeElement!);

    if (key === "W") {
      await questionDialog();
      await answerQuestion(true);
      await waitFor(() => expect(document.querySelector('.bubble[data-bubble="7"]')).toBeNull());
    } else await waitFor(() => expect(document.querySelector<HTMLElement>('.bubble[data-bubble="7"]')?.hidden).toBe(true));
    results.push(app.calls.filter((call) => call.method === "terminal.close" || call.method === "terminal.write"));
    cleanup();
    sessionStorage.clear();
  }

  expect(results).toEqual(key === "W" ? [[{ method: "terminal.close", params: { id: "7" } }], [{ method: "terminal.close", params: { id: "7" } }]] : [[], []]);
});

it.each(["S", "⇧J"])("u2572b_actions_share_key_behavior_in_the_opening_view: App owner %s", async (key) => {
  for (const byAction of [false, true]) {
    await openTabs({ api: [agent("a", "needs-you", "asks")], web: [agent("w", "error", "failure")] });

    const beforeShelf = document.querySelector<HTMLElement>(".window")?.dataset.shelf;

    if (byAction) { await show(); await chooseAction(key === "S" ? "Show or hide the work items" : "Jump to what needs you in another Project", "enter"); }
    else leaderKey("Key" + (key === "S" ? "S" : "J"), { shiftKey: key === "⇧J" });

    if (key === "S") await waitFor(() => expect(document.querySelector<HTMLElement>(".window")?.dataset.shelf).toBe(beforeShelf === "open" ? "closed" : "open"));
    else await waitFor(() => expect(tab("api").getAttribute("aria-selected")).toBe("true"));
    cleanup();
  }
});

it.each(["rail", "pane", "bubble", "editor", "text"])("u2572c_actions_follow_opening_context_and_revalidate: %s", async (context) => {
  await mountSwitcher(context === "bubble" || context === "editor" ? "ops" : "ann");
  let focus: HTMLElement;

  if (context === "rail") focus = document.querySelector<HTMLElement>('[data-id="ann"][role="treeitem"]')!;
  else if (context === "pane") focus = document.querySelector<HTMLElement>(".pane-screen [tabindex]")!;
  else if (context === "bubble") focus = document.querySelector<HTMLElement>('.terminal-tab[data-bubble="7"]')!;
  else if (context === "editor") {
    fireEvent.dblClick(document.querySelector('.terminal-tab[data-bubble="7"] .terminal-tab-name')!);
    await waitFor(() => expect(document.querySelector(".terminal-tab-editor input")).not.toBeNull());
    focus = document.querySelector<HTMLInputElement>(".terminal-tab-editor input")!;
  } else {
    focus = document.createElement("input");
    document.body.append(focus);
  }

  focus.focus();
  await show();

  const expected = ["New Agent", "New Agent with a prompt", ...(context === "editor" ? [] : ["New Terminal"]),
    ...(context === "rail" ? ["Remove"] : []),
    "Ask bar", "Jump to what needs you", ...(["bubble", "editor"].includes(context) ? ["Minimise or restore the Bubble", "Close the Bubble"] : [])];

  expect(actionNames()).toEqual(expected);
  press("Escape", {}, field()!);

  if (focus.isConnected) await waitFor(() => expect(document.activeElement).toBe(focus));

  if (context === "text") focus.remove();
});

it.each(["daemon", "removed", "selection", "pending"])("u2572c_actions_follow_opening_context_and_revalidate: stale %s", async (change) => {
  const [daemonExit, setDaemonExit] = createSignal<{ code: number | null } | null>(null);
  const { app, connected } = await mountSwitcher("ann", undefined, undefined, daemonExit);
  const origin = document.querySelector<HTMLElement>('[data-id="ann"][role="treeitem"]')!;
  origin.focus();
  await show();
  const label = "New Agent";
  await search(label);
  const stale = actionRows().find((row) => row.querySelector(".switcher-name")?.textContent === label)!;
  const before = app.calls.length;

  if (change === "daemon") setDaemonExit({ code: 1 });
  else if (change === "removed") { app.handlers["rail.tree"] = () => []; await connected.rail.refresh(); }
  else if (change === "selection") connected.rail.select("ops");
  else {
    app.handlers["agent.spawn"] = () => new Promise(() => {});
    fireEvent.click(document.querySelector<HTMLButtonElement>('[data-id="lead"] button[aria-label="Add Agent"]')!);
  }

  await waitFor(() => expect(actionNames()).not.toContain(label));
  const afterChange = app.calls.length;
  fireEvent.pointerDown(stale, { button: 0 });
  fireEvent.click(stale);
  expect(field()).not.toBeNull();
  expect(app.calls.length).toBe(afterChange);

  if (change !== "selection") expect(connected.rail.selected()).not.toBe("ops");
  expect(app.calls.slice(before).filter((call) => call.method === "terminal.write")).toEqual([]);
});

it("u2572c_no_selection_no_attention_and_a_removed_Bubble_never_fall_back", async () => {
  const { app, connected } = await mountSwitcher(null, [workstream("ws", { name: "only" })], [info("7", { owner: "ws" })]);
  await show();
  expect(actionNames()).toEqual(["New Agent", "New Agent with a prompt", "New Terminal", "Ask bar"]);
  press("Escape", {}, field()!);
  connected.rail.select("ws");
  document.querySelector<HTMLElement>('.terminal-tab[data-bubble="7"]')!.focus();
  await show();
  expect(actionNames()).toContain("Close the Bubble");
  app.handlers["terminal.list"] = () => [];
  await connected.rail.refresh({ terminals: true });
  await waitFor(() => expect(actionNames()).not.toContain("Close the Bubble"));
});

it.each(["rail", "bubble", "App"])("u2572d_action_keys_come_from_the_tables: %s", async (context) => {
  if (context === "App") await openTabs({ api: [agent("a", "needs-you", "asks")], web: [agent("w", "error", "failure")] });
  else {
    await mountSwitcher(context === "rail" ? "ann" : "ops");
    document.querySelector<HTMLElement>(context === "rail" ? '[data-id="ann"][role="treeitem"]' : '.terminal-tab[data-bubble="7"]')!.focus();
  }

  await show();

  for (const option of actionRows()) {
    const row = [...CHORDS, ...LEADER].find((row) => row.what === option.querySelector(".switcher-name")?.textContent)!;
    const keys = "press" in row ? `${CHORDS[3]!.keys} ${row.keys}` : row.keys;
    expect(option.querySelector(".key-chips")?.textContent).toBe(keys);
    expect([...option.querySelectorAll("kbd.key-chip")].map((chip) => chip.textContent).join("")).toBe(keys.replaceAll(" ", ""));
  }
});

it("u2572c_Project_changes_dispose_the_opening_actions", async () => {
  const { app } = await openTabs({ api: [agent("a", "idle", "idle")], web: [agent("w", "idle", "idle")] });
  await show();
  const stale = actionRows()[0]!;
  fireEvent.click(tab("api"));
  await waitFor(() => expect(field()).toBeNull());
  fireEvent.click(stale);
  expect(document.querySelector('[data-id="a"][aria-selected="true"]')).toBeNull();
  expect(app.of("/Users/me/web").calls.filter((call) => call.method === "agent.spawn")).toEqual([]);
  expect(app.of("/Users/me/api").calls.filter((call) => call.method === "agent.spawn")).toEqual([]);
});

it("u2572c_an_existing_modal_owns_keys_and_cannot_open_the_Switcher", async () => {
  const { app } = await openTabs({ web: [agent("a", "working", "busy")] });
  press("/", { metaKey: true, code: "Slash" });
  await waitFor(() => expect(document.activeElement?.closest(".ask-bar")).not.toBeNull());
  const menu = document.activeElement!;
  press("k", { metaKey: true, code: "KeyK" }, menu);
  press("k", { code: "KeyK" }, menu);
  expect(field()).toBeNull();
  expect(app.calls.filter((call) => call.method === "agent.spawn" || call.method === "terminal.write")).toEqual([]);
});

it("u2572c_New_Agent_can_return_to_the_existing_prompt_while_creation_is_pending", async () => {
  const { app } = await mountSwitcher("ops");
  press("N", { metaKey: true, shiftKey: true, code: "KeyN" });
  const prompt = document.querySelector<HTMLTextAreaElement>(".spawn-prompt-field")!;
  fireEvent.input(prompt, { target: { value: "keep my draft" } });
  app.handlers["rail.createWorkstream"] = () => new Promise(() => {});
  fireEvent.click(document.querySelector<HTMLButtonElement>('.rail-actions [data-button="add"]')!);
  await show();
  expect(actionNames()).toContain("New Agent");
  expect(actionNames()).not.toContain("New Agent with a prompt");
  await chooseAction("New Agent", "enter");
  expect(document.activeElement).toBe(prompt);
  expect(prompt.value).toBe("keep my draft");
  expect(app.calls.filter((call) => call.method === "agent.spawn")).toEqual([]);
});

it("u2572b_the_Shelf_action_keeps_its_opening_toggle_after_the_overlay_closes", async () => {
  await openTabs({ web: [agent("w", "idle", "idle")] });
  const window = document.querySelector<HTMLElement>(".window")!;
  window.dataset.shelfMode = "overlay";
  await show();
  expect(window.dataset.shelf).toBe("closed");
  await chooseAction("Show or hide the work items", "enter");
  expect(window.dataset.shelf).toBe("closed");
});

it("u2572c_a_root_Terminal_action_does_not_fall_back_to_another_Workstream", async () => {
  const first = workstream("first", { order: 0 });
  const second = workstream("second", { order: 1 });
  const { app, connected } = await mountSwitcher(null, [first, second], []);
  await show();
  expect(actionNames()).toContain("New Terminal");
  await search("New Terminal");
  const stale = actionRows()[0]!;
  app.handlers["rail.tree"] = () => [second];
  await connected.rail.refresh();
  await waitFor(() => expect(actionNames()).not.toContain("New Terminal"));
  fireEvent.click(stale);
  expect(field()).not.toBeNull();
  expect(app.calls.filter((call) => call.method === "terminal.open")).toEqual([]);
  expect(connected.rail.selected()).toBeNull();
});

it("u2572c_a_pending_Bubble_owner_removes_its_actions_while_the_Switcher_is_open", async () => {
  const { app } = await mountSwitcher("ops");
  document.querySelector<HTMLElement>('.terminal-tab[data-bubble="7"]')!.focus();
  await show();
  expect(actionNames()).toContain("Close the Bubble");
  app.handlers["terminal.close"] = () => new Promise(() => {});
  fireEvent.click(document.querySelector<HTMLButtonElement>('.terminal-tab-pair[data-bubble="7"] button[aria-label^="Close "]')!);
  await answerQuestion(true);
  await waitFor(() => expect(actionNames()).not.toContain("Close the Bubble"));
  expect(app.calls.filter((call) => call.method === "terminal.close").map((call) => call.params)).toEqual([{ id: "7" }]);
  expect(field()).not.toBeNull();
});
