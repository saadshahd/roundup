import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { openLane } from "./laneBrowser";
import type { LaneBrowser } from "./laneBrowser";

/** Record rendered Checks for the independent critic; these tests assert actions and readback only. */
const read = async (lane: LaneBrowser, step: string): Promise<void> => {
  await lane.capture(step.startsWith("reorder") ? "U490a" : step === "baseline" || step === "dense-failure" ? "U492a" : "U491a", step);
};

describe.each([[1280, "light"], [1512, "dark"], [1280, "dark"], [1512, "light"]] as const)("u492a at %i %s", (width, scheme) => {
  let lane: LaneBrowser;

  beforeAll(async () => { lane = await openLane(width, scheme, "u492a"); }, 180_000);

  afterAll(async () => { await lane?.close(); });

  it("u492a_rendered_checks", async () => {
    await lane.seedTodos(["first", "second", "third"]);

    await read(lane, "baseline");

    // Reorder by Move down, then a failed one.
    const initial = await lane.order();
    const [one] = initial;

    await lane.browser("click", `.shelf [data-todo="${one}"] [aria-label="Move down"]`);
    await lane.until(`document.querySelector('.shelf .todo-row[data-id]').dataset.id !== '${one}'`);
    await read(lane, "reorder");
    await lane.evaluate("(() => { window.__fake.failNext(-32001, 'cannot reorder'); return true; })()");
    // A click anywhere clears a row's failure, and the fake Daemon answers within the click, so the chord stands in for the control.
    await lane.browser("focus", `.shelf [data-todo="${one}"] .todo-row-button`);
    await lane.browser("press", "Alt+ArrowUp");
    await lane.until("document.querySelector('.shelf').textContent.includes('cannot reorder')");
    await read(lane, "reorder-failure");
    expect((await lane.calls("todo.reorder")).map(call => call.params)).toEqual([{ id: one, before: initial[2] }]);

    // Bulk delete: cancel, then a partial failure.
    const [first, second] = await lane.order() ;

    await lane.browser("click", `.shelf [data-todo="${first}"] [aria-label="Select"]`);
    await lane.browser("click", `.shelf [data-todo="${second}"] [aria-label="Select"]`);
    await lane.browser("click", ".shelf-bulk-line button:nth-of-type(1)");
    await lane.until("!!document.querySelector('[role=\"alertdialog\"]')");
    await read(lane, "question");
    await lane.browser("press", "Escape");
    await lane.until("!document.querySelector('[role=\"alertdialog\"]')");
    await lane.browser("click", ".shelf-bulk-line button:nth-of-type(1)");
    await lane.until("!!document.querySelector('[role=\"alertdialog\"]')");
    await lane.evaluate("(() => { window.__fake.failNext(-32001, 'cannot delete'); return true; })()");
    await lane.browser("click", '[role="alertdialog"] .shelf-question-words button:last-child');
    await lane.until("document.querySelector('.shelf-bulk')?.textContent.includes('cannot delete')");
    await read(lane, "bulk-failure");
    expect(await lane.order()).toEqual([first, initial[2]]);
    expect((await lane.calls("todo.delete")).map(call => call.params)).toEqual([{ id: second }]);

    // Tabs and the Home menu of an Agent's own Todo.
    await lane.browser("click", '.shelf [role="tab"][id="shelf-tab-pads"]');
    await lane.until("document.querySelector('.shelf [role=\"tab\"][aria-selected=\"true\"]')?.id === 'shelf-tab-pads'");
    await read(lane, "tabs");
    await lane.browser("click", '.shelf [role="tab"][id="shelf-tab-todos"]');

    const made = await lane.evaluate<number>(`(async () => {
      window.__fake.actAs({ kind: 'agent', id: 'agent-1', parent: null });
      const made = await window.__fake.app.rpc('todo.create', { title: 'agent todo', body: null, blockers: null });
      window.__fake.actAs({ kind: 'user', id: 'you', parent: null });
      document.querySelector('.rail [data-id="agent-1"] [data-pads-control]').click();
      return made.id;
    })()`);

    await lane.until(`!!document.querySelector('.rail [data-todo="${made}"]')`);
    await lane.browser("hover", `.rail [data-todo="${made}"]`);
    await lane.browser("focus", `.rail [data-todo="${made}"] .todo-move`);
    await lane.browser("press", "Enter");
    await lane.until("!!document.querySelector('[role=\"menu\"]')");
    await read(lane, "home-menu");
    await lane.browser("press", "Escape");
  }, 240_000);

  it("u492a_dense_failure", async () => {
    // U154's densest data: a 120-character title and a Todo that waits on ten others.
    await lane.seedTodos(["x", "y", "z"]);
    // The Shelf lists the selected Agent's own Todos, so the Agent makes them.
    await lane.evaluate(`(async () => {
      const rpc = window.__fake.app.rpc;
      window.__fake.actAs({ kind: 'agent', id: 'tokens', parent: null });
      const ids = [];
      for (let at = 0; at < 10; at++) ids.push((await rpc('todo.create', { title: 'blocker ' + at, body: null, blockers: null })).id);
      const long = await rpc('todo.create', { title: 't'.repeat(120), body: null, blockers: null });
      const blocked = await rpc('todo.create', { title: 'blocked', body: null, blockers: null });
      await rpc('todo.setBlockers', { id: blocked.id, blockers: ids });
      window.__fake.actAs({ kind: 'user', id: 'you', parent: null });
      return long.id;
    })()`);
    await lane.until("document.querySelectorAll('.shelf .todo-row').length >= 13");

    const ids = await lane.order();

    // An injected move error shows on its row.
    await lane.evaluate("(() => { window.__fake.failNext(-32001, 'cannot move this Todo because the Daemon refused it'); return true; })()");
    await lane.browser("focus", `.shelf [data-todo="${ids[0]}"] .todo-row-button`);
    await lane.browser("press", "Alt+ArrowDown");
    await lane.until("document.querySelector('.shelf').textContent.includes('cannot move this Todo')");
    await read(lane, "dense-failure");
    expect(await lane.order()).toContain(ids[0]);
    expect(await lane.calls("todo.reorder")).toEqual([]);

    // An injected delete error shows in the bar.
    await lane.browser("click", `.shelf [data-todo="${ids[1]}"] [aria-label="Select"]`);
    await lane.browser("click", `.shelf [data-todo="${ids[2]}"] [aria-label="Select"]`);
    await lane.browser("click", ".shelf-bulk-line button:nth-of-type(1)");
    await lane.until("!!document.querySelector('[role=\"alertdialog\"]')");
    await lane.evaluate("(() => { window.__fake.failNext(-32001, 'cannot delete this Todo because the Daemon refused it'); return true; })()");
    await lane.browser("click", '[role="alertdialog"] .shelf-question-words button:last-child');
    await lane.until("document.querySelector('.shelf-bulk')?.textContent.includes('cannot delete this Todo')");
    await read(lane, "dense-failure");
    expect(await lane.order()).toContain(ids[1]);
    expect(await lane.order()).not.toContain(ids[2]);
    expect(await lane.calls("todo.delete")).toHaveLength(1);
  }, 240_000);
});
