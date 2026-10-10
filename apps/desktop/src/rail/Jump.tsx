import { createSignal, onCleanup } from "solid-js";
import { useConnectedProject } from "../state/connectedProject";

/** `⌥1` to `⌥9` by key position: ⌥ makes another character on a Mac, so the character is never read. */
const digitOf = (press: KeyboardEvent): number | null => {
  const found = /^Digit([0-9])$/.exec(press.code);

  return found ? Number(found[1]) : null;
};

/** The ids of the Agent rows the Rail shows, top to bottom: a Door's row (its Workstream's) counts, a Terminal's does not, and a collapsed Workstream's Agents are not in the DOM. */
const shownAgentIds = (kinds: ReadonlyMap<string, string>): string[] =>
  [...document.querySelectorAll<HTMLElement>('[role="tree"] [role="treeitem"][data-id]')].flatMap((row) => {
    const id = row.dataset.id ?? "";

    return kinds.get(id) === "agent" || kinds.get(id) === "workstream" ? [id] : [];
  });

const focusPane = (): void => document.querySelector<HTMLElement>(".pane-screen textarea, .pane-screen [tabindex]")?.focus();

const focusThread = (): void => document.querySelector<HTMLElement>("[data-thread-input]")?.focus();

/** U107: `⌥1` to `⌥9` select the n-th Agent row and begin a Takeover of it; `⌥0` ends it. No UI of its own; `onFailure` gets the message of a failed call, and `null` when the next jump starts. */
export const Jump = (props: { onFailure: (message: string | null) => void }) => {
  const { app, rail, events, daemonExit } = useConnectedProject();
  const [taken, setTaken] = createSignal<string | null>(null);

  // A Takeover ends by itself when its Agent exits (B6), and a Daemon that starts again has none (B8): the Daemon's word is the truth.
  const unsubscribe = events.subscribe((event) => {
    if (event.name !== "takeover.changed") return;

    if (event.data.on) setTaken(event.data.agent);
    else if (taken() === event.data.agent) setTaken(null);
  });

  onCleanup(unsubscribe);

  const call = async (method: "takeover.begin" | "takeover.end", agent: string): Promise<void> => {
    if (daemonExit() !== null) return;

    try {
      await app.rpc(method, { agent });
    } catch (error) {
      if (!(error instanceof Error)) throw error;

      props.onFailure(error.message);

      return;
    }

    setTaken(method === "takeover.begin" ? agent : null);
  };

  const endCurrent = async (): Promise<void> => {
    const current = taken();

    if (current !== null) await call("takeover.end", current);
  };

  /** A queue, so two quick chords end one Takeover before the next begins. */
  let last: Promise<void> = Promise.resolve();

  const jump = async (agent: string): Promise<void> => {
    if (taken() !== null && taken() !== agent) await endCurrent();

    if (taken() !== agent) await call("takeover.begin", agent);
  };

  const onKey = (press: KeyboardEvent): void => {
    if (!press.altKey || press.metaKey || press.ctrlKey || press.shiftKey) return;

    const digit = digitOf(press);

    if (digit === null) return;

    if (digit === 0) {
      press.preventDefault();
      press.stopPropagation();
      focusThread();
      last = last.then(endCurrent);

      return;
    }

    const kinds = new Map(rail.nodes.map((node) => [node.id, node.kind]));
    const target = shownAgentIds(kinds)[digit - 1];

    if (target === undefined) return;

    press.preventDefault();
    press.stopPropagation();
    props.onFailure(null);
    rail.select(target);
    focusPane();
    last = last.then(() => jump(target));
  };

  // Capture, and before the Pane or xterm sees it: the chord is the webview's wherever focus is (it narrows U12).
  document.addEventListener("keydown", onKey, true);
  onCleanup(() => document.removeEventListener("keydown", onKey, true));

  return null;
};
