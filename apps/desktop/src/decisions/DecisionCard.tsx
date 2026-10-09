import { For, Show } from "solid-js";
import * as v from "valibot";
import type { Decision } from "@contracts/decision/Decision";
import { ErrorLine } from "../ink/ErrorLine";
import { useConnectedProject } from "../state/connectedProject";
import "./styles.css";

const json = v.pipe(v.string(), v.parseJson());

const questionSchema = v.pipe(json, v.object({ question: v.string(), answers: v.array(v.string()) }));

const commandSchema = v.pipe(json, v.object({ command: v.string() }));

/** The question an `ask_user` Decision carries (H14), or null when its payload is not readable. */
const questionOf = (args: string) => {
  const result = v.safeParse(questionSchema, args);

  return result.success ? result.output : null;
};

/** The command of a tool call when it has one, else the arguments as the Daemon sent them, indented when they are JSON. */
const readable = (args: string): string => {
  const command = v.safeParse(commandSchema, args);

  if (command.success) return command.output.command;

  const parsed = v.safeParse(json, args);

  return parsed.success ? JSON.stringify(parsed.output, null, 2) : args;
};

/** The Decision an Agent waits on, with the buttons that answer it; `focusTerminal` hands the keyboard to that Agent's Terminal. */
export const DecisionCard = (props: { decision: Decision; name: string; focusTerminal: () => void; ref: (card: HTMLElement) => void; onFocusIn: () => void; onFocusOut: (leave: FocusEvent) => void }) => {
  const { decisions, daemonExit } = useConnectedProject();
  const id = () => props.decision.id;
  const asked = () => (props.decision.tool === "ask_user" ? questionOf(props.decision.args) : null);
  const unreadable = () => props.decision.tool === "ask_user" && asked() === null;

  const answers = () => {
    if (!props.decision.answerable || unreadable()) return [];

    return asked()?.answers ?? ["allow", "deny"];
  };

  const stuck = () => daemonExit() !== null || decisions.answering(id());

  return (
    <section class="decision-card" aria-label={`Decision for ${props.name}`} ref={props.ref} onFocusIn={props.onFocusIn} onFocusOut={props.onFocusOut}>
      <p class="decision-title">
        <span>{props.name}</span> <span class="light">{props.decision.tool}</span>
      </p>
      <Show when={asked()} fallback={<pre class="decision-args">{readable(props.decision.args)}</pre>}>
        {(question) => <p class="decision-question">{question().question}</p>}
      </Show>
      <Show when={unreadable()}>
        <ErrorLine message="unreadable question: answer in the Terminal" />
      </Show>
      <Show when={!props.decision.answerable && !unreadable()}>
        <p class="light">answer in the Terminal</p>
      </Show>
      <Show when={decisions.answerFailure(id())}>{(message) => <ErrorLine message={message()} />}</Show>
      <div class="decision-buttons">
        <For each={answers()}>
          {(each) => (
            <button type="button" class="word" disabled={stuck()} onClick={() => void decisions.answer(id(), each)}>
              {each}
            </button>
          )}
        </For>
        <Show when={answers().length === 0}>
          <button type="button" class="word" onClick={props.focusTerminal}>
            focus Terminal
          </button>
        </Show>
      </div>
    </section>
  );
};
