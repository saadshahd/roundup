import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { chromePath, openPage } from "../testing/chromePage";
import type { ChromePage } from "../testing/chromePage";

const found = chromePath();

const run = found || process.env.CI ? describe : describe.skip;

let page: ChromePage;

beforeAll(async () => {
  if (!found) throw new Error("U490a needs Chromium");
  page = await openPage(found);
}, 90_000);

afterAll(async () => { await page?.close(); });

type Point = { x: number; y: number };

type State = { ids: number[]; active: string | null; drawer: boolean; reorders: unknown[] };

type Frame = { t: number; tops: Record<string, number> };

type List = { name: string; rows: string; setup: () => Promise<void> };

/** The Rail of the `tree-40` harness with its first Agent's Todos shown. */
const nested: List = {
  name: "an Agent's Todos in the Rail",
  rows: "[data-pads] .todo-row-button",
  setup: async () => {
    await page.load("/harness.html?seed=tree-40", 1280, 820);
    await page.until(`() => Boolean(document.querySelector('.rail-pads-control')) && Boolean(window.__fake)`);
    await page.call(`() => document.querySelector('.rail-pads-control').click()`);
    await page.until(`() => document.querySelectorAll("[data-pads] .todo-row-button").length > 1`);
  },
};

/** The Shelf's `todos` list of the `tree-40` harness, every Todo shown. */
const shelf: List = {
  name: "the Shelf's Todos",
  rows: ".shelf .todo-list .todo-row-button",
  setup: async () => {
    await page.load("/harness.html?seed=tree-40", 1280, 820);
    await page.until(`() => Boolean(document.querySelector('.shelf [role=tab]')) && Boolean(document.querySelector('[data-part="branch"][aria-expanded] > .rail-row')) && Boolean(window.__fake)`);
    await page.call(`() => {
      [...document.querySelectorAll('.shelf [role=tab]')].find(t => t.textContent.startsWith('Todos')).click();
      document.querySelector('[data-part="branch"][aria-expanded] > .rail-row').click();
    }`);
    await page.until(`() => Boolean(document.querySelector('.shelf button[aria-pressed]'))`);
    await page.call(`() => document.querySelector('.scope-control[aria-pressed="false"]').click()`);
    await page.until(`() => document.querySelectorAll(".shelf .todo-list .todo-row-button").length > 2`);
  },
};

const centre = (rows: string, n: number) => page.call<Point>(`(rows, n) => {
  const b = document.querySelectorAll(rows)[n].getBoundingClientRect();
  return {x: b.left + b.width / 2, y: b.top + b.height / 2};
}`, rows, n);

const state = (rows: string) => page.call<State>(`rows => ({
  ids: [...document.querySelectorAll(rows)].map(b => Number(/#(\\d+)/.exec(b.getAttribute('aria-label'))[1])),
  active: document.activeElement?.getAttribute('aria-label') ?? null,
  drawer: document.querySelector('[aria-label=Drawer]')?.getAttribute('data-open') === 'true',
  reorders: window.__fake.app.calls.filter(c => c.method === 'todo.reorder').map(c => c.params),
})`, rows);

/** A pointer drag in small steps, as a hand makes one. */
const drag = async (from: Point, to: Point, release = true) => {
  await page.pointer("down", from.x, from.y);

  for (let step = 1; step <= 8; step++) await page.pointer("move", from.x + ((to.x - from.x) * step) / 8, from.y + ((to.y - from.y) * step) / 8);

  if (release) await page.pointer("up", to.x, to.y);
};

const settled = () => new Promise((resolve) => setTimeout(resolve, 400));

const ALT = 1;

/** Every row's on-screen top, once per animation frame, from now until `stop`. */
const sample = (rows: string) => page.call<void>(`rows => {
  window.__frames = [];
  window.__sampling = true;
  const read = () => {
    window.__frames.push({t: performance.now(), tops: Object.fromEntries([...document.querySelectorAll(rows)].map(b => [/#(\\d+)/.exec(b.getAttribute('aria-label'))[1], b.closest('.todo-row').getBoundingClientRect().top]))});
    if (window.__sampling) requestAnimationFrame(read);
  };
  read();
}`, rows);

const frames = () => page.call<Frame[]>(`() => { window.__sampling = false; return window.__frames; }`);

for (const list of [nested, shelf]) {
  const { rows } = list;

  run(`u490a ${list.name} order on the shared Sortable`, () => {
    it("u490a_pointer_reorders", async () => {
      await page.scheme("light");
      await list.setup();

      const before = await state(rows);
      const last = before.ids.length - 1;

      await drag(await centre(rows, last), await centre(rows, 0));
      await page.until(`() => window.__fake.app.calls.some(c => c.method === 'todo.reorder')`);
      await settled();

      const after = await state(rows);

      expect(after.reorders).toEqual([{ id: before.ids[last], before: before.ids[0] }]);
      expect(after.ids[0]).toBe(before.ids[last]);
      expect(after.drawer).toBe(false);
      expect(after.active).toContain(`#${before.ids[last]} `);
    }, 60_000);

    it("u490a_cancel_and_noop", async () => {
      await page.scheme("light");
      await list.setup();

      const first = await centre(rows, 0);
      const last = await centre(rows, (await state(rows)).ids.length - 1);

      await drag(last, first, false);
      expect(await page.call(`rows => {
        const gap = document.querySelector(rows)?.closest('[data-plain]')?.querySelector('[data-dnd-placeholder]');
        const style = gap && getComputedStyle(gap);
        return gap ? [style.outlineStyle, style.borderRadius] : null;
      }`, rows)).toEqual(["none", "0px"]);
      await page.press("Escape", "Escape");
      await page.pointer("up", first.x, first.y);
      await settled();
      // Past the activation distance and back to the row's own place: no move.
      await page.pointer("down", last.x, last.y);
      await page.pointer("move", last.x, last.y - 12);
      await page.pointer("move", last.x, last.y);
      await page.pointer("up", last.x, last.y);
      await settled();
      expect(await state(rows)).toMatchObject({ drawer: false, reorders: [] });
    }, 60_000);

    it("u490a_a_click_opens_the_Todo_not_a_drag", async () => {
      await page.scheme("light");
      await list.setup();

      const point = await centre(rows, 0);

      await page.pointer("down", point.x, point.y);
      await page.pointer("up", point.x, point.y);
      await page.until(`() => document.querySelector('[aria-label=Drawer]')?.getAttribute('data-open') === 'true'`);
      expect(await state(rows)).toMatchObject({ drawer: true, reorders: [] });
    }, 60_000);

    it("u490a_Enter_on_a_row_button_opens_it_without_drag_semantics", async () => {
      await page.scheme("light");
      await list.setup();
      await page.call(`rows => document.querySelectorAll(rows)[0].focus()`, rows);
      // The shared Tooltip may describe the row; Sortable adds no drag role or disabled state.
      expect(await page.call<string[]>(`rows => [...document.querySelectorAll(rows)[0].attributes].map(a => a.name + "=" + a.value).filter(a => /^aria-(roledescription|grabbed|disabled)=/.test(a))`, rows)).toEqual([]);
      await page.press("Enter", "Enter");
      await page.until(`() => document.querySelector('[aria-label=Drawer]')?.getAttribute('data-open') === 'true'`);
      expect(await state(rows)).toMatchObject({ drawer: true, reorders: [] });
    }, 60_000);

    it("u490a_keyboard_reorders", async () => {
      await page.scheme("light");
      await list.setup();

      const before = await state(rows);

      await page.call(`rows => document.querySelectorAll(rows)[1].focus()`, rows);
      await page.press("ArrowUp", "ArrowUp", ALT);
      await page.until(`() => window.__fake.app.calls.some(c => c.method === 'todo.reorder')`);
      await settled();

      const after = await state(rows);

      expect(after.reorders).toEqual([{ id: before.ids[1], before: before.ids[0] }]);
      expect(after.ids.slice(0, 2)).toEqual([before.ids[1], before.ids[0]]);
      expect(after.active).toContain(`#${before.ids[1]} `);

      await page.call(`rows => document.querySelectorAll(rows)[0].focus()`, rows);
      await page.press("ArrowUp", "ArrowUp", ALT);
      await settled();
      expect((await state(rows)).reorders).toHaveLength(1);
    }, 60_000);

    it("u490a_reorder_motion", async () => {
      await page.scheme("light");
      await list.setup();

      const ids = (await state(rows)).ids;

      await page.call(`rows => document.querySelectorAll(rows)[0].focus()`, rows);
      await sample(rows);
      await page.press("ArrowDown", "ArrowDown", ALT);
      await settled();

      const run1 = await frames();
      const first = run1[0]!.tops;
      const final = run1.at(-1)!.tops;
      const moved = Object.keys(final).filter((id) => Math.abs(final[id]! - first[id]!) > 1);

      expect(moved.sort()).toEqual([String(ids[0]), String(ids[1])].sort());

      for (const id of Object.keys(final).filter((other) => !moved.includes(other))) expect(new Set(run1.map((frame) => Math.round(frame.tops[id]!))).size).toBe(1);

      for (const id of moved) {
        const series = run1.map((frame) => ({ t: frame.t, top: frame.tops[id]! }));
        const begin = series.find((point) => Math.abs(point.top - first[id]!) >= 1)!;
        const end = series.find((point) => point.t >= begin.t && Math.abs(point.top - final[id]!) < 1)!;

        // The row starts where it showed, not at its final place.
        expect(Math.abs(begin.top - first[id]!)).toBeLessThan(Math.abs(final[id]! - first[id]!) / 2);
        expect(end.t - begin.t).toBeGreaterThan(60);
        expect(end.t - begin.t).toBeLessThanOrEqual(200);

        // While far from its place, no frame covers more than half of what remains.
        for (let at = series.indexOf(begin); at < series.indexOf(end); at++) {
          const remaining = Math.abs(final[id]! - series[at]!.top);

          if (remaining > 12) expect(Math.abs(series[at + 1]!.top - series[at]!.top)).toBeLessThanOrEqual(remaining / 2 + 1);
        }
      }
    }, 60_000);

    it("u490a_reorder_motion_continues_from_the_screen_when_interrupted", async () => {
      await page.scheme("light");
      await list.setup();

      const id = String((await state(rows)).ids[0]);

      await page.call(`rows => document.querySelectorAll(rows)[0].focus()`, rows);
      await sample(rows);
      await page.press("ArrowDown", "ArrowDown", ALT);
      // About half the distance covered: the row has left its old place and not reached its new one.
      await page.until(`() => {
        const id = ${JSON.stringify(id)};
        const f = window.__frames, a = f[0].tops[id], now = f.at(-1).tops[id];
        const others = Object.keys(f[0].tops).filter(k => k !== id);
        return Math.abs(now - a) > 5 && others.some(k => Math.abs(f.at(-1).tops[k] - f[0].tops[k]) > 5);
      }`);

      const mark = await page.call<number>(`() => window.__frames.length`);

      await page.press("ArrowDown", "ArrowDown", ALT);
      await settled();

      const series = (await frames()).map((frame) => frame.tops[id]!);
      const steps = series.slice(1).map((top, at) => Math.abs(top - series[at]!));

      // Across the second move no frame jumps: the largest step stays a frame's travel, never the old or the final place.
      const travel = Math.max(...steps.slice(0, mark - 1));

      expect(Math.max(...steps.slice(mark - 1, mark + 4))).toBeLessThanOrEqual(travel + 12);
    }, 60_000);

    it("u490a_reorder_motion_under_reduced_motion_is_one_frame", async () => {
      await page.scheme("light", undefined, "motion");
      await list.setup();
      await page.call(`rows => document.querySelectorAll(rows)[0].focus()`, rows);
      await sample(rows);
      await page.press("ArrowDown", "ArrowDown", ALT);
      await settled();

      const run3 = await frames();
      const first = run3[0]!.tops;
      const final = run3.at(-1)!.tops;
      const changed = run3.filter((frame) => Object.keys(first).some((id) => Math.abs(frame.tops[id]! - first[id]!) > 1));

      expect(changed.length).toBeGreaterThan(0);
      expect(changed[0]!.tops).toEqual(final);
    }, 60_000);
  });
}
