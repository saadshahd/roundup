import { leaderKey } from "../keys/leaderFixture";
import { render } from "@solidjs/testing-library";
import { createSignal, Show } from "solid-js";
import type { RailNode } from "@contracts/agent/RailNode";
import type { TerminalInfo } from "@contracts/terminal/TerminalInfo";
import { Bubbles } from "../bubbles/Bubbles";
import { AskBar } from "../ask/AskBar";
import { AttentionKey } from "../rail/AttentionKey";
import { Keys } from "../keys/Keys";
import { Rail } from "../rail/Rail";
import { ConnectedProjectContext, connectProject } from "../state/connectedProject";
import { createFakeApp } from "../testing/fakeApp";
import { agent, door, info, workstream } from "../testing/nodes";
import { Pane } from "../terminal/Pane";
import { fakeEmulators } from "../terminal/paneHarness";
import type { Screens } from "../terminal/screens";
import { Switcher } from "./Switcher";

const TREE: RailNode[] = [
  door("lead", "idle", "idle", { name: "lead" }),
  agent("ann", "working", "busy", { parent: "lead", name: "Ann" }),
  agent("bob", "needs-you", "asks", { parent: "lead", order: 1, name: "Bob" }),
  agent("cy", "error", "failed", { parent: "lead", order: 2, name: "Cy" }),
  workstream("ops", { order: 1, name: "ops" }),
  agent("dee", "idle", "idle", { parent: "ops", name: "Dee" }),
];

const TERMINALS: TerminalInfo[] = [info("7", { owner: "ops", title: "build shell" })];

/** The Rail, `Keys`, a Pane, its bubbles and the Switcher in one open Project; `failures` collects what the Switcher reports. */
export const mountSwitcher = async (selected: string | null = null, tree: RailNode[] = TREE, terminals: TerminalInfo[] = TERMINALS, daemonExit: () => { code: number | null } | null = () => null) => {
  const app = createFakeApp();
  let listed = [...terminals, ...tree.flatMap((node) => node.terminal_id ? [info(node.terminal_id)] : [])];

  app.handlers["rail.tree"] = () => tree;
  app.handlers["terminal.list"] = () => listed;
  app.handlers["terminal.close"] = ({ id }) => {
    listed = listed.filter((terminal) => terminal.id !== id);

    return null;
  };

  app.handlers["terminal.write"] = () => null;
  app.handlers["terminal.resize"] = () => null;
  app.handlers["terminal.snapshot"] = () => ({ cols: 100, rows: 30, after: 0, data: "" });

  const connected = await connectProject(app, { name: "p", path: "/p" }, () => false, () => 0, daemonExit);
  const emulators = fakeEmulators();
  const failures: (string | null)[] = [];

  if (selected !== null) connected.rail.select(selected);

  render(() => {
    const [screens, setScreens] = createSignal<Screens>();

    return (
      <ConnectedProjectContext.Provider value={connected}>
        <Keys />
        <AttentionKey />
        <AskBar />
        <Rail />
        <div class="centre-stack">
          <Pane createEmulator={emulators.factory} screensRef={setScreens} />
          <Show when={screens()}>{(shared) => <Bubbles screens={shared()} />}</Show>
          <div class="resource-strip" />
          <Switcher onFailure={(message) => failures.push(message)} onOpen={() => {}} />
        </div>
      </ConnectedProjectContext.Provider>
    );
  });

  await connected.rail.settled();

  return { app, connected, emulators, failures };
};

export const switcherKey = (key: string, init: KeyboardEventInit = {}, target: EventTarget = document.activeElement ?? document.body): KeyboardEvent => {
  const event = new KeyboardEvent("keydown", { key, code: key.length === 1 ? `Key${key.toUpperCase()}` : key, bubbles: true, cancelable: true, ...init });

  target.dispatchEvent(event);

  return event;
};

/** Open the switcher as the leader does: `⌘K` then `K` (U2239). */
export const openSwitcher = (): void => void leaderKey("KeyK");

export const field = (): HTMLInputElement | null => document.querySelector<HTMLInputElement>(".switcher input");

export const rows = (): string[] => [...document.querySelectorAll('.switcher .app-combobox-item:not([data-value^="action:"])')].map((row) => [...row.querySelectorAll("span:not(.glyph, .switcher-mark):not(.glyph *)")].map((part) => part.textContent).join(" "));
