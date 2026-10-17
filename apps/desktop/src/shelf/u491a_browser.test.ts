import { afterAll, beforeAll, beforeEach, describe, expect, it } from "vitest";
import { openLane } from "../todos/laneBrowser";
import type { LaneBrowser } from "../todos/laneBrowser";

const rows = (lane: LaneBrowser) => lane.evaluate<number[]>("[...document.querySelectorAll('.shelf .todo-row[data-id]')].map(row => Number(row.dataset.id))");

const inDialog = (lane: LaneBrowser) => lane.evaluate<boolean>("document.querySelector('[role=\"alertdialog\"]').contains(document.activeElement)");

describe("u491a Shelf questions and navigation", () => {
  let lane: LaneBrowser;

  beforeAll(async () => { lane = await openLane(1512, "light", "u491a"); }, 180_000);

  beforeEach(async () => { await lane.seedTodos(["first", "second", "third"]); }, 120_000);

  afterAll(async () => { await lane?.close(); });

  /** Selects the first two Todos with the Select controls and opens the question with the bar's Delete. */
  const ask = async () => {
    const [one, two] = await rows(lane);

    await lane.browser("click", `.shelf [data-todo="${one}"] [aria-label="Select"]`);
    await lane.browser("click", `.shelf [data-todo="${two}"] [aria-label="Select"]`);
    await lane.until("document.querySelector('.shelf-bulk')?.textContent.includes('2 selected')");
    await lane.browser("click", ".shelf-bulk-line button:nth-of-type(1)");
    await lane.until("!!document.querySelector('[role=\"alertdialog\"]')");

    return [one!, two!] as const;
  };

  it("u490a_pointer_reorders_with_the_title_tooltip_open", async () => {
    const [one, two, three] = await rows(lane);
    await lane.browser("hover", `.shelf [data-id="${three}"] .todo-row-button`);
    await lane.until("[...document.querySelectorAll('[role=tooltip]')].some(tip => tip.textContent === 'third')");

    const points = await lane.evaluate<{ from: { x: number; y: number }; to: { x: number; y: number } }>(`(() => {
      const point = id => { const rect = document.querySelector('.shelf [data-id="' + id + '"] .todo-row-button').getBoundingClientRect(); return { x: Math.round(rect.left + rect.width / 2), y: Math.round(rect.top + rect.height / 2) }; };
      return { from: point(${three}), to: point(${one}) };
    })()`);

    await lane.browser("mouse", "move", String(points.from.x), String(points.from.y));
    await lane.browser("mouse", "down");

    for (let step = 1; step <= 8; step++) await lane.browser("mouse", "move", String(Math.round(points.from.x + (points.to.x - points.from.x) * step / 8)), String(Math.round(points.from.y + (points.to.y - points.from.y) * step / 8)));

    await lane.browser("mouse", "up");
    await lane.until(`document.querySelector('.shelf .todo-row[data-id]').dataset.id === '${three}'`);
    expect(await rows(lane)).toEqual([three, one, two]);
    expect((await lane.calls("todo.reorder")).map(call => call.params)).toEqual([{ id: three, before: one }]);
    expect(await lane.evaluate<boolean>("!!document.querySelector('.drawer[data-open=\"true\"]')")).toBe(false);
    expect(await lane.evaluate<boolean>(`document.activeElement === document.querySelector('.shelf [data-id="${three}"] .todo-row-button')`)).toBe(true);
  }, 120_000);

  it("u490a_tab_space_enter_reorders", async () => {
    const [one, two, three] = await rows(lane);

    await lane.browser("focus", ".shelf-add");
    await lane.tabTo(`Move down@${one}`);
    await lane.browser("press", "Space");
    await lane.until(`document.querySelector('.shelf .todo-row[data-id]').dataset.id === '${two}'`);
    expect(await rows(lane)).toEqual([two, one, three]);
    expect(await lane.evaluate<boolean>(`document.activeElement === document.querySelector('.shelf [data-id="${one}"] .todo-row-button')`)).toBe(true);
    await lane.tabTo(`Move up@${one}`);
    await lane.browser("press", "Enter");
    await lane.until(`document.querySelector('.shelf .todo-row[data-id]').dataset.id === '${one}'`);
    expect(await rows(lane)).toEqual([one, two, three]);
    expect((await lane.calls("todo.reorder")).map(call => call.params)).toEqual([{ id: one, before: three }, { id: one, before: two }]);
  }, 120_000);

  it("u491a_owned_tooltips_reveal_titles_and_blockers", async () => {
    const [one, , three] = await rows(lane);

    await lane.browser("hover", `.shelf [data-id="${one}"] .shelf-title`);
    await lane.until("[...document.querySelectorAll('[role=tooltip]')].some(tip => tip.textContent === 'first')");
    expect(await lane.evaluate<string[]>("[...document.querySelectorAll('[role=tooltip]')].map(tip => tip.textContent)")).toContain("first");
    await lane.browser("hover", `.shelf [data-id="${three}"] .todo-blocker`);
    await lane.until("[...document.querySelectorAll('[role=tooltip]')].some(tip => tip.textContent === 'Waits on first')");
    expect(await lane.evaluate<string[]>("[...document.querySelectorAll('[role=tooltip]')].map(tip => tip.textContent)")).toContain("Waits on first");
  }, 120_000);

  it("u492a_failed_reorder_dismisses_the_title_tooltip", async () => {
    const initial = await rows(lane);
    const one = initial[0]!;

    await lane.browser("hover", `.shelf [data-id="${one}"] .shelf-title`);
    await lane.until("!!document.querySelector('[role=tooltip]')");
    await lane.evaluate("(() => { window.__fake.failNext(-32001, 'cannot move this Todo'); return true; })()");
    await lane.browser("focus", `.shelf [data-id="${one}"] .todo-row-button`);
    await lane.browser("press", "Alt+ArrowDown");
    await lane.until("document.querySelector('.shelf [role=alert]')?.textContent.includes('cannot move this Todo')");
    expect(await lane.evaluate<number>("document.querySelectorAll('[role=tooltip]').length")).toBe(0);
    expect(await rows(lane)).toEqual(initial);
  }, 120_000);

  it("u491a_bulk_question_focus", async () => {
    await ask();
    expect(await lane.evaluate<string[]>("[...document.querySelectorAll('[role=\"alertdialog\"] li')].map(item => item.textContent)")).toHaveLength(2);
    expect(await lane.evaluate<string>("document.querySelector('[role=\"alertdialog\"]').getAttribute('aria-labelledby') !== null ? document.activeElement.textContent : ''")).toBe("Keep");

    for (let press = 0; press < 4; press++) {
      await lane.browser("press", "Tab");
      expect(await inDialog(lane)).toBe(true);
    }

    for (let press = 0; press < 4; press++) {
      await lane.browser("press", "Shift+Tab");
      expect(await inDialog(lane)).toBe(true);
    }

    await lane.capture("U491a", "question");
  }, 120_000);

  it("u491a_bulk_question_cancels", async () => {
    for (const how of ["Escape", "Keep", "outside"]) {
      await ask();
      await lane.resetCalls();

      if (how === "Escape") await lane.browser("press", "Escape");
      else if (how === "Keep") await lane.browser("click", '[role="alertdialog"] .shelf-question-words button:first-child');
      else {
        await lane.browser("mouse", "move", "100", "100");
        await lane.browser("mouse", "down");
        await lane.browser("mouse", "up");
      }

      await lane.until("!document.querySelector('[role=\"alertdialog\"]')");
      await lane.until("document.activeElement?.textContent === 'Delete 2'");
      expect(await lane.calls("todo.delete")).toEqual([]);
      expect(await rows(lane)).toHaveLength(3);
      expect(await lane.evaluate<boolean>("document.querySelector('.shelf-bulk').textContent.includes('2 selected')")).toBe(true);
      await lane.browser("click", ".shelf-bulk-line button:nth-of-type(2)");
    }
  }, 180_000);

  it("u491a_bulk_delete_readback", async () => {
    const [one, two] = await ask();
    const third = (await rows(lane)).find((id) => id !== one && id !== two)!;

    await lane.resetCalls();
    await lane.evaluate("(() => { window.__fake.failNext(-32001, 'denied'); return true; })()");
    await lane.browser("click", '[role="alertdialog"] .shelf-question-words button:last-child');
    await lane.until("document.querySelector('.shelf-bulk')?.textContent.includes('denied')");
    // One target went; the failed one stays selected with its failure text, and the third was never selected.
    expect(await rows(lane)).toHaveLength(2);
    expect(await rows(lane)).toContain(third);
    expect(await lane.evaluate<string>("document.querySelector('.shelf-bulk').textContent")).toContain("denied");
    await lane.capture("U491a", "failure");
    await lane.browser("click", ".shelf-bulk-line button:nth-of-type(1)");
    await lane.until("!!document.querySelector('[role=\"alertdialog\"]')");
    await lane.browser("click", '[role="alertdialog"] .shelf-question-words button:last-child');
    await lane.until("!document.querySelector('.shelf-bulk')");
    expect(await rows(lane)).toEqual([third]);
    // The injected failure is refused before the Daemon sees it, so the Daemon took each target once.
    expect((await lane.calls("todo.delete")).map((call) => call.params.id).sort()).toEqual([one, two].sort());
  }, 120_000);

  it("u491a_pad_bulk_delete", async () => {
    await lane.evaluate(`(async () => {
      [...document.querySelectorAll('[role="tab"]')].find(tab => tab.textContent.startsWith('Pads')).click();
      return true;
    })()`);
    await lane.until("document.querySelectorAll('.shelf .pad-name').length >= 3");

    const padCount = await lane.evaluate<number>("document.querySelectorAll('.shelf .pad-name').length");

    await lane.browser("focus", ".shelf .pad-name");
    await lane.browser("press", "Meta+Enter");
    await lane.browser("press", "Shift+ArrowDown");
    await lane.until("document.querySelector('.shelf-bulk')?.textContent.includes('2 selected')");
    await lane.resetCalls();
    await lane.browser("click", ".shelf-bulk-line button:nth-of-type(1)");
    await lane.until("!!document.querySelector('[role=\"alertdialog\"]')");
    expect(await lane.evaluate<number>("document.querySelectorAll('[role=\"alertdialog\"] li').length")).toBe(2);
    await lane.evaluate("(() => { window.__fake.failNext(-32001, 'pad denied'); return true; })()");
    await lane.browser("click", '[role="alertdialog"] .shelf-question-words button:last-child');
    await lane.until("document.querySelector('.shelf-bulk')?.textContent.includes('pad denied')");
    expect((await lane.calls("pad.delete")).length).toBe(1);
    expect(await lane.evaluate<number>("document.querySelectorAll('.shelf .pad-name').length")).toBe(padCount - 1);
  }, 120_000);

  it("u491a_tab_space_enter_bulk_deletes", async () => {
    const [one, two, three] = await rows(lane);

    expect([one, two, three]).not.toContain(undefined);

    await lane.resetCalls();
    await lane.browser("focus", ".shelf-add");
    await lane.tabTo(`Select@${one}`);
    await lane.browser("press", "Space");
    await lane.tabTo(`Select@${two}`);
    await lane.browser("press", "Enter");
    await lane.tabTo("Delete 2");
    await lane.browser("press", "Enter");
    await lane.until("!!document.querySelector('[role=\"alertdialog\"]')");
    expect(await lane.focused()).toBe("Keep");
    await lane.tabTo("Delete 2");
    await lane.browser("press", "Space");
    await lane.until("!document.querySelector('[role=\"alertdialog\"]')");
    await lane.until(`JSON.stringify([...document.querySelectorAll('.shelf .todo-row[data-id]')].map(row => Number(row.dataset.id))) === '[${three}]'`);
    expect((await lane.calls("todo.delete")).map((call) => call.params)).toEqual([{ id: one }, { id: two }]);
    expect(await lane.evaluate<boolean>("document.activeElement.isConnected && !!document.activeElement.closest('.shelf')")).toBe(true);
  }, 120_000);

  it("u491a_drawer_question_keeps_the_Drawer_open_on_Escape", async () => {
    await lane.browser("click", ".shelf .todo-row-button");
    await lane.until("!!document.querySelector('.drawer[data-open=\"true\"] .todo-detail')");
    await lane.browser("click", '.drawer [aria-label="Delete Todo"]');
    await lane.until("document.activeElement?.textContent === 'Keep'");
    await lane.browser("press", "Escape");
    await lane.until("!document.querySelector('[role=\"alertdialog\"]')");
    expect(await lane.evaluate<boolean>("!!document.querySelector('.drawer[data-open=\"true\"]')")).toBe(true);
    expect(await lane.evaluate<string>("document.activeElement.getAttribute('aria-label')")).toBe("Delete Todo");
  }, 120_000);

  it("u491a_shelf_tabs", async () => {
    const state = () => lane.evaluate<{ selected: string[]; panel: string | null; panels: string[] }>(`({
      selected: [...document.querySelectorAll('.shelf [role="tab"]')].filter(tab => tab.getAttribute('aria-selected') === 'true').map(tab => tab.id),
      panel: document.querySelector('.shelf [role="tabpanel"]:not([hidden])')?.getAttribute('aria-labelledby') ?? null,
      panels: [...document.querySelectorAll('.shelf [role="tabpanel"]')].map(panel => panel.getAttribute('aria-labelledby')),
    })`);

    expect(await state()).toEqual({ selected: ["shelf-tab-todos"], panel: "shelf-tab-todos", panels: ["shelf-tab-todos"] });
    await lane.browser("click", '.shelf [role="tab"][id="shelf-tab-pads"]');
    await lane.until("document.querySelector('.shelf [role=\"tab\"][aria-selected=\"true\"]')?.id === 'shelf-tab-pads'");
    expect(await state()).toEqual({ selected: ["shelf-tab-pads"], panel: "shelf-tab-pads", panels: ["shelf-tab-pads"] });

    for (const [key, id] of [["ArrowLeft", "todos"], ["ArrowRight", "pads"], ["Home", "todos"], ["End", "pads"]] as const) {
      await lane.browser("press", key);
      await lane.until(`document.querySelector('.shelf [role="tab"][aria-selected="true"]')?.id === 'shelf-tab-${id}'`);
      expect((await state()).selected).toEqual([`shelf-tab-${id}`]);
    }

    // The Shelf toggle sits between the tabs and their panel; the panel is the next stop after it.
    const panelHolds = "!!document.querySelector('.shelf [role=\"tabpanel\"]:not([hidden])').contains(document.activeElement)";

    for (let press = 0; press < 4 && !(await lane.evaluate<boolean>(panelHolds)); press++) await lane.browser("press", "Tab");

    expect(await lane.evaluate<boolean>(panelHolds)).toBe(true);
    await lane.browser("click", '.shelf [role="tab"][id="shelf-tab-todos"]');
    await lane.until("document.querySelectorAll('.shelf .todo-row').length === 3");
  }, 120_000);

  it("u491a_scope_toggle", async () => {
    const toggle = () => lane.evaluate<{ pressed: string | null; label: string | null; count: number }>("({ pressed: document.querySelector('.scope-control').getAttribute('aria-pressed'), label: document.querySelector('.scope-control').getAttribute('aria-label'), count: document.querySelectorAll('.scope-control').length })");

    await lane.browser("click", '.rail [data-id="backend"] .name');
    await lane.until("!!document.querySelector('.scope-control')");

    const before = await toggle();

    expect(before.count).toBe(1);
    await lane.browser("click", ".scope-control");
    await lane.until(`document.querySelector('.scope-control').getAttribute('aria-pressed') !== '${before.pressed}'`);

    const after = await toggle();

    expect(after.label).not.toBe(before.label);
    expect(after.count).toBe(1);
    await lane.browser("click", ".scope-control");
    await lane.until(`document.querySelector('.scope-control').getAttribute('aria-pressed') === '${before.pressed}'`);
    expect(await toggle()).toEqual(before);
  }, 120_000);

  it("u491a_home_menu", async () => {
    // An Agent's own Todo lists under that Agent in the Rail, where its Home is moved by the menu.
    const owner = "{ kind: 'agent', id: 'agent-1', parent: null }";

    const made = await lane.evaluate<number>(`(async () => {
      window.__fake.actAs(${owner});
      const made = await window.__fake.app.rpc('todo.create', { title: 'agent todo', body: null, blockers: null });
      window.__fake.actAs({ kind: 'user', id: 'you', parent: null });
      document.querySelector('.rail [data-id="agent-1"] [data-pads-control]').click();
      return made.id;
    })()`);

    const nested = `.rail [data-todo="${made}"]`;
    const homeOf = () => lane.evaluate<string>(`document.querySelector('${nested} [data-todo-home]').textContent`);

    await lane.until(`!!document.querySelector('${nested}')`);

    const was = await homeOf();

    await lane.browser("focus", `${nested} .todo-row-button`);
    await lane.until(`!!document.querySelector('${nested} .todo-move')`);

    // Escape closes the menu and returns focus to the trigger.
    await lane.browser("focus", `${nested} .todo-move`);
    await lane.browser("press", "Enter");
    await lane.until("!!document.querySelector('[role=\"menu\"]')");
    await lane.browser("press", "Escape");
    await lane.until("!document.querySelector('[role=\"menu\"]')");
    expect(await lane.evaluate<boolean>(`document.activeElement === document.querySelector('${nested} .todo-move')`)).toBe(true);

    // A failed move shows its message and keeps the Home.
    await lane.resetCalls();
    await lane.evaluate("(() => { window.__fake.failNext(-32001, 'move denied'); return true; })()");
    await lane.browser("press", "Enter");
    await lane.until("!!document.querySelector('[role=\"menu\"]')");
    await lane.browser("press", "Enter");
    await lane.until(`document.querySelector('${nested}').textContent.includes('move denied')`);
    expect(await homeOf()).toBe(was);
    await lane.capture("U491a", "home-failure");

    // The keyboard picks a Home.
    await lane.browser("focus", `${nested} .todo-move`);
    await lane.browser("press", "Enter");
    await lane.until("!!document.querySelector('[role=\"menu\"]')");
    await lane.browser("press", "Enter");
    await lane.until(`document.querySelector('${nested} [data-todo-home]').textContent !== ${JSON.stringify(was)}`);
    expect((await lane.calls("todo.move")).map((call) => call.params.id)).toContain(made);
  }, 120_000);
});
