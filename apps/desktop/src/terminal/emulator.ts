import { FitAddon } from "@xterm/addon-fit";
import { WebglAddon } from "@xterm/addon-webgl";
import { Terminal } from "@xterm/xterm";
import type { ITerminalAddon } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";

export type Size = { cols: number; rows: number };

/** One Terminal's screen. It exists before it is shown: bytes written to it wait for the first `show`. */
export type Emulator = {
  write(bytes: Uint8Array): void;
  /** Keystrokes and pastes, as the bytes the program expects, in the order they were typed. */
  onInput(listener: (bytes: Uint8Array) => void): void;
  /** Moves the emulator into `host` (opening it the first time), gives it focus, and returns its fitted size. */
  show(host: HTMLElement): Size;
  /** Gives the keyboard back to the emulator. */
  focus(): void;
  /** The size that fills the emulator's current host. */
  fit(): Size;
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

/** The real emulators (xterm.js). WebGL is tried until the webview first refuses it, then every later Terminal goes straight to the DOM renderer. */
export const createXtermEmulators = (): EmulatorFactory => {
  let webglDenied = false;

  return () => {
    const terminal = new Terminal({ fontFamily: 'ui-monospace, "SF Mono", Menlo, monospace', fontSize: 13 });
    const fitter = new FitAddon();
    const element = document.createElement("div");
    let opened = false;

    element.style.height = "100%";
    terminal.loadAddon(fitter);

    const size = (): Size => {
      fitter.fit();

      return { cols: terminal.cols, rows: terminal.rows };
    };

    return {
      write: (bytes) => terminal.write(bytes),
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
        }

        terminal.focus();

        return size();
      },
      fit: size,
      focus: () => terminal.focus(),
      dispose: () => terminal.dispose(),
    };
  };
};
