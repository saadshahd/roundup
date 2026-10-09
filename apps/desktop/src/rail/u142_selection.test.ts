import { spawn } from "node:child_process";
import type { ChildProcess } from "node:child_process";
import { existsSync, mkdtempSync, readdirSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createServer } from "vite";
import type { ViteDevServer } from "vite";
import { afterAll, beforeAll, describe, expect, it } from "vitest";

/** U142 in a real Chromium: Vite serves the U26 harness, Chrome is driven over its DevTools pipe, so no package beyond a Chrome binary is needed. */

const chromePath = (): string | undefined => {
  const fromBrowsers = (root: string | undefined) => {
    if (!root || !existsSync(root)) return undefined;

    for (const entry of readdirSync(root).filter((name) => /^chromium-\d+$/.test(name))) {
      for (const sub of ["chrome-linux/chrome", "chrome-linux64/chrome", "chrome-mac/Chromium.app/Contents/MacOS/Chromium"]) {
        if (existsSync(join(root, entry, sub))) return join(root, entry, sub);
      }
    }

    return undefined;
  };

  return [
    process.env["CHROME_PATH"],
    fromBrowsers(process.env["PLAYWRIGHT_BROWSERS_PATH"]),
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/usr/bin/google-chrome",
    "/usr/bin/chromium",
  ].find((path) => path !== undefined && existsSync(path));
};

type Json = string | number | boolean | null | readonly Json[] | { readonly [name: string]: Json };

type Params = Readonly<Record<string, Json>>;

type Reply = { id?: number; sessionId?: string; result?: unknown; error?: { message: string } };

class Browser {
  private next = 0;
  private waiting = new Map<number, (reply: Reply) => void>();
  private buffer = "";
  session = "";

  constructor(private child: ChildProcess) {
    // SAFETY: the child is spawned with a pipe at fd 4.
    (child.stdio[4] as NodeJS.ReadableStream).on("data", (chunk: Buffer) => {
      this.buffer += chunk.toString();
      let end = this.buffer.indexOf("\0");

      while (end >= 0) {
        // SAFETY: Chrome writes one JSON message per NUL on its DevTools pipe.
        const reply = JSON.parse(this.buffer.slice(0, end)) as Reply;

        this.buffer = this.buffer.slice(end + 1);

        if (reply.id !== undefined) this.waiting.get(reply.id)?.(reply);
        end = this.buffer.indexOf("\0");
      }
    });
  }

  send<T = unknown>(method: string, params: Params = {}, sessionId?: string): Promise<T> {
    const id = ++this.next;

    return new Promise((resolve, reject) => {
      // SAFETY: the caller names the result type it expects of this DevTools method.
      this.waiting.set(id, (reply) => (reply.error ? reject(new Error(`${method}: ${reply.error.message}`)) : resolve(reply.result as T)));
      // SAFETY: the child is spawned with a pipe at fd 3.
      (this.child.stdio[3] as NodeJS.WritableStream).write(`${JSON.stringify({ id, method, params, sessionId })}\0`);
    });
  }

  async open() {
    const { targetId } = await this.send<{ targetId: string }>("Target.createTarget", { url: "about:blank" });
    const { sessionId } = await this.send<{ sessionId: string }>("Target.attachToTarget", { targetId, flatten: true });

    this.session = sessionId;
  }

  page<T = unknown>(method: string, params: Params = {}) {
    return this.send<T>(method, params, this.session);
  }

  async eval<T>(expression: string): Promise<T> {
    const out = await this.page<{ result: { value: T }; exceptionDetails?: { text: string; exception?: { description: string } } }>(
      "Runtime.evaluate",
      { expression, returnByValue: true, awaitPromise: true },
    );

    if (out.exceptionDetails) throw new Error(out.exceptionDetails.exception?.description ?? out.exceptionDetails.text);

    return out.result.value;
  }

  /** Runs a constant function declaration in the page with `args` passed as data, so no value is ever spliced into source. */
  async call<T>(declaration: string, ...args: Json[]): Promise<T> {
    const { result: page } = await this.page<{ result: { objectId: string } }>("Runtime.evaluate", { expression: "globalThis" });

    const out = await this.page<{ result: { value: T }; exceptionDetails?: { text: string; exception?: { description: string } } }>("Runtime.callFunctionOn", {
      objectId: page.objectId,
      functionDeclaration: declaration,
      arguments: args.map((value) => ({ value })),
      returnByValue: true,
      awaitPromise: true,
    });

    if (out.exceptionDetails) throw new Error(out.exceptionDetails.exception?.description ?? out.exceptionDetails.text);

    return out.result.value;
  }

  mouse(type: "mousePressed" | "mouseMoved" | "mouseReleased", x: number, y: number, extra: Params = {}) {
    return this.page("Input.dispatchMouseEvent", { type, x, y, button: type === "mouseMoved" && !("buttons" in extra) ? "none" : "left", ...extra });
  }

  async drag(from: Point, to: Point) {
    await this.mouse("mouseMoved", from.x, from.y);
    await this.mouse("mousePressed", from.x, from.y, { buttons: 1, clickCount: 1 });

    for (let step = 1; step <= 8; step++) {
      await this.mouse("mouseMoved", from.x + ((to.x - from.x) * step) / 8, from.y + ((to.y - from.y) * step) / 8, { buttons: 1 });
    }

    await this.mouse("mouseReleased", to.x, to.y, { buttons: 0, clickCount: 1 });
  }

  async click(at: Point, clickCount = 1) {
    await this.mouse("mouseMoved", at.x, at.y);

    for (let count = 1; count <= clickCount; count++) {
      await this.mouse("mousePressed", at.x, at.y, { buttons: 1, clickCount: count });
      await this.mouse("mouseReleased", at.x, at.y, { buttons: 0, clickCount: count });
    }
  }

  key(key: string, extra: Params = {}) {
    return this.page("Input.dispatchKeyEvent", { type: "keyDown", key, ...extra });
  }
}

type Point = { x: number; y: number };

const SIZES = [
  { width: 1280, height: 800 },
  { width: 700, height: 800 },
] as const;

let server: ViteDevServer;

let child: ChildProcess;

let profile: string;

let browser: Browser;

let origin = "";

const found = chromePath();

/** Where a selector's n-th match is, as a point `dx` pixels from its left edge or `fx` of its width, and the vertical middle. */
const pointOf = (selector: string, at: { dx?: number; fx?: number; nth?: number } = {}) =>
  browser.call<Point>(
    `function (selector, dx, fx, nth) {
      const element = [...document.querySelectorAll(selector)][nth];
      if (!element) throw new Error("no " + selector);
      element.scrollIntoView({ block: "nearest" });
      const box = element.getBoundingClientRect();
      return { x: box.left + dx + box.width * fx, y: box.top + box.height / 2 };
    }`,
    selector,
    at.dx ?? 0,
    at.fx ?? 0,
    at.nth ?? 0,
  );

const selected = () => browser.eval<string>("window.getSelection().toString()");

/** Resolves when the page function (a constant declaration taking `args`) returns truthy: the App's own state is the bound, not a sleep. */
const until = async (predicate: string, ...args: Json[]) => {
  const stop = Date.now() + 15_000;

  while (!(await browser.call<boolean>(predicate, ...args))) {
    if (Date.now() > stop) throw new Error(`never became true: ${predicate}`);

    await new Promise((resolve) => setTimeout(resolve, 20));
  }
};

const load = async (size: { width: number; height: number }) => {
  await browser.page("Emulation.setDeviceMetricsOverride", { ...size, deviceScaleFactor: 1, mobile: false });
  await browser.page("Page.navigate", { url: `${origin}/harness.html?seed=tree-40` });
  // The harness asks for its pads last, once the Rail's tree and Terminals are in.
  await until(`() => Boolean(window.__fake && window.__fake.app.calls.some((call) => call.method === "pad.list") && document.querySelector(".rail-fold button"))`);
  await browser.eval("window.getSelection().removeAllRanges()");
};

const calls = (method: string) =>
  browser.call<unknown[]>(`(method) => window.__fake.app.calls.filter((call) => call.method === method).map((call) => call.params)`, method);

beforeAll(async () => {
  if (!found) return;

  server = await createServer({
    root: process.cwd(),
    server: { port: 0, strictPort: false, hmr: false },
    logLevel: "silent",
  });
  await server.listen();
  origin = (server.resolvedUrls?.local[0] ?? "").replace(/\/$/, "");
  profile = mkdtempSync(join(tmpdir(), "u142-"));
  child = spawn(
    found,
    ["--headless=new", "--no-sandbox", "--disable-gpu", "--remote-debugging-pipe", `--user-data-dir=${profile}`, "--no-first-run", "about:blank"],
    { stdio: ["ignore", "ignore", "ignore", "pipe", "pipe"] },
  );
  browser = new Browser(child);
  await browser.open();
  await browser.page("Page.enable");
  await browser.page("Runtime.enable");
  // Warm the module graph so no later wait covers Vite's first transform.
  await load(SIZES[0]);
}, 90_000);

afterAll(async () => {
  if (child) {
    const exited = new Promise((resolve) => child.once("exit", resolve));
    // Browser.close lets Chrome stop its helper processes, which still write the profile after a SIGKILL of the main one.
    const closed = browser.send("Browser.close").then(() => "closed");
    const late = new Promise((resolve) => setTimeout(resolve, 10_000, "late"));

    if ((await Promise.race([closed, late])) === "late") child.kill("SIGKILL");

    await exited;
  }

  await server?.close();

  if (profile) rmSync(profile, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 });
});

const run = found ? describe : describe.skip;

const nameAt = (nth: number) => pointOf(".rail-row .name", { dx: 3, nth });

const editName = async (nth: number) => {
  await browser.click(await nameAt(nth), 2);
  await until(`() => Boolean(document.querySelector(".rail-row input"))`);
};

const selectAllInField = () => browser.page("Input.dispatchKeyEvent", { type: "keyDown", key: "a", code: "KeyA", modifiers: 2, commands: ["selectAll"] });

run("u142 the Rail selects rows, not browser text", () => {
  it("u142_the_drag_helper_selects_text_where_the_browser_allows_it", async () => {
    await load(SIZES[0]);
    await browser.eval(`document.body.insertAdjacentHTML("afterbegin", '<p id="ctl" style="position:fixed;top:0;left:400px;margin:0;z-index:99;background:white;color:black">selectable control text</p>')`);
    await browser.drag(await pointOf("#ctl", { dx: 2 }), await pointOf("#ctl", { fx: 1, dx: -2 }));
    expect(await selected()).not.toBe("");
  }, 30_000);

  for (const size of SIZES) {
    const at = `${size.width}x${size.height}`;

    /** Each attempt: where the press lands and where it is released. */
    const attempts: [string, () => Promise<[Point, Point]>][] = [
      ["within a name, left to right", async () => [await pointOf(".rail-row .name", { dx: 1 }), await pointOf(".rail-row .name", { fx: 1, dx: -1 })]],
      ["within a name, right to left", async () => [await pointOf(".rail-row .name", { fx: 1, dx: -1 }), await pointOf(".rail-row .name", { dx: 1 })]],
      ["from the gap right of a name across the rows below", async () => [await pointOf(".rail-row", { fx: 1, dx: -3 }), await pointOf(".rail-row", { dx: 2, nth: 3 })]],
      ["from the left gap upward across rows", async () => [await pointOf(".rail-row", { dx: 1, nth: 3 }), await pointOf(".rail-row", { fx: 1, dx: -3, nth: 0 })]],
      ["across a Group label", async () => [await pointOf(".rail-row[aria-expanded] .name", { dx: 1 }), await pointOf(".rail-row[aria-expanded] .name", { fx: 1 })]],
      ["through a Terminal and an Agent across rows", async () => [await pointOf(".rail-row .name", { dx: 1, nth: 2 }), await pointOf(".rail-row .name", { fx: 1, nth: 5 })]],
      ["from the left gap of the done summary across its label", async () => [await pointOf(".rail-fold", { dx: 1 }), await pointOf(".rail-fold", { fx: 1, dx: -2 })]],
      ["from the right gap of the done summary leftwards", async () => [await pointOf(".rail-fold", { fx: 1, dx: -2 }), await pointOf(".rail-fold button", { dx: 1 })]],
      ["from the gap between the first two add controls across the rest", async () => {
        const first = await pointOf(".rail-actions button", { fx: 1 });
        const second = await pointOf(".rail-actions button", { nth: 1 });
        const last = await pointOf(".rail-actions button", { nth: 2, fx: 1, dx: -1 });

        return [{ x: (first.x + second.x) / 2, y: first.y }, last];
      }],
      ["from the gap between the first two add controls back across the first", async () => {
        const first = await pointOf(".rail-actions button", { fx: 1 });
        const second = await pointOf(".rail-actions button", { nth: 1 });
        const start = await pointOf(".rail-actions button", { dx: 1 });

        return [{ x: (first.x + second.x) / 2, y: first.y }, start];
      }],
      ["from the gap between the first two add controls up through the done summary", async () => {
        const first = await pointOf(".rail-actions button", { fx: 1 });
        const second = await pointOf(".rail-actions button", { nth: 1 });

        return [{ x: (first.x + second.x) / 2, y: first.y }, await pointOf(".rail-fold", { fx: 0.5 })];
      }],
      ["from the pinned bar's empty end back across the controls", async () => [await pointOf(".rail-actions", { fx: 1, dx: -2 }), await pointOf(".rail-actions button", { dx: 1 })]],
    ];

    for (const [name, aim] of attempts) {
      it(`u142_${at}_a_drag_${name.replaceAll(/[^a-z0-9]+/gi, "_")}_leaves_no_selection`, async () => {
        await load(size);
        const [from, to] = await aim();

        await browser.drag(from, to);
        expect(await selected()).toBe("");
      }, 30_000);
    }

    it(`u142_${at}_the_add_controls_and_done_summary_keep_their_accessible_names`, async () => {
      await load(size);
      const names = await browser.eval<string[]>(`[...document.querySelectorAll(".rail-actions button, .rail-fold button")].map((button) => button.textContent.trim())`);

      expect(names.filter((label) => /^\+?\s*(agent|terminal|room)$/.test(label))).toHaveLength(3);
      expect(names.some((label) => label.endsWith("done"))).toBe(true);
    }, 30_000);

    it(`u142_${at}_clicking_a_row_selects_it`, async () => {
      await load(size);
      await browser.click(await nameAt(2));
      await until(`() => document.querySelector('.rail-row[aria-selected="true"]')?.dataset.id === document.querySelectorAll(".rail-row")[2].dataset.id`);
    }, 30_000);

    it(`u142_${at}_clicking_the_done_summary_folds_it`, async () => {
      await load(size);
      const rows = await browser.eval<number>(`document.querySelectorAll(".rail-row").length`);

      await browser.click(await pointOf(".rail-fold button", { dx: 3 }));
      await until(`(rows) => document.querySelectorAll(".rail-row").length !== rows`, rows);
    }, 30_000);

    it(`u142_${at}_clicking_add_terminal_spawns_one`, async () => {
      await load(size);
      await browser.click(await pointOf(".rail-actions button", { nth: 1, dx: 3 }));
      await until(`() => window.__fake.app.calls.filter((call) => call.method === "rail.spawnTerminal").length === 1`);
    }, 30_000);

    it(`u142_${at}_a_row_drop_still_changes_home`, async () => {
      await load(size);
      // agent-4 sits inside "payments"; released below that Group's last child at the left edge it lands at the top level.
      const target = await pointOf('[data-id="agent-2"]', { dx: 3 });

      await browser.drag(await pointOf('[data-id="agent-4"] .name', { dx: 3 }), target);
      await until(`() => window.__fake.app.calls.some((call) => call.method === "rail.move")`);
      expect(await calls("rail.move")).toEqual([{ id: "agent-4", parent: null, index: expect.any(Number) }]);
      expect(await selected()).toBe("");
    }, 30_000);

    it(`u142_${at}_dragging_in_the_rename_input_selects_its_text_and_moves_no_row`, async () => {
      await load(size);
      await editName(2);
      // The field opens with its whole name selected, and a press inside a selection starts a text drag on macOS; Home puts the caret at the start first.
      await browser.key("Home", { code: "Home", windowsVirtualKeyCode: 36 });
      expect(await browser.eval<boolean>(`document.querySelector(".rail-row input").selectionEnd === document.querySelector(".rail-row input").selectionStart`)).toBe(true);
      await browser.drag(await pointOf(".rail-row input", { dx: 2 }), await pointOf(".rail-row input", { dx: 30 }));
      expect(await browser.eval<boolean>(`document.querySelector(".rail-row input").selectionEnd > document.querySelector(".rail-row input").selectionStart`)).toBe(true);
      expect(await calls("rail.move")).toEqual([]);
    }, 30_000);

    it(`u142_${at}_the_rename_input_the_error_line_and_the_menu_stay_selectable`, async () => {
      await load(size);
      await editName(2);
      await browser.eval(`document.querySelector(".rail-tree").insertAdjacentHTML("beforeend", '<p role="alert" id="u142-alert">failed</p><div class="rail-menu" id="u142-menu">menu</div>')`);
      const modes = await browser.eval<string[]>(`[".rail-row input", "#u142-alert", "#u142-menu"].map((selector) => getComputedStyle(document.querySelector(selector)).userSelect)`);

      expect(modes).toEqual(["text", "text", "text"]);
    }, 30_000);

    it(`u142_${at}_typing_in_the_rename_input_replaces_the_selected_name`, async () => {
      await load(size);
      await editName(2);
      const original = await browser.eval<string>(`document.querySelector(".rail-row input").value`);

      await selectAllInField();
      expect(await browser.eval<string>(`(() => { const field = document.querySelector(".rail-row input"); return field.value.slice(field.selectionStart, field.selectionEnd); })()`)).toBe(original);
      await browser.page("Input.insertText", { text: "renamed" });
      expect(await browser.eval<string>(`document.querySelector(".rail-row input").value`)).toBe("renamed");
    }, 30_000);

    it(`u142_${at}_enter_in_the_rename_input_commits_the_name_once`, async () => {
      await load(size);
      await editName(2);
      await selectAllInField();
      await browser.page("Input.insertText", { text: "renamed" });
      await browser.key("Enter", { code: "Enter", windowsVirtualKeyCode: 13, text: "\r" });
      await until(`() => !document.querySelector(".rail-row input")`);
      expect(await calls("rail.rename")).toEqual([expect.objectContaining({ name: "renamed" })]);
    }, 30_000);

    it(`u142_${at}_escape_in_the_rename_input_keeps_the_old_name`, async () => {
      await load(size);
      const name = await browser.eval<string>(`document.querySelectorAll(".rail-row .name")[2].textContent`);

      await editName(2);
      await browser.page("Input.insertText", { text: "x" });
      await browser.key("Escape", { code: "Escape", windowsVirtualKeyCode: 27 });
      await until(`() => !document.querySelector(".rail-row input")`);
      expect(await calls("rail.rename")).toEqual([]);
      expect(await browser.eval<string>(`document.querySelectorAll(".rail-row .name")[2].textContent`)).toBe(name);
    }, 30_000);
  }
});
