import { FitAddon } from "@xterm/addon-fit";
import { Unicode11Addon } from "@xterm/addon-unicode11";
import { WebglAddon } from "@xterm/addon-webgl";
import { Terminal } from "@xterm/xterm";
import type { ITerminalAddon, ITerminalOptions } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";

export type Size = { cols: number; rows: number };

/** One Terminal's screen. It exists before it is shown: bytes written to it wait for the first `show`. */
export type Emulator = {
  write(bytes: Uint8Array): void;
  /** Restores the Daemon's screen at its recorded size before later output is written. */
  setSize(size: Size): void;
  /** Clears the last complete screen before a replacement snapshot is written. */
  reset(): void;
  /** Keystrokes and pastes, as the bytes the program expects, in the order they were typed. */
  onInput(listener: (bytes: Uint8Array) => void): void;
  /** Moves the emulator into `host` (opening it the first time), gives it focus, and returns its fitted size. */
  show(host: HTMLElement): Size;
  /** Gives the keyboard back to the emulator. */
  focus(): void;
  /** The size that fills the emulator's current host. */
  fit(): Size;
  /** Whether the view sits at the newest line; false once the user has scrolled up. */
  isAtBottom(): boolean;
  /** Fires whenever the view's scroll position changes. */
  onScroll(listener: () => void): void;
  scrollToBottom(): void;
  dispose(): void;
};

export type RendererAddon = ITerminalAddon & Pick<WebglAddon, "onContextLoss">;

export type EmulatorFactory = (id: string) => Emulator;

/** Loads the WebGL renderer, or leaves the DOM renderer and says so once through `warn`. Returns whether WebGL is in use. */
export const attachRenderer = (
  terminal: Pick<Terminal, "loadAddon">,
  createWebgl: () => RendererAddon,
  warn: (message: string) => void,
): boolean => {
  try {
    const webgl = createWebgl();

    terminal.loadAddon(webgl);
    // Past the webview's context limit the oldest context is lost; its Terminal falls back to the DOM renderer.
    webgl.onContextLoss(() => webgl.dispose());

    return true;
  } catch (failure) {
    if (!(failure instanceof Error)) throw failure;

    warn(`WebGL renderer unavailable, using the DOM renderer: ${failure.message}`);

    return false;
  }
};

const encoder = new TextEncoder();

/** Rule 7's 150 MB budget for ten idle Agents must hold under a Terminal that prints without end. */
export const SCROLLBACK_LINES = 10_000;

export const xtermOptions: ITerminalOptions = {
  allowProposedApi: true,
  fontFamily: 'ui-monospace, "SF Mono", Menlo, monospace',
  fontSize: 13,
  scrollback: SCROLLBACK_LINES,
};

/** What the keystroke run (`scenarios/perf.md` K1 to K3) watches: each chunk the emulator has parsed, each render, and which renderer drew. A build without the run passes none. */
export type EchoProbe = {
  written(bytes: Uint8Array): void;
  rendered(): void;
  renderer(webgl: boolean): void;
};

/** The real emulators (xterm.js). WebGL is tried until the webview first refuses it, then every later Terminal goes straight to the DOM renderer. */
export const createXtermEmulators = (probe?: EchoProbe): EmulatorFactory => {
  let webglDenied = false;

  return () => {
    const terminal = new Terminal(xtermOptions);
    terminal.loadAddon(new Unicode11Addon());
    terminal.unicode.activeVersion = "11";
    const fitter = new FitAddon();
    const element = document.createElement("div");
    let opened = false;

    element.style.height = "100%";
    terminal.loadAddon(fitter);
    terminal.onRender(() => probe?.rendered());

    const size = (): Size => {
      fitter.fit();

      return { cols: terminal.cols, rows: terminal.rows };
    };

    return {
      write: (bytes) => terminal.write(bytes, probe && (() => probe.written(bytes))),
      setSize: ({ cols, rows }) => terminal.resize(cols, rows),
      reset: () => terminal.write("\x1bc"),
      onInput: (listener) => {
        terminal.onData((text) => listener(encoder.encode(text)));
        terminal.onBinary((text) => listener(Uint8Array.from(text, (char) => char.charCodeAt(0))));
      },
      show: (host) => {
        host.replaceChildren(element);

        if (!opened) {
          terminal.open(element);
          opened = true;

          if (!webglDenied) webglDenied = !attachRenderer(terminal, () => new WebglAddon(), console.warn);

          probe?.renderer(!webglDenied);
        }

        terminal.focus();

        return size();
      },
      fit: size,
      focus: () => terminal.focus(),
      isAtBottom: () => terminal.buffer.active.viewportY === terminal.buffer.active.baseY,
      onScroll: (listener) => terminal.onScroll(() => listener()),
      scrollToBottom: () => terminal.scrollToBottom(),
      dispose: () => terminal.dispose(),
    };
  };
};
