import type { AppSeam } from "../app/seam";
import { toBase64 } from "../terminal/base64";
import { createXtermEmulators } from "../terminal/emulator";
import type { EmulatorFactory } from "../terminal/emulator";
import { PATIENCE_MS, WARMUP_KEYS, createKeystrokeProbe, typeKeys } from "./keystrokes";
import type { Summary } from "./keystrokes";

const KEYS = 1000;

/** The Terminal's program prints this once its tty is raw (`crates/perf`), so no key is typed into a line-edited tty. */
const READY = "ready";

const SETUP_BOUND_MS = 10_000;

const FRAMES = 60;

const sleep = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms));

/** Resolves with what `find` returns as soon as the DOM holds it; fails at the bound. */
const appears = <T>(find: () => T | null, what: string): Promise<T> =>
  new Promise((resolve, reject) => {
    const found = find();

    if (found) return resolve(found);

    const watch = new MutationObserver(() => {
      const next = find();

      if (!next) return;

      watch.disconnect();
      resolve(next);
    });

    watch.observe(document.body, { childList: true, subtree: true, attributes: true });
    setTimeout(() => {
      watch.disconnect();
      reject(new Error(`${what} did not appear within ${SETUP_BOUND_MS} ms`));
    }, SETUP_BOUND_MS);
  });

/** Mean time between animation frames over `FRAMES`: about 8.3 or 16.7 ms when the window is drawn at full rate, far more when throttled behind other windows. */
const frameMs = (): Promise<number> =>
  new Promise((resolve) => {
    let first = 0;

    let seen = 0;

    const tick = (at: number) => {
      if (seen === 0) first = at;

      seen += 1;

      if (seen === FRAMES) resolve((at - first) / (FRAMES - 1));
      else requestAnimationFrame(tick);
    };

    requestAnimationFrame(tick);
  });

/** The smallest step `performance.now()` takes, which is 1 ms in WKWebView. */
const timerResolutionMs = (): number => {
  let smallest = Infinity;

  for (let read = 0, last = performance.now(); read < 100_000; read += 1) {
    const at = performance.now();

    if (at > last) smallest = Math.min(smallest, at - last);

    last = at;
  }

  return smallest;
};

/** The `PERF` line's body: the K2 numbers and the conditions they were taken in, or the reason there are none (`scenarios/perf.md` K3). */
type PerfReport =
  | { error: string }
  | (Summary & {
      keys: number;
      patience_ms: number;
      renderer: "webgl" | "dom";
      frame_ms: number;
      timer_resolution_ms: number;
      device_pixel_ratio: number;
      user_agent: string;
    });

const press = (textarea: HTMLTextAreaElement) => {
  textarea.dispatchEvent(new KeyboardEvent("keydown", { key: "a", code: "KeyA", keyCode: 65, bubbles: true }));
  textarea.dispatchEvent(new KeyboardEvent("keypress", { key: "a", charCode: 97, keyCode: 97, bubbles: true }));
};

/**
 * The keystroke run of `scenarios/perf.md` (K1 to K3): in a build made with `VITE_ROUNDUP_PERF`, `start` opens a Terminal, types
 * into it and writes one `PERF <json>` line through that Terminal, where `crates/perf` reads it from `terminal.output`.
 */
export const createKeystrokeRun = (app: AppSeam) => {
  const probe = createKeystrokeProbe(() => performance.now(), "a");
  let terminalId: string | null = null;
  let webgl: boolean | null = null;

  let ready: () => void = () => {};

  const isReady = new Promise<void>((resolve) => (ready = resolve));

  const emulators = createXtermEmulators({
    written: (bytes) => {
      probe.written(bytes);

      if (new TextDecoder().decode(bytes).includes(READY)) ready();
    },
    rendered: probe.rendered,
    renderer: (used) => (webgl = used),
  });

  const report = async (body: PerfReport) => {
    if (terminalId === null) throw new Error("no Terminal was opened to report through");

    const line = `PERF ${JSON.stringify({ probe: "roundup-keystroke-probe", ...body })}\n`;

    await app.rpc("terminal.write", { id: terminalId, data: toBase64(new TextEncoder().encode(line)) });
  };

  const measure = async () => {
    const spawn = await appears(
      () => [...document.querySelectorAll("button")].find((button) => button.textContent?.trim() === "+ terminal" && !button.disabled) ?? null,
      "the + terminal button",
    );

    spawn.click();
    (await appears(() => document.querySelector<HTMLElement>('[role="treeitem"]'), "the Terminal's row")).click();

    const textarea = await appears(() => document.querySelector<HTMLTextAreaElement>("textarea.xterm-helper-textarea"), "the Terminal's input");

    await Promise.race([isReady, sleep(SETUP_BOUND_MS).then(() => Promise.reject(new Error(`the Terminal never printed ${READY}`)))]);
    textarea.focus();

    const frame = await frameMs();

    await typeKeys(probe, { press: () => press(textarea), sleep, random: Math.random }, KEYS + WARMUP_KEYS);
    await report({
      ...probe.summary(WARMUP_KEYS),
      keys: KEYS,
      patience_ms: PATIENCE_MS,
      renderer: webgl ? "webgl" : "dom",
      frame_ms: frame,
      timer_resolution_ms: timerResolutionMs(),
      device_pixel_ratio: window.devicePixelRatio,
      user_agent: navigator.userAgent,
    });
  };

  return {
    createEmulator: (id: string) => {
      terminalId = id;

      return emulators(id);
    },
    start: async () => {
      try {
        await measure();
      } catch (failure) {
        if (!(failure instanceof Error)) throw failure;

        await report({ error: failure.message });
      }
    },
  };
};
