import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterAll, beforeAll, expect, it } from "vitest";
import { captureDir } from "../testing/captures";
import { appServer, type AppServer } from "../testing/appServer";

const exec = promisify(execFile);

const profile = mkdtempSync(join(tmpdir(), "u484-"));

const session = `u484-${process.pid}`;

let server: AppServer;

let origin: string;

const browser = async (...args: string[]) => (await exec("agent-browser", ["--session", session, "--json", ...args], { env: { ...process.env, AGENT_BROWSER_DEFAULT_TIMEOUT: "15000" }, maxBuffer: 8 * 1024 * 1024, timeout: 25_000 })).stdout;

const read = async <T,>(expression: string): Promise<T> => {
  // SAFETY: the envelope is agent-browser's result for the constant expressions below.
  const reply = JSON.parse(await browser("eval", expression)) as { success: boolean; data: { result: T }; error?: string };

  if (!reply.success) throw new Error(reply.error);

  return reply.data.result;
};

beforeAll(async () => {
  server = await appServer();
  origin = server.resolvedUrls.local[0]!;
  await browser("--profile", profile, "open", new URL(new URL("./u484Fixture.tsx", import.meta.url).pathname.replace("Fixture.tsx", ".html"), origin).href);
  await browser("set", "viewport", "1512", "950");
  await browser("set", "media", "dark");
}, 90_000);

afterAll(async () => {
  try { await browser("close"); } finally { await server?.close(); rmSync(profile, { recursive: true, force: true }); }
});


const load = async (dense = false, query = dense ? "?dense" : "") => {
  await browser("open", new URL(`${new URL("./u484Fixture.tsx", import.meta.url).pathname.replace("Fixture.tsx", ".html")}${query}`, origin).href);
  await browser("wait", "--fn", `document.querySelectorAll('.sortable-fixture > .sortable-row:not([data-dnd-placeholder])').length === ${dense ? 40 : 3}`);
};

const order = () => read<string[]>(`[...document.querySelectorAll('.sortable-fixture > .sortable-row:not([data-dnd-placeholder])')].map(el => el.dataset.id)`);

const committed = async (value: string, count = 1) => {
  await browser("wait", "--fn", `document.querySelector('[aria-label="Commits"]').textContent === '${count}'`);
  expect(await read<string>(`document.querySelector('[aria-label="Committed order"]').textContent`)).toBe(value);
  expect((await order()).join(",")).toBe(value);
  expect(await read<string>(`document.querySelector('[aria-label="Commits"]').textContent`)).toBe(String(count));
};

const capture = async (id: string, name: string) => {
  const directory = captureDir(id);

  if (!directory) return;
  mkdirSync(directory, { recursive: true });
  await browser("screenshot", join(directory, `${name}.png`));
  writeFileSync(join(directory, `${name}.checks.json`), JSON.stringify(await read("window.__checks()"), null, 2));
};

const keyboard = async (id: string) => {
  await browser("focus", `[aria-label="Move ${id}"]`);
  await browser("press", "Space");
  await browser("wait", "--fn", `document.querySelector('[data-id="${id}"]').dataset.dragging === 'true'`);
};

const down = async (expected: string[]) => {
  await browser("press", "ArrowDown");
  await browser("wait", "--fn", `document.querySelector('.sortable-fixture').dataset.sortableIndex === '${expected.indexOf(expected.includes("5") ? "5" : "A")}'`);
};

const drag = async (id: string, target: string) => {
  const point = async (selector: string) => read<{ x: number; y: number }>(`(() => { const rect = document.querySelector(${JSON.stringify(selector)}).getBoundingClientRect(); return { x: rect.x + rect.width / 2, y: rect.y + rect.height / 2 }; })()`);
  const start = await point(`[aria-label="Move ${id}"]`);
  await browser("mouse", "move", String(Math.round(start.x)), String(Math.round(start.y)));
  await browser("mouse", "down");
  await browser("mouse", "move", String(Math.round(start.x + 15)), String(Math.round(start.y)));
  await browser("wait", "--fn", `document.querySelector('[data-id="${id}"]').dataset.dragging === 'true'`);
  const end = await point(`[aria-label="Move ${target}"]`);

  await browser("mouse", "move", String(Math.round(end.x)), String(Math.round(end.y)));
};

it("u484_external_drop", async () => {
  const outside = async () => {
    const point = await read<{ x: number; y: number }>(`(() => { const rect = document.querySelector('[data-outside]').getBoundingClientRect(); return { x: rect.x + rect.width / 2, y: rect.y + rect.height / 2 }; })()`);

    for (const step of [0.25, 0.5, 0.75, 1]) await browser("mouse", "move", String(Math.round(28 + (point.x - 28) * step)), String(Math.round(20 + (point.y - 20) * step)));
    await browser("wait", "--fn", `document.querySelector('[data-outside]').hasAttribute('data-drop')`);
  };

  const dropped = () => read<string>(`document.querySelector('[aria-label=Dropped]').textContent`);

  await load(false, "?external");
  await drag("A", "A");
  await outside();
  await browser("mouse", "up");
  await browser("wait", "--fn", `document.querySelector('[aria-label=Dropped]').textContent === 'A>Rail'`);
  expect(await read<string>(`document.querySelector('[aria-label=Commits]').textContent`)).toBe("0");
  expect(await read<boolean>(`document.querySelector('[data-outside]').hasAttribute('data-drop')`)).toBe(false);
  expect(await order()).toEqual(["A", "B", "C"]);
  await browser("wait", "--fn", `document.activeElement?.getAttribute('aria-label') === 'Move A'`);

  await load(false, "?external");
  await drag("A", "A");
  await outside();
  await browser("press", "Escape");
  await browser("mouse", "up");
  await browser("wait", "--fn", `document.querySelector('[data-id=A]').dataset.dragging === 'false'`);
  expect(await dropped()).toBe("");
  expect(await read<string>(`document.querySelector('[aria-label=Commits]').textContent`)).toBe("0");
  expect(await read<boolean>(`document.querySelector('[data-outside]').hasAttribute('data-drop')`)).toBe(false);

  await load(false, "?external");
  await drag("A", "C");
  await browser("mouse", "up");
  await committed("B,C,A");
  expect(await dropped()).toBe("");
}, 90_000);

// U3800a/c: one drop look's behaviour — pointer drag, keyboard lift, Escape and a following click — through the installed shared Sortable.
it("u3800a_one_drag_fixture", async () => {
  const idle = `[...document.querySelectorAll('.sortable-fixture > .sortable-row:not([data-dnd-placeholder])')].map(el => el.dataset.id).join(',') === 'A,B,C' && document.querySelector('[data-id=A]').dataset.dragging === 'false'`;
  const commits = () => read<string>(`document.querySelector('[aria-label=Commits]').textContent`);
  const focusedA = () => browser("wait", "--fn", `document.activeElement?.getAttribute('aria-label') === 'Move A'`);

  await load();
  await drag("A", "C");
  await browser("wait", "--fn", `document.querySelector('.sortable-fixture').dataset.sortableIndex === '2'`);
  expect(await order()).toEqual(['A', 'B', 'C']);
  await browser("press", "Escape");
  await browser("mouse", "up");
  await browser("wait", "--fn", idle);
  expect(await commits()).toBe("0");
  await focusedA();

  await drag("A", "C");
  await browser("mouse", "up");
  await committed("B,C,A");
  await browser("click", '[aria-label="Inspect A"]');
  await browser("wait", "--fn", `document.querySelector('[aria-label=Actions]').textContent === '1'`);
  expect(await commits()).toBe("1");

  await load();
  const handle = await read<{x:number; y:number}>(`(() => { const b=document.querySelector('[aria-label="Move A"]').getBoundingClientRect(); return {x:Math.round(b.left+b.width/2),y:Math.round(b.top+b.height/2)}; })()`);

  await browser("mouse", "move", String(handle.x), String(handle.y));
  await browser("mouse", "down");
  await browser("mouse", "move", String(handle.x + 2), String(handle.y));
  await browser("mouse", "up");
  expect(await read<string>(`document.querySelector('[aria-label=Actions]').textContent`)).toBe("1");
  expect(await commits()).toBe("0");
  expect(await read<boolean>(`(() => {
    const row = document.querySelector('[data-id=A]');
    const transfer = new DataTransfer(); transfer.items.add(new File(['hello'], 'example.txt', {type: 'text/plain'}));
    const over = new DragEvent('dragover', { bubbles: true, cancelable: true, dataTransfer: transfer });
    row.dispatchEvent(over); row.dispatchEvent(new DragEvent('drop', { bubbles: true, cancelable: true, dataTransfer: transfer }));
    return over.defaultPrevented;
  })()`)).toBe(true);
  await browser("wait", "--fn", `document.querySelector('[aria-label=Files]').textContent === 'example.txt'`);
  expect(await commits()).toBe("0");
  expect(await read<number>(`document.querySelectorAll('[data-dragging=true]').length`)).toBe(0);

  await keyboard("A");
  await down(["B", "A", "C"]);
  await browser("press", "Escape");
  await browser("wait", "--fn", idle);
  expect(await commits()).toBe("0");
  await focusedA();

  await keyboard("A");
  await down(["B", "A", "C"]);
  await browser("press", "Space");
  await committed("B,A,C");
  await focusedA();

  // The same session exercises native path routing through the real App and fake event seam.
  await browser("open", new URL("harness.html?seed=tree-40", origin).href);
  await browser("wait", "--fn", "!!document.querySelector('.rail-row[data-id=tokens]')");
  await browser("click", ".rail-row[data-id=tokens] .name");
  await browser("wait", "--fn", "document.querySelector('.pane-body .xterm') !== null");

  const feed = (phase: string, paths: string[] = []) => read(`(() => {
    const b = document.querySelector('.pane-body').getBoundingClientRect();
    window.__fake.app.dropFiles({phase:${JSON.stringify(phase)},paths:${JSON.stringify(paths)},x:b.left+b.width/2,y:b.top+b.height/2});
    return true;
  })()`);

  for (const id of ["tokens", "migrate"]) {
    await read(`(() => {
      const box = document.querySelector('.rail-row[data-id="${id}"]').getBoundingClientRect();
      window.__fake.app.dropFiles({phase:'over',paths:[],x:box.x + box.width / 2,y:box.y + box.height / 2});
      return true;
    })()`);
    expect(await read<string[]>(`[...document.querySelectorAll('[data-drop=hover]')].map(row => row.dataset.id)`)).toEqual([id]);
  }

  await feed("over");
  await browser("wait", "--fn", "!!document.querySelector('.pane-body.drop-over')");
  expect(await read<number>("document.querySelectorAll('.drop-over,[data-drop=hover]').length")).toBe(1);
  await capture("U3800b", "centre-hover");
  await feed("leave");
  expect(await read<number>("document.querySelectorAll('.drop-over,[data-drop=hover]').length")).toBe(0);
  await feed("over");
  await browser("click", ".rail-row[data-id=migrate] .name");
  await browser("wait", "--fn", "document.querySelector('.rail-row[data-selected=true]')?.dataset.id === 'migrate'");
  expect(await read<number>("document.querySelectorAll('.drop-over').length")).toBe(0);
  await feed("over");
  const writes = "window.__fake.app.calls.filter(c=>c.method==='terminal.write').map(c=>({id:c.params.id,text:atob(c.params.data)}))";
  const before = await read<unknown[]>(writes);
  await feed("drop");
  expect(await read<unknown[]>(writes)).toEqual(before);
  await feed("drop", ["/Users/me/My Pics/it's.png", "/Users/me/two.txt"]);
  await browser("wait", "--fn", `${writes}.length === ${before.length + 1}`);
  expect((await read<{id:string; text:string}[]>(writes)).slice(before.length)).toEqual([
    {id:"t-migrate",text:"'/Users/me/My Pics/it'\\''s.png' '/Users/me/two.txt' "},
  ]);
  expect(await read<number>("document.querySelectorAll('.drop-over,[data-drop=hover]').length")).toBe(0);
  await read(`(() => {window.__fake.app.dropFiles({phase:'drop',paths:['/tmp/no.txt'],x:-1,y:-1});return true})()`);
  expect(await read<unknown[]>(writes)).toHaveLength(before.length + 1);
  expect(await read<unknown[]>("window.__fake.app.calls.filter(c=>c.method==='rail.move'||c.method==='todo.move')")).toEqual([]);
}, 90_000);

it("u484_scroll_and_stop", async () => {
  await load(true);
  const initial = Array.from({ length: 40 }, (_, index) => String(index + 1));
  await drag("1", "4");
  const edge = await read<{ x: number; y: number }>(`(() => { const r = document.querySelector('.sortable-fixture').getBoundingClientRect(); return { x: r.x + r.width / 2, y: r.bottom - 2 }; })()`);
  await browser("mouse", "move", String(Math.round(edge.x)), String(Math.round(edge.y)));
  await browser("wait", "--fn", `document.querySelector('.sortable-fixture').scrollTop > 160`);
  await browser("press", "Escape");
  await browser("mouse", "up");
  await browser("wait", "--fn", `document.querySelector('[data-id="1"]').dataset.dragging === 'false' && document.activeElement?.getAttribute('aria-label') === 'Move 1'`);
  expect(await order()).toEqual(initial);
  expect(await read<string>(`document.querySelector('[aria-label=Commits]').textContent`)).toBe("0");
  const stopped = () => read<number[]>(`new Promise(resolve => { const samples = []; const sample = () => { samples.push(document.querySelector('.sortable-fixture').scrollTop); if (samples.length === 10) resolve(samples); else requestAnimationFrame(sample); }; requestAnimationFrame(sample); })`);
  // The scenario explicitly observes constant scrollTop across frames after cancellation/release.
  expect(new Set(await stopped()).size).toBe(1);
  await load(true);
  await drag("1", "4");
  await browser("mouse", "move", String(Math.round(edge.x)), String(Math.round(edge.y)));
  await browser("wait", "--fn", `document.querySelector('.sortable-fixture').scrollTop > 160`);
  await browser("mouse", "move", String(Math.round(edge.x)), String(Math.round(edge.y / 2)));
  await browser("wait", "--fn", `Number(document.querySelector('.sortable-fixture').dataset.sortableIndex) > 4`);
  expect(new Set(await stopped()).size).toBe(1);
  const preview = initial.filter(id => id !== "1");
  preview.splice(await read<number>(`Number(document.querySelector('.sortable-fixture').dataset.sortableIndex)`), 0, "1");
  expect(preview.indexOf("1")).toBeGreaterThan(4);
  await browser("mouse", "up");
  await committed(preview.join(","));
  expect(new Set(await stopped()).size).toBe(1);
  await load(true);
  await keyboard("5");

  for (let index = 0; index < 8; index++) {
    const next = initial.filter(id => id !== "5");
    next.splice(4 + index, 0, "5");
    const from = next.indexOf("5");
    next.splice(from, 1); next.splice(from + 1, 0, "5");
    await down(next);
  }

  await browser("press", "Space");
  const final = initial.filter(id => id !== "5"); final.splice(12, 0, "5");
  await committed(final.join(","));
  expect(new Set(await stopped()).size).toBe(1);
}, 90_000);
