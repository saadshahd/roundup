import { fireEvent, render, screen, waitFor } from "@solidjs/testing-library";
import { expect } from "vitest";
import type { Actor } from "@contracts/Actor";
import type { Touch } from "@contracts/Touch";
import type { Pad } from "@contracts/pad/Pad";
import { RpcError } from "../app/seam";
import { DrawerHost } from "../drawer/DrawerHost";
import { createFakeApp } from "../testing/fakeApp";
import type { FakeApp } from "../testing/fakeApp";
import { EditorView } from "@codemirror/view";
import { padHandlers } from "../testing/stores";
import type { PadStore } from "../testing/stores";
import {
  connectProject,
  ConnectedProjectContext,
} from "../state/connectedProject";
import { Pads } from "./Pads";

export const AGENT: Actor = { kind: "agent", id: "agent-7f3", parent: null };

const AGENT_NAME = "auth-refactor";

let currentPads: PadStore;

export const padOf = (name: string, owner: Actor, text = ""): Pad => ({
  name,
  owner,
  text,
  updated_at: 0,
});

export const CONFLICT = -32003;

export { RpcError };

export type Deferred<T> = {
  promise: Promise<T>;
  resolve(value: T): void;
  reject(error: Error): void;
};

export const deferred = <T,>(): Deferred<T> => {
  let resolve: (value: T) => void = () => {};

  let reject: (error: Error) => void = () => {};

  const promise = new Promise<T>((done, fail) => {
    resolve = done;
    reject = fail;
  });

  return { promise, resolve, reject };
};

/** The Shelf and the Drawer over a fake Daemon whose Pads are `state.pads`, with `AGENT` off the Rail unless `onRail`; `state.history` answers `provenance.history`. */
export const openShelf = async (pads: Pad[], onRail = false) => {
  const history: Touch[] = [];
  const state: PadStore = { pads, history };
  currentPads = state;
  const app = createFakeApp();
  app.handlers["rail.tree"] = () => [
    {
      id: AGENT.id,
      // U58 lists a Pad under its Agent once that Agent is on the Rail. Off it, `AGENT` stays a named owner as a Terminal node, which `nameOf` reads and the Shelf's filter does not take for an Agent.
      kind: onRail ? "agent" : "terminal",
      name: AGENT_NAME,
      parent: null,
      order: 0,
      status: null,
      attempt: "1", status_revision: null,
      terminal_id: null,
      worktree: null,
      can_resume: false,
    },
  ];
  Object.assign(app.handlers, padHandlers(state, () => {}));

  const connected = await connectProject(
    app,
    { name: "p", path: "/p" },
    () => false,
    () => 0,
  );

  render(() => (
    <ConnectedProjectContext.Provider value={connected}>
      <Pads />
      <DrawerHost drawer={connected.drawer} reducedMotion={() => true} />
    </ConnectedProjectContext.Provider>
  ));

  return {
    app,
    state,
    connected,
    calls: () => app.calls.map((call) => call.method),
  };
};

export const enterEditor = async () => {
  fireEvent.click(screen.getByRole("button", { name: "edit" }));
  fireEvent.click(await screen.findByRole("button", { name: "source" }));
  const field = await screen.findByRole<HTMLElement>("textbox", { name: "Editor" }, { timeout: 5000 });
  const view = EditorView.findFromDOM(field);

  if (!view) throw new Error("Pad Editor has no CodeMirror view");

  Object.defineProperties(field, {
    value: {
      configurable: true,
      get: () => view.state.doc.toString(),
      set: (text: string) => view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: text } }),
    },
    readOnly: { configurable: true, get: () => false },
  });

  // SAFETY: the content element has textarea-shaped accessors installed above; input writes still dispatch real CodeMirror transactions.
  return field as HTMLTextAreaElement;
};

export const openPad = async (name: string) => {
  fireEvent.click(await screen.findByText(name));
  const reader = await screen.findByLabelText<HTMLElement>("Pad body");
  const edit = screen.queryByRole("button", { name: "edit" });

  if (edit) return enterEditor();

  Object.defineProperties(reader, {
    value: { configurable: true, get: () => currentPads.pads.find((pad) => pad.name === name)?.text ?? "" },
    readOnly: { configurable: true, get: () => true },
  });

  // SAFETY: this reader has the value and readOnly accessors installed above; no test writes through its textarea shape.
  return reader as HTMLTextAreaElement;
};

/** `pad.list` is refetched on every `pad.changed`, so a test waits for that count to pass `atLeast` rather than for a specific call index. */
export const waitForReload = (app: FakeApp, atLeast = 1) =>
  waitFor(() =>
    expect(
      app.calls.filter((call) => call.method === "pad.list").length,
    ).toBeGreaterThan(atLeast),
  );

/** A resolved `pad.write` reply reaches `write`'s `adopt` call (PadDrawer.tsx) one microtask after a test's own `await` on the reply: `rpc` (fakeApp.ts) is itself `async`, so unwrapping the handler's promise takes one more turn than the test's direct `await` on that same promise. A test awaits this, after resolving a deferred `pad.write` reply, before the next input that depends on `adopt` having run. */
export const writeAdopted = () => Promise.resolve();
