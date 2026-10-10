import { render } from "@solidjs/testing-library";
import type { Event as DaemonEvent } from "@contracts/Event";
import type { RailNode } from "@contracts/agent/RailNode";
import type { Message } from "@contracts/message/Message";
import { Jump } from "../rail/Jump";
import { Rail } from "../rail/Rail";
import { connectProject, ConnectedProjectContext } from "../state/connectedProject";
import { fakeEmulators } from "../terminal/paneHarness";
import { Pane } from "../terminal/Pane";
import { createFakeApp } from "../testing/fakeApp";
import { event, info, NOW, USER } from "../testing/nodes";
import { Pads } from "../pads/Pads";
import { Todos } from "../todos/Todos";
import { trackSelections } from "./emulators";
import { Thread } from "./Thread";

export const message = (id: number, over: Partial<Message> = {}): Message => ({
  id,
  from: { kind: "agent", id: "a", parent: null },
  to: "b",
  kind: "note",
  body: `body ${id}`,
  replyTo: null,
  status: "delivered",
  reason: null,
  passedFrom: null,
  at: NOW,
  ...over,
});

export const sent = (data: Message): DaemonEvent => event({ name: "message.sent", data });

type Options = { messages?: Message[]; reducedMotion?: boolean; failure?: Error; select?: string | null };

/** The Rail, the Pane, the Thread and the Shelf in one `.window`, as the App lays them out, over a fake Daemon. */
export const mountThread = async (tree: RailNode[], options: Options = {}) => {
  const app = createFakeApp();

  app.handlers["rail.tree"] = () => tree;
  app.handlers["terminal.list"] = () => tree.flatMap((node) => (node.terminal_id ? [info(node.terminal_id)] : []));
  app.handlers["terminal.write"] = () => null;
  app.handlers["terminal.resize"] = () => null;
  app.handlers["terminal.snapshot"] = () => ({ cols: 100, rows: 30, after: 0, data: "" });
  app.handlers["message.list"] = () => options.messages ?? [];
  app.handlers["message.send"] = (params) => message(99, { from: USER, to: params.to, body: params.body });
  app.handlers["takeover.begin"] = () => null;
  app.handlers["takeover.end"] = () => null;
  app.handlers["todo.list"] = () => [{ id: 1, title: "todo 1", body: "", done: false, blockers: [], blocked: false, created_at: 0, creator: USER, home: null }];
  app.handlers["pad.list"] = () => [{ name: "notes", owner: USER, text: "", updated_at: 0 }];

  const reduced = options.reducedMotion ?? false;
  const connected = await connectProject(app, { name: "p", path: "/p" }, () => reduced, () => NOW);
  const { factory, made } = fakeEmulators();
  const selections = trackSelections(factory);

  const view = render(() => (
    <ConnectedProjectContext.Provider value={connected}>
      <Jump onFailure={() => {}} />
      <div class="window">
        <section class="rail"><Rail /></section>
        <section class="centre">
          <div class="centre-stack">
            <Pane createEmulator={selections.create} />
            <Thread selectionOf={selections.selectionOf} />
          </div>
        </section>
        <section class="shelf"><Todos /><Pads /></section>
      </div>
    </ConnectedProjectContext.Provider>
  ));

  await connected.rail.settled();

  // U145: the Thread shows only under a Workstream, so these tests open the first one unless `select` says otherwise.
  const opened = options.select === undefined ? tree.find((node) => node.kind === "workstream" || node.parent === null)?.id : options.select;

  if (opened !== undefined && opened !== null) connected.rail.select(opened);

  return { app, connected, emulators: made, ...view };
};

/** The product of `opacity` along `element` and its ancestors: the effective opacity a person sees. */
export const effectiveOpacity = (element: Element): number => {
  let product = 1;

  for (let at: Element | null = element; at !== null; at = at.parentElement) {
    const own = getComputedStyle(at).opacity;

    product *= own === "" ? 1 : Number(own);
  }

  return product;
};

export const threadInput = (): HTMLInputElement => {
  const found = document.querySelector<HTMLInputElement>("[data-thread-input]");

  if (!found) throw new Error("no Thread input");

  return found;
};
