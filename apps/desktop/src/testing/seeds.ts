import type { Event as DaemonEvent } from "@contracts/Event";
import type { EventData } from "@contracts/EventData";
import type { Kind } from "@contracts/Kind";
import type { RailNode } from "@contracts/agent/RailNode";
import type { Pad } from "@contracts/pad/Pad";
import type { Todo } from "@contracts/todo/Todo";
import { RpcError } from "../app/seam";
import type { Project } from "../app/seam";
import { createFakeApp } from "./fakeApp";
import { toBase64 } from "../terminal/base64";
import type { FakeApp } from "./fakeApp";
import { agent, group, MINUTE, metaAgent, terminal, USER } from "./nodes";
import { padHandlers, todoHandlers } from "./stores";

const CONFLICT = -32003;

const NOT_FOUND = -32001;

const KINDS: Kind[] = ["error", "needs-you", "blocked", "working", "idle", "done"];

export const SEEDS = ["first-run", "agents-10", "tree-40", "daemon-exits", "conflict"] as const;

export type SeedName = (typeof SEEDS)[number];

export const isSeedName = (name: string): name is SeedName => SEEDS.some((seed) => seed === name);

const PROJECT: Project = { name: "harness", path: "/Users/you/harness" };

/** What the page can do to the Daemon the App is talking to. */
export type Controls = {
  app: FakeApp;
  emit(event: DaemonEvent): void;
  /** Replaces one node's Status and sends `agent.status`, as the Daemon does when an Adapter decides a new Kind. */
  setStatus(id: string, kind: Kind, label: string): void;
  /** Sends `terminal.output` to the Terminal as if its program wrote `text`. */
  writeOutput(terminalId: string, text: string): void;
  /** Makes the next `rpc` call, whatever its method, reject with `code` and `message`. */
  failNext(code: number, message: string): void;
};

const statusAt = (now: number, kind: Kind, label: string, minutesAgo: number) => ({
  kind,
  label,
  since: now - minutesAgo * MINUTE,
});

const cyclingAgents = (now: number, count: number, parentOf: (index: number) => string | null): RailNode[] =>
  Array.from({ length: count }, (_, index) => {
    const kind = KINDS[index % KINDS.length] ?? "idle";
    const name = `agent-${index + 1}`;

    return agent(name, kind, `${kind} label ${index + 1}`, {
      parent: parentOf(index),
      order: index,
      status: statusAt(now, kind, `${kind} label ${index + 1}`, index * 3 + 1),
    });
  });

/** Groups nested two deep, a Meta-agent with children, long names and Terminals; 40 nodes in all. */
const nestedTree = (now: number): RailNode[] => [
  group("backend", { name: "backend" }),
  group("auth", { name: "auth-refactor", parent: "backend" }),
  metaAgent("payments", "working", "coordinating 3 children", {
    name: "payments meta-agent with a very long name that overflows the rail",
    order: 1,
    status: statusAt(now, "working", "coordinating 3 children", 42),
  }),
  agent("migrate", "needs-you", "asks: keep v1 routes? and a long label that should truncate in the live line", {
    name: "migrate-db",
    parent: "backend",
    order: 1,
    status: statusAt(now, "needs-you", "asks: keep v1 routes? and a long label that should truncate in the live line", 4),
  }),
  agent("tokens", "working", "editing src/auth/token.rs", {
    name: "token-rotation",
    parent: "auth",
    status: statusAt(now, "working", "editing src/auth/token.rs", 12),
  }),
  agent("logins", "error", "tests failed: 3", {
    name: "login-store",
    parent: "auth",
    order: 1,
    status: statusAt(now, "error", "tests failed: 3", 1),
  }),
  agent("docs", "done", "finished", { parent: "auth", order: 2, status: statusAt(now, "done", "finished", 30) }),
  terminal("shell", { name: "zsh", parent: "backend", order: 2 }),
  terminal("loose-shell", { name: "zsh", order: 2 }),
  ...cyclingAgents(now, 31, (index) => (index % 3 === 0 ? "payments" : null)),
];

const todosAt = (now: number): Todo[] =>
  Array.from({ length: 8 }, (_, index) => ({
    id: index + 1,
    title: index === 2 ? "a very long todo title that goes on and on past the width of the shelf column" : `todo ${index + 1}`,
    body: "body text",
    done: index === 6,
    blockers: index === 3 ? [3, 5] : [],
    blocked: index === 3,
    created_at: now,
    creator: USER,
  }));

const padsAt = (now: number): Pad[] =>
  Array.from({ length: 4 }, (_, index) => ({
    name: index === 1 ? "a-pad-with-a-very-long-name-here" : `pad-${index}`,
    owner: index % 2 === 1 ? USER : { kind: "agent", id: "agent-1", parent: null },
    text: "# hello\nsome text\n".repeat(30),
    updated_at: now,
  }));

const exitCodeOf = (target: RailNode): number | null => {
  if (target.status?.kind === "error") return 1;

  return target.status?.kind === "done" ? 0 : null;
};

/** Installs a Daemon that keeps the Rail, Todos and Pads in memory and sends the Events a real one sends after the same write. */
const installDaemon = (app: FakeApp, tree: RailNode[], now: number, withShelf: boolean): Controls => {
  const nodes = structuredClone(tree);
  const exits = new Map<string, number | null>();
  const outputOffsets = new Map<string, number>();
  const outputBytes = new Map<string, Uint8Array[]>();

  for (const other of nodes) {
    if (other.terminal_id && exitCodeOf(other) !== null) exits.set(other.terminal_id, exitCodeOf(other));
  }

  const send = (data: EventData) => app.emit({ actor: USER, ...data });

  const changed = <T,>(value: T): T => {
    queueMicrotask(() => send({ name: "rail.changed" }));

    return value;
  };

  const find = (id: string): RailNode => {
    const found = nodes.find((other) => other.id === id);

    if (!found) throw new RpcError(NOT_FOUND, `not found: node ${id}`);

    return found;
  };

  const append = (parent: string | null, made: (id: string, order: number) => RailNode): RailNode => {
    const added = made(`n${nodes.length + 1}`, nodes.filter((other) => other.parent === parent).length);
    nodes.push(added);

    return changed(added);
  };

  const exit = (terminalId: string) => {
    exits.set(terminalId, null);
    send({ name: "terminal.exited", data: { id: terminalId, code: null } });
  };

  Object.assign(
    app.handlers,
    todoHandlers({ todos: withShelf ? todosAt(now) : [] }, send),
    padHandlers({ pads: withShelf ? padsAt(now) : [], history: [{ actor: USER, verb: "wrote", item: "todo:1", at: now - 5 * MINUTE }] }, send),
  );
  app.handlers["rail.tree"] = () => structuredClone(nodes);
  app.handlers["terminal.list"] = () =>
    nodes.flatMap((other) =>
      other.terminal_id
        ? [{ id: other.terminal_id, cwd: PROJECT.path, title: null, running: !exits.has(other.terminal_id), exit_code: exits.get(other.terminal_id) ?? null }]
        : [],
    );
  app.handlers["agent.spawn"] = ({ parent }) =>
    append(parent, (id, order) => agent(id, "working", "starting", { name: "agent", parent, order, status: statusAt(Date.now(), "working", "starting", 0) }));
  app.handlers["rail.spawnTerminal"] = ({ parent }) => append(parent, (id, order) => terminal(id, { name: "zsh", parent, order }));
  app.handlers["rail.createGroup"] = ({ name, parent }) => append(parent, (id, order) => group(id, { name, parent, order }));
  app.handlers["rail.rename"] = ({ id, name }) => changed(Object.assign(find(id), { name }));
  app.handlers["rail.promote"] = ({ id }) =>
    changed(Object.assign(find(id), { meta: true, terminal_id: `t-${id}`, status: statusAt(Date.now(), "working", "starting", 0) }));
  app.handlers["rail.move"] = ({ id, parent, index }) => {
    const moved = find(id);
    const siblings = nodes.filter((other) => other.parent === parent && other.id !== id).sort((left, right) => left.order - right.order);
    siblings.splice(index, 0, Object.assign(moved, { parent }));
    siblings.forEach((sibling, order) => Object.assign(sibling, { order }));

    return changed(null);
  };

  app.handlers["rail.remove"] = ({ id }) => {
    const node = find(id);

    if (node.kind === "agent" || node.meta) exit(node.terminal_id ?? `t-${id}`);
    else if (node.kind === "terminal" && node.terminal_id) exit(node.terminal_id);

    const siblings = nodes.filter((other) => other.parent === node.parent).sort((left, right) => left.order - right.order);
    const at = siblings.findIndex((other) => other.id === id);
    const children = nodes.filter((other) => other.parent === id).sort((left, right) => left.order - right.order);
    siblings.splice(at, 1, ...children.map((child) => Object.assign(child, { parent: node.parent })));
    siblings.forEach((sibling, order) => Object.assign(sibling, { order }));
    nodes.splice(nodes.indexOf(node), 1);

    return changed(null);
  };

  app.handlers["terminal.resize"] = () => null;
  app.handlers["terminal.snapshot"] = ({ id }) => {
    if (!nodes.some((node) => node.terminal_id === id)) throw new RpcError(NOT_FOUND, `not found: terminal ${id}`);

    const chunks = outputBytes.get(id) ?? [];
    const bytes = new Uint8Array(outputOffsets.get(id) ?? 0);
    let at = 0;

    for (const chunk of chunks) {
      bytes.set(chunk, at);
      at += chunk.length;
    }

    return { cols: 100, rows: 30, after: at, data: toBase64(bytes) };
  };

  app.handlers["terminal.write"] = () => null;
  app.handlers["terminal.kill"] = (terminalId) => {
    exit(terminalId.id);

    return null;
  };

  app.handlers["agent.stop"] = ({ id }) => {
    exit(find(id).terminal_id ?? `t-${id}`);

    return null;
  };

  let failure: RpcError | null = null;
  const rpc = app.rpc;
  app.rpc = async (method, params) => {
    if (failure) {
      const failing = failure;
      failure = null;
      throw failing;
    }

    return rpc(method, params);
  };

  return {
    app,
    emit: (event) => app.emit(event),
    setStatus: (id, kind, label) => {
      const status = statusAt(Date.now(), kind, label, 0);
      find(id).status = status;
      send({ name: "agent.status", data: { id, status } });
    },
    writeOutput: (terminalId, text) => {
      const bytes = new TextEncoder().encode(text);
      const data = toBase64(bytes);
      const offset = outputOffsets.get(terminalId) ?? 0;
      outputOffsets.set(terminalId, offset + bytes.length);
      outputBytes.set(terminalId, [...(outputBytes.get(terminalId) ?? []), bytes]);
      send({ name: "terminal.output", data: { id: terminalId, offset, data } });
    },
    failNext: (code, message) => {
      failure = new RpcError(code, message);
    },
  };
};

/** Runs `action` once the App has subscribed and its first reads have settled. */
const afterFirstLoad = (app: FakeApp, action: () => void) => {
  const subscribe = app.subscribe;
  app.subscribe = async (listener) => {
    await subscribe(listener);
    setTimeout(action, 0);
  };
};

/** A fake App on a Daemon holding the named seed; `now` is the clock the Statuses are measured from. */
export const seedApp = (name: SeedName, now: number): Controls => {
  const app = createFakeApp();
  const tree = name === "tree-40" ? nestedTree(now) : cyclingAgents(now, 10, () => null);
  const controls = installDaemon(app, name === "first-run" ? [] : tree, now, name === "tree-40");
  app.opened.project = name === "first-run" ? null : PROJECT;

  if (name === "daemon-exits") afterFirstLoad(app, () => app.exitDaemon({ code: 1 }));

  if (name === "conflict") afterFirstLoad(app, () => controls.failNext(CONFLICT, "name already taken"));

  return controls;
};
