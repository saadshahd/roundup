import { createSignal } from "solid-js";
import type { Accessor } from "solid-js";
import type { Decision } from "@contracts/decision/Decision";
import type { Events } from "../app/events";
import type { AppSeam, DaemonExit } from "../app/seam";

export type DecisionsState = {
  /** The Decision the Agent (or a Workstream's Door) waits on; the newest when a replacement is still arriving. */
  of(agent: string | null): Decision | undefined;
  /** The message of the last failed `decision.list`, until a later list succeeds. */
  listFailure(): string | null;
  retryList(): void;
  answering(id: string): boolean;
  answerFailure(id: string): string | null;
  answer(id: string, answer: string): Promise<void>;
};

/**
 * Subscribes before it lists, and reconciles by id: an id an Event touched while a list was in flight keeps what the
 * Event said, so an old list neither resurrects a cleared Decision nor overwrites its replacement.
 */
export const createDecisions = (app: AppSeam, events: Events, daemonExit: Accessor<DaemonExit | null>): DecisionsState => {
  const [decisions, setDecisions] = createSignal<readonly Decision[]>([]);
  const [listFailure, setListFailure] = createSignal<string | null>(null);
  const [pending, setPending] = createSignal<ReadonlySet<string>>(new Set());
  const [failures, setFailures] = createSignal<ReadonlyMap<string, string>>(new Map());
  let reading: Set<string> | null = null;

  const without = (id: string) => setDecisions((current) => current.filter((each) => each.id !== id));

  const forget = (id: string) => {
    reading?.add(id);
    without(id);
    setFailures((current) => new Map([...current].filter(([key]) => key !== id)));
  };

  events.subscribe((event) => {
    if (event.name === "decision.opened") {
      reading?.add(event.data.id);
      setDecisions((current) => [...current.filter((each) => each.id !== event.data.id), event.data]);
    } else if (event.name === "decision.cleared") {
      forget(event.data.id);
    }
  });

  const list = async (): Promise<void> => {
    const touched = new Set<string>();

    reading = touched;

    try {
      const listed = await app.rpc("decision.list", null);

      setDecisions((current) => [...current, ...listed.filter((each) => !touched.has(each.id) && !current.some((have) => have.id === each.id))]);
      setListFailure(null);
    } catch (thrown) {
      if (!(thrown instanceof Error)) throw thrown;

      setListFailure(thrown.message);
    } finally {
      if (reading === touched) reading = null;
    }
  };

  void list();

  const answer = async (id: string, text: string): Promise<void> => {
    if (daemonExit() !== null || pending().has(id)) return;

    setPending((current) => new Set(current).add(id));
    setFailures((current) => new Map([...current].filter(([key]) => key !== id)));

    try {
      const proof = await app.daemonProof();

      await app.rpc("decision.answer", { id, answer: text, proof });
      forget(id);
    } catch (thrown) {
      if (!(thrown instanceof Error)) throw thrown;

      setFailures((current) => new Map(current).set(id, thrown.message));
    } finally {
      setPending((current) => new Set([...current].filter((key) => key !== id)));
    }
  };

  return {
    of: (agent) => decisions().filter((each) => each.agent === agent).reduce<Decision | undefined>((newest, each) => (!newest || each.opened_at >= newest.opened_at ? each : newest), undefined),
    listFailure,
    retryList: () => void list(),
    answering: (id) => pending().has(id),
    answerFailure: (id) => failures().get(id) ?? null,
    answer,
  };
};
