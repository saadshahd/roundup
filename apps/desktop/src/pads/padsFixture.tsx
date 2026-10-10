import { fireEvent, render, screen, waitFor } from "@solidjs/testing-library";
import { expect } from "vitest";
import type { Actor } from "@contracts/Actor";
import type { Touch } from "@contracts/Touch";
import type { Pad } from "@contracts/pad/Pad";
import { RpcError } from "../app/seam";
import { DrawerHost } from "../drawer/DrawerHost";
import { createFakeApp } from "../testing/fakeApp";
import type { FakeApp } from "../testing/fakeApp";
import { Editor } from "@milkdown/kit/core";
import { getMarkdown, replaceAll } from "@milkdown/kit/utils";
import { padHandlers } from "../testing/stores";
import type { PadStore } from "../testing/stores";
import {
  connectProject,
  ConnectedProjectContext,
} from "../state/connectedProject";
import { Pads } from "./Pads";

// jsdom has no layout; ProseMirror asks a Range for its rectangles when a selection moves.
Range.prototype.getClientRects ??= () => document.body.getClientRects();

Range.prototype.getBoundingClientRect ??= () => new DOMRect();

export const AGENT: Actor = { kind: "agent", id: "agent-7f3", parent: null };

const AGENT_NAME = "auth-refactor";

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
      channel: null,
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

const editors: Editor[] = [];

let tracking = false;

/** The Pad's surface is a ProseMirror document, so a test types by replacing it through the Milkdown editor the drawer created. */
const trackEditors = () => {
  if (tracking) return;

  tracking = true;
  const make = Editor.make.bind(Editor);

  Editor.make = () => {
    const editor = make();

    editors.push(editor);

    return editor;
  };
};

/** Opens a Pad's Drawer and returns its scroll root with textarea-shaped `value` accessors over the document, so a test can type `fireEvent.input(field, { target: { value } })` and read what the Pad holds. */
export const openPad = async (name: string) => {
  trackEditors();
  fireEvent.click(await screen.findByText(name));
  const surface = await screen.findByRole<HTMLElement>("textbox", { name: "Pad body" }, { timeout: 5000 });
  const root = surface.closest<HTMLElement>(".pad-editor");
  const editor = editors.at(-1);

  if (!root || !editor) throw new Error("Pad surface did not open");

  Object.defineProperties(root, {
    value: {
      configurable: true,
      get: () => editor.action(getMarkdown()).replace(/\n$/, ""),
      set: (text: string) => editor.action(replaceAll(text)),
    },
    readOnly: { configurable: true, get: () => surface.getAttribute("contenteditable") !== "true" },
  });

  // SAFETY: the root has the value and readOnly accessors installed above; input writes still dispatch real ProseMirror transactions.
  return root as HTMLTextAreaElement;
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
