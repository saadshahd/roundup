import { cleanup, render } from "@solidjs/testing-library";
import { afterEach, expect, it, vi } from "vitest";
import { info, terminal } from "../testing/nodes";
import { ConnectedProjectContext } from "../state/connectedProject";
import { createXtermEmulators } from "./emulator";
import { Pane } from "./Pane";
import { callsTo, connectFakeProject } from "./paneHarness";

afterEach(() => { cleanup(); vi.unstubAllGlobals(); });

const PATH = "/Users/me/My Pics/it's.png";

const QUOTED = "'/Users/me/My Pics/it'\\''s.png' ";

const dropOnRealEmulator = async (bracketed: boolean) => {
  vi.stubGlobal("matchMedia", () => ({ matches: false, addListener() {}, removeListener() {}, addEventListener() {}, removeEventListener() {} }));
  const { app, connected } = await connectFakeProject([terminal("a")], [info("t-a")]);
  const emulators = new Map<string, ReturnType<ReturnType<typeof createXtermEmulators>>>();

  const real = createXtermEmulators(undefined, () => ({
    activate() {}, dispose() {}, onContextLoss() { return { dispose() {} }; },
  }));

  const { container } = render(() => (
    <ConnectedProjectContext.Provider value={connected}>
      <Pane createEmulator={(id) => {
        const made = real(id);

        emulators.set(id, made);

        return made;
      }} />
    </ConnectedProjectContext.Provider>
  ));

  connected.rail.select("a");
  const body = container.querySelector<HTMLElement>(".pane-body")!;

  body.getBoundingClientRect = () => new DOMRect(0, 0, 100, 100);
  await vi.waitFor(() => expect(emulators.get("t-a")).toBeDefined());

  const input: string[] = [];

  emulators.get("t-a")!.onInput((bytes) => input.push(new TextDecoder().decode(bytes)));

  if (bracketed) await new Promise<void>((resolve) => emulators.get("t-a")!.write(new TextEncoder().encode("\x1b[?2004h"), resolve));

  app.dropFiles({ phase: "drop", paths: [PATH], x: 50, y: 50 });
  await vi.waitFor(() => expect(callsTo(app, "terminal.write").length).toBeGreaterThan(0));
  await new Promise<void>((resolve) => emulators.get("t-a")!.write(new Uint8Array(), resolve));
  await new Promise((resolve) => setTimeout(resolve, 50));

  return { input: input.join(""), writes: callsTo(app, "terminal.write") };
};

it("u175_a_dropped_image_writes_one_bracketed_paste", async () => {
  const { input, writes } = await dropOnRealEmulator(true);

  expect(input).toBe(`\x1b[200~${QUOTED}\x1b[201~`);
  expect(writes).toEqual([{ id: "t-a", data: btoa(`\x1b[200~${QUOTED}\x1b[201~`) }]);
});

it("u175_without_bracketed_paste_the_path_has_no_marker", async () => {
  const { input, writes } = await dropOnRealEmulator(false);

  expect(input).toBe(QUOTED);
  expect(writes).toEqual([{ id: "t-a", data: btoa(QUOTED) }]);
});
