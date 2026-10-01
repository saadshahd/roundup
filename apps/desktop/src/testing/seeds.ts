import type { Actor } from "@contracts/Actor";
import type { Event as DaemonEvent } from "@contracts/Event";
import type { EventData } from "@contracts/EventData";
import type { Kind } from "@contracts/Kind";
import type { NodeKind } from "@contracts/agent/NodeKind";
import type { RailNode } from "@contracts/agent/RailNode";
import type { Pad } from "@contracts/pad/Pad";
import type { TerminalInfo } from "@contracts/terminal/TerminalInfo";
import type { Todo } from "@contracts/todo/Todo";
import { RpcError } from "../app/seam";
import type { Project } from "../app/seam";
import { createFakeApp } from "./fakeApp";
import type { FakeApp } from "./fakeApp";

const CONFLICT = -32003;

const MINUTE = 60_000;

const USER: Actor = { kind: "user", id: "you", parent: null };

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

const node = (id: string, kind: NodeKind, name: string, parent: string | null, order: number, status: RailNode["status"], meta = false): RailNode => ({
  id,
  kind,
  name,
  parent,
  order,
  status,
  meta,
  terminal_id: kind === "group" && !meta ? null : `t-${id}`,
});

const statusAt = (now: number, kind: Kind, label: string, minutesAgo: number) => ({
  kind,
  label,
  since: now - minutesAgo * MINUTE,
});

const cyclingAgents = (now: number, count: number, parentOf: (index: number) => string | null): RailNode[] =>
  Array.from({ length: count }, (_, index) => {
    const kind = KINDS[index % KINDS.length] ?? "idle";

    return node(`agent-${index + 1}`, "agent", `agent-${index + 1}`, parentOf(index), index, statusAt(now, kind, `${kind} label ${index + 1}`, index * 3 + 1));
  });

/** Groups nested two deep, a Meta-agent with children, long names and Terminals; 40 nodes in all. */
const nestedTree = (now: number): RailNode[] => [
  node("backend", "group", "backend", null, 0, null),
  node("auth", "group", "auth-refactor", "backend", 0, null),
  node("payments", "group", "payments meta-agent with a very long name that overflows the rail", null, 1, statusAt(now, "working", "coordinating 3 children", 42), true),
  node("migrate", "agent", "migrate-db", "backend", 1, statusAt(now, "needs-you", "asks: keep v1 routes? and a long label that should truncate in the live line", 4)),
  node("tokens", "agent", "token-rotation", "auth", 0, statusAt(now, "working", "editing src/auth/token.rs", 12)),
  node("sessions", "agent", "login-store", "auth", 1, statusAt(now, "error", "tests failed: 3", 1)),
  node("docs", "agent", "docs", "auth", 2, statusAt(now, "done", "finished", 30)),
  node("shell", "terminal", "zsh", "backend", 2, null),
  node("loose-shell", "terminal", "zsh", null, 2, null),
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

const terminalOf = (target: RailNode): TerminalInfo => {
  const exitCode = exitCodeOf(target);

  return { id: `t-${target.id}`, cwd: PROJECT.path, title: null, running: exitCode === null, exit_code: exitCode };
};

const failMissing = (what: string): never => {
  throw new RpcError(-32001, `no ${what}`);
};

/** Installs a Daemon that keeps the Rail, Todos and Pads in memory and tells the App when a write changes them. */
const installDaemon = (app: FakeApp, tree: RailNode[], now: number, withShelf: boolean): Controls => {
  const nodes = structuredClone(tree);
  const todos = withShelf ? todosAt(now) : [];
  const pads = withShelf ? padsAt(now) : [];
  const emit = (event: DaemonEvent) => app.emit(event);
  const send = (data: EventData) => emit({ actor: USER, ...data });

  const changed = <T>(value: T): T => {
    queueMicrotask(() => send({ name: "rail.changed" }));

    return value;
  };

  const append = (kind: NodeKind, name: string, parent: string | null, status: RailNode["status"]): RailNode => {
    const added = node(`n${nodes.length + 1}`, kind, name, parent, nodes.filter((other) => other.parent === parent).length, status);
    nodes.push(added);

    return changed(added);
  };

  const find = (id: string): RailNode => {
    const found = nodes.find((other) => other.id === id);

    if (!found) throw new RpcError(-32001, `no node ${id}`);

    return found;
  };

  app.handlers["rail.tree"] = () => structuredClone(nodes);
  app.handlers["terminal.list"] = () => nodes.filter((other) => other.terminal_id).map(terminalOf);
  app.handlers["todo.list"] = () => structuredClone(todos);
  app.handlers["todo.get"] = ({ id }) => structuredClone(todos.find((todo) => todo.id === id) ?? failMissing(`todo ${id}`));
  app.handlers["todo.create"] = ({ title }) => {
    const created: Todo = { id: todos.length + 1, title, body: "", done: false, blockers: [], blocked: false, created_at: Date.now() };
    todos.push(created);
    send({ name: "todo.created", data: created });

    return created;
  };

  app.handlers["pad.list"] = () => structuredClone(pads);
  app.handlers["pad.read"] = ({ name }) => structuredClone(pads.find((pad) => pad.name === name) ?? failMissing(`pad ${name}`));
  app.handlers["provenance.history"] = () => [{ actor: USER, verb: "wrote", item: "todo:1", at: now - 5 * MINUTE }];
  app.handlers["agent.spawn"] = ({ parent }) => append("agent", "agent", parent, statusAt(Date.now(), "working", "starting", 0));
  app.handlers["rail.spawnTerminal"] = ({ parent }) => append("terminal", "zsh", parent, null);
  app.handlers["rail.createGroup"] = ({ name, parent }) => append("group", name, parent, null);
  app.handlers["rail.rename"] = ({ id, name }) => changed(Object.assign(find(id), { name }));
  app.handlers["terminal.resize"] = () => null;
  app.handlers["terminal.write"] = () => null;
  app.handlers["terminal.kill"] = () => null;
  app.handlers["agent.stop"] = () => null;

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
    emit,
    setStatus: (id, kind, label) => {
      const status = statusAt(Date.now(), kind, label, 0);
      find(id).status = status;
      send({ name: "agent.status", data: { id, status } });
    },
    writeOutput: (terminalId, text) => send({ name: "terminal.output", data: { id: terminalId, data: btoa(text) } }),
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
