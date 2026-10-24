import { captureFocus } from "../keys/focus";
import { ReservedLine } from "../ink/ReservedLine";
import { For, Show, createEffect, createMemo, createResource, createSignal, onCleanup } from "solid-js";
import type { ContinuationAction } from "@contracts/message/ContinuationAction";
import type { Progress } from "@contracts/message/Progress";
import type { Reconciliation } from "@contracts/message/Reconciliation";
import type { ResumeWorkParams } from "@contracts/message/ResumeWorkParams";
import type { ResultProgress } from "@contracts/message/ResultProgress";
import { openDeliverable } from "../ask/openDeliverable";
import { INTERNAL_CODE, RpcError } from "../app/seam";
import { copy } from "../copy";
import { Icon } from "../ink/Icon";
import { useConnectedProject } from "../state/connectedProject";
import { IconButton } from "../ui/IconButton";
import { Disclosure } from "../ui/Disclosure";

type Read = { id: string; progress: Progress; plan: Reconciliation } | { id: string; failure: string };

/** The Daemon's cause starts in lower case and may end without a stop; a note reads as a sentence. */
const sentence = (cause: string) => {
  const text = cause.trim();

  return text === "" ? "" : `${text[0]?.toUpperCase()}${text.slice(1)}${/[.!?]$/.test(text) ? "" : "."}`;
};

type Phase = "idle" | "pending" | "failed" | "unknown" | "resumed" | "nothing";

/** U3078i: the retained progress of one Agent's work and the explicit Resume. Reading sends nothing; only the Resume control calls `message.resumeWork`, always for the Agent and action it read. */
export const WorkProgress = (props: { id: string }) =>
  <Show when={props.id} keyed>{id => <ProgressContent id={id} />}</Show>;

const ProgressContent = (props: { id: string }) => {
  const { app, project, rail, events, daemonExit } = useConnectedProject();
  const [version, setVersion] = createSignal(0);

  const [read] = createResource(() => [props.id, version()] as const, async ([id]): Promise<Read> => {
    try {
      const query = { project: project.path, id };
      const [progress, plan] = await Promise.all([app.rpc("message.progress", query), app.rpc("message.reconcileWork", query)]);

      return { id, progress, plan };
    } catch (error) {
      if (!(error instanceof Error)) throw error;

      return { id, failure: error.message };
    }
  });

  const refresh = () => { if (daemonExit() === null) setVersion((count) => count + 1); };

  const unsubscribe = events.subscribe((event) => { if (event.name === "rail.changed") refresh(); });
  onCleanup(unsubscribe);

  const loaded = createMemo((previous: Extract<Read, { progress: Progress }> | undefined) => {
    const each = read.latest;

    if (each && "progress" in each && each.id === props.id) return each;

    return previous?.id === props.id ? previous : undefined;
  });

  const [phase, setPhase] = createSignal<Phase>("idle");
  const [cause, setCause] = createSignal("");
  const [failures, setFailures] = createSignal<Record<string, string>>({});
  const [dismissedRead, setDismissedRead] = createSignal<Read>();
  const [resumeFailure, setResumeFailure] = createSignal<"failed" | "unknown" | null>(null);
  let wasPresent = rail.nodes.some(node => node.id === props.id);
  createEffect(() => {
    if (rail.failure() !== null) return;
    const present = rail.nodes.some(node => node.id === props.id);

    if (wasPresent && !present) setResumeFailure(null);
    wasPresent = present;
  });

  const clearReport = (report: string) => setFailures(previous => {
    const next = { ...previous };
    delete next[report];

    return next;
  });

  createEffect(() => {
    const each = loaded();

    if (each) {
      const reports = new Set(each.progress.results.map(result => result.report.report));
      setFailures(previous => Object.fromEntries(Object.entries(previous).filter(([report]) => reports.has(report))));
    }
  });
  const [opening, setOpening] = createSignal<string | null>(null);

  let request: string | null = null;
  let carried = "";
  let button: HTMLButtonElement | undefined;
  let status: HTMLParagraphElement | undefined;
  let disposed = false;
  onCleanup(() => { disposed = true; });

  let statusFocus = captureFocus();

  const open = (): ContinuationAction[] => loaded()?.plan.actions.filter((action) => action.decision === null) ?? [];

  createEffect(() => {
    if (open().length === 0 && (phase() === "resumed" || phase() === "nothing")) queueMicrotask(() => {
      if (!disposed && document.activeElement === document.body && status?.isConnected) statusFocus(() => status?.focus());
    });
  });

  const inspect = async (result: ResultProgress) => {
    const report = result.report.report;

    if (opening() !== null || daemonExit() !== null) return;
    setOpening(report);

    try {
      const unavailable = await openDeliverable(app, result.report.publication.project, result.report.publication.id);

      if (!disposed) {
        if (unavailable === null) clearReport(report);
        else setFailures(previous => ({ ...previous, [report]: unavailable }));
      }
    } catch (error) {
      if (!(error instanceof Error)) throw error;

      if (!disposed) setFailures(previous => ({ ...previous, [report]: error.message }));
    } finally {
      if (!disposed) setOpening(null);
    }
  };

  /** The Daemon holds what was accepted, so an unknown reply reads it again by identity before it asks a second time. */
  const settled = async (agent: string, actions: string[]): Promise<boolean> => {
    const plan = await app.rpc("message.reconcileWork", { project: project.path, id: agent });

    return actions.every((id) => plan.actions.find((action) => action.id === id)?.decision != null);
  };

  const resume = async () => {
    const agent = props.id;
    const chosen = open();

    if (phase() === "pending" || daemonExit() !== null || chosen.length === 0) return;
    const focus = captureFocus();
    statusFocus = captureFocus();
    const identity = chosen.map((action) => action.id);
    const previous = phase();

    // One request id carries one payload: other actions are a new request.
    if (carried !== identity.join("\n")) request = null;
    carried = identity.join("\n");
    request ??= crypto.randomUUID();
    setPhase("pending");

    try {
      if (previous === "unknown" && await settled(agent, identity)) {
        request = null;
        setPhase("resumed");
        setResumeFailure(null);
        refresh();

        return;
      }

      const params: ResumeWorkParams = { project: project.path, id: agent, request };

      const [only] = identity;

      if (only !== undefined && identity.length === 1) params.action = only;

      const answer = await app.rpc("message.resumeWork", params);

      request = null;

      if (!disposed) {
        setPhase(answer.decision === null ? "nothing" : "resumed");
        setResumeFailure(null);
      }
    } catch (thrown) {
      const known = thrown instanceof RpcError && thrown.code !== INTERNAL_CODE;

      if (!disposed) {
        setResumeFailure(known ? "failed" : "unknown");
        setCause(thrown instanceof Error ? thrown.message : "");
        setPhase(known ? "failed" : "unknown");
      }
    } finally {
      if (!disposed) {
        refresh();
        queueMicrotask(() => { if (!disposed && button?.isConnected) focus(() => button?.focus()); });
      }
    }
  };

  const confirmed = (result: ResultProgress) => result.observations.some((each) => each.verdict === "passed");

  const receipt = (result: ResultProgress) => {
    const each = result.observations.findLast((seen) => seen.verdict === "resolved");

    return each?.verdict === "resolved" ? each.evidence.outcome.kind : null;
  };

  const noteOf = (result: ResultProgress, plan: Reconciliation): { cause: string; action: string }[] => {
    const blocked = plan.blocked.filter((each) => each.report === result.report.report);

    if (blocked.length > 0) return blocked;
    const last = result.observations.findLast((each) => each.verdict === "unknown");

    return last && !confirmed(result) ? [last] : [];
  };

  const rows = createMemo(() => {
    const each = loaded();

    return each ? { current: each.progress.results.filter((result) => result.current), earlier: each.progress.results.filter((result) => !result.current) } : null;
  });

  const row = (result: ResultProgress, plan: Reconciliation) => {
    const pending = () => plan.pending_checks.some((target) => target.id === result.report.request.target.id
      && target.revision === result.report.request.target.revision);

    const outcome = () => receipt(result);
    const next = () => result.report.request.next.trim();

    return (
      <li class="progress-row" data-report={result.report.report} data-current={String(result.current)}>
        <p class="progress-result">{result.report.request.result}</p>
        <p class="progress-states">
          <Show when={!result.current}><span class="progress-state" data-state="older"><Icon name="circle" />{copy.progressOlder}</span></Show>
          <Show when={result.acceptances.length > 0}><span class="progress-state" data-state="accepted"><Icon name="check" />{copy.progressAccepted}</span></Show>
          <Show when={confirmed(result)}><span class="progress-state" data-state="checked"><Icon name="check" />{copy.progressChecked}</span></Show>
          <Show when={outcome()}>{(kind) => <span class="progress-state" data-state={kind() === "completed" ? "checked" : "unknown"}><Icon name={kind() === "completed" ? "check" : "x"} />{kind() === "completed" ? copy.progressReceipt : copy.progressNotPerformed}</span>}</Show>
          <Show when={result.current && pending() && !confirmed(result)}><span class="progress-state" data-state="pending"><Icon name="circle" />{copy.progressChecksPending}</span></Show>
          <Show when={result.report.request.state === "unknown-effect" && outcome() === null}><span class="progress-state" data-state="unknown"><Icon name="diamond" />{copy.progressUnknown}</span></Show>
          <Show when={result.availability.kind === "unavailable"}><span class="progress-state" data-state="unavailable"><Icon name="x" />{copy.progressUnavailable}</span></Show>
        </p>
        <Show when={next() !== ""}><p class="progress-next"><span class="progress-label">{copy.progressRemaining}</span> {next()}</p></Show>
        <For each={noteOf(result, plan)}>{(note) => <p class="progress-note" role="note">{sentence(note.cause)} {sentence(note.action)}</p>}</For>
        <button class="word progress-evidence" data-button="quiet" type="button" aria-label={copy.openEvidence(result.report.publication.title)}
          aria-disabled={opening() !== null ? true : undefined} onClick={() => void inspect(result)}>{result.report.publication.title}</button>
        <ReservedLine message={failures()[result.report.report]}><IconButton aria-label={copy.clearName(copy.error.toLowerCase())} onClick={() => clearReport(result.report.report)}><Icon name="x" /></IconButton></ReservedLine>
      </li>
    );
  };

  const unread = () => {
    const each = read.latest;

    return each !== dismissedRead() && each?.id === props.id && "failure" in each ? each.failure : undefined;
  };

  return (
    <>
      <ReservedLine message={unread() ? copy.progressUnreadable(unread()!) : null}>
        <button class="word" data-button="quiet" type="button" onClick={refresh}>{copy.progressRetry}</button>
        <IconButton aria-label={copy.clearName(copy.error.toLowerCase())} onClick={() => setDismissedRead(read.latest)}><Icon name="x" /></IconButton>
      </ReservedLine>
      <Show when={loaded()}>
        {(data) => (
          <Show when={data().progress.results.length > 0}>
            <section class="progress-section" aria-label={copy.progress}>
              <div class="progress-head">
                <h3>{copy.progress}</h3>
                <Show when={open().length > 0}>
                  <button ref={button} class="word" data-button="primary" type="button" aria-disabled={phase() === "pending" || daemonExit() !== null ? true : undefined}
                    onClick={() => void resume()}>{phase() === "pending" ? copy.resuming : phase() === "failed" || phase() === "unknown" ? copy.resumeRetry : copy.resumeUnfinished}</button>
                </Show>
              </div>
              <ReservedLine ref={status} class="progress-status" tabindex="-1" message={resumeFailure() === "unknown" ? copy.resumeUnknown : resumeFailure() === "failed" ? copy.resumeFailed(sentence(cause())) : null}
                status={phase() === "resumed" ? copy.resumed : phase() === "nothing" ? copy.resumeNothing : null}><Show when={resumeFailure()}><IconButton aria-label={copy.clearName(copy.error.toLowerCase())} onClick={() => setResumeFailure(null)}><Icon name="x" /></IconButton></Show></ReservedLine>
              <ul class="progress-rows"><For each={rows()?.current}>{(result) => row(result, data().plan)}</For></ul>
              <Show when={(rows()?.earlier.length ?? 0) > 0}>
                <Disclosure class="progress-earlier" label={copy.progressEarlier(data().progress.results.filter((result) => !result.current).length)}>
                  <ul class="progress-rows"><For each={rows()?.earlier}>{(result) => row(result, data().plan)}</For></ul>
                </Disclosure>
              </Show>
            </section>
          </Show>
        )}
      </Show>
    </>
  );
};
