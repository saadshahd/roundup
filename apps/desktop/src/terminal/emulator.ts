import { DEFAULT_FONT_SIZE } from "./fontSize";
import { FitAddon } from "@xterm/addon-fit";
import { Unicode11Addon } from "@xterm/addon-unicode11";
import { WebglAddon } from "@xterm/addon-webgl";
import { Terminal } from "@xterm/xterm";
import type { IMarker, ITerminalAddon, ITerminalOptions } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";

export type Size = { cols: number; rows: number };

/** One Terminal's screen. It exists before it is shown: bytes written to it wait for the first `show`. */
export type Emulator = {
  write(bytes: Uint8Array, parsed?: () => void): void;
  /** Restores the Daemon's screen at its recorded size before later output is written. */
  setSize(size: Size): void;
  setFontSize(size: number): void;
  /** Clears the last complete screen before a replacement snapshot is written. */
  reset(): void;
  /** Keystrokes and pastes, as the bytes the program expects, in the order they were typed. */
  onInput(listener: (bytes: Uint8Array) => void): void;
  selection(): string;
  paste(text: string): void;
  /** Moves the emulator into `host` (opening it the first time), returns its fitted size, and focuses it unless restoring the Rail. */
  show(host: HTMLElement, focus?: boolean): Size;
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
  fontSize: DEFAULT_FONT_SIZE,
  scrollback: SCROLLBACK_LINES,
};

/** What the keystroke run (`scenarios/perf.md` K1 to K3) watches: each chunk the emulator has parsed, each render, and which renderer drew. A build without the run passes none. */
export type EchoProbe = {
  written(bytes: Uint8Array): void;
  rendered(): void;
  renderer(webgl: boolean): void;
};

/** The real emulators (xterm.js). WebGL is tried until the webview first refuses it, then every later Terminal goes straight to the DOM renderer. */
export const createXtermEmulators = (
  probe?: EchoProbe,
  createWebgl: () => RendererAddon = () => new WebglAddon(),
): EmulatorFactory => {
  let webglDenied = false;

  return () => {
    const terminal = new Terminal(xtermOptions);
    terminal.loadAddon(new Unicode11Addon());
    terminal.unicode.activeVersion = "11";
    const fitter = new FitAddon();
    const element = document.createElement("div");
    let opened = false;
    let fontSizeChanged = false;
    let fontViewport: IMarker | null = null;
    let fontFrame: number | null = null;
    let loadedWebgl: RendererAddon | null = null;

    element.style.height = "100%";
    terminal.loadAddon(fitter);
    terminal.onRender(() => probe?.rendered());

    const restoreFontViewport = () => {
      fontFrame = null;

      if (fontViewport === null || fontSizeChanged) return;

      if (!fontViewport.isDisposed) {
        // xterm's pixel offset can disagree with viewportY after a font change; reset that origin first.
        terminal.scrollLines(-terminal.buffer.active.length);
        terminal.scrollToLine(fontViewport.line);
      }

      fontViewport.dispose();
      fontViewport = null;
    };

    const size = (): Size => {
      const reflowCursorLine = terminal.options.reflowCursorLine ?? false;

      // Font resizing must retain unfinished output; ordinary shell resizing keeps xterm's policy.
      if (fontSizeChanged) terminal.options.reflowCursorLine = true;

      try {
        fitter.fit();
      } finally {
        terminal.options.reflowCursorLine = reflowCursorLine;
        fontSizeChanged = false;
      }

      if (fontViewport !== null) {
        // xterm syncs viewport pixels in its render frame; scrolling before it uses the old cell height.
        if (opened) fontFrame ??= requestAnimationFrame(restoreFontViewport);
        else restoreFontViewport();
      }

      return { cols: terminal.cols, rows: terminal.rows };
    };

    return {
      write: (bytes, parsed) => terminal.write(bytes, () => {
        probe?.written(bytes);
        parsed?.();
      }),
      setSize: ({ cols, rows }) => {
        if (opened && loadedWebgl && (terminal.cols !== cols || terminal.rows !== rows)) {
          loadedWebgl.dispose();
          loadedWebgl = null;
        }

        terminal.resize(cols, rows);
      },
      setFontSize: (size) => {
        if (terminal.options.fontSize === size) return;

        // Markers follow xterm's rewrap; a saved physical row would jump to different content.
        if (terminal.buffer.active.viewportY !== terminal.buffer.active.baseY) {
          const buffer = terminal.buffer.active;

          fontViewport ??= terminal.registerMarker(buffer.viewportY - buffer.baseY - buffer.cursorY) ?? null;
        }

        terminal.options.fontSize = size;
        fontSizeChanged = true;
      },
      reset: () => terminal.write("\x1bc"),
      onInput: (listener) => {
        terminal.onData((text) => listener(encoder.encode(text)));
        terminal.onBinary((text) => listener(Uint8Array.from(text, (char) => char.charCodeAt(0))));
      },
      selection: () => terminal.getSelection(),
      paste: (text) => terminal.paste(text),
      show: (host, focus = true) => {
        host.replaceChildren(element);

        if (!opened) {
          terminal.open(element);
          opened = true;

          if (!webglDenied) webglDenied = !attachRenderer(terminal, () => {
            loadedWebgl = createWebgl();

            return loadedWebgl;
          }, console.warn);

          probe?.renderer(!webglDenied);
        }

        if (focus) terminal.focus();

        return size();
      },
      fit: size,
      focus: () => terminal.focus(),
      isAtBottom: () => terminal.buffer.active.viewportY === terminal.buffer.active.baseY,
      onScroll: (listener) => terminal.onScroll(() => listener()),
      scrollToBottom: () => {
        fontViewport?.dispose();
        fontViewport = null;
        terminal.scrollToBottom();
      },
      dispose: () => {
        if (fontFrame !== null) cancelAnimationFrame(fontFrame);

        fontViewport?.dispose();
        terminal.dispose();
      },
    };
  };
};
