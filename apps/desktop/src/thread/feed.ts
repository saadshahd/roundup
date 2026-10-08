import { createSignal, onCleanup } from "solid-js";
import type { Accessor } from "solid-js";
import type { Message } from "@contracts/message/Message";
import type { Events } from "../app/events";
import type { AppSeam } from "../app/seam";

export type MessageFeed = {
  /** Every Message the Daemon has told this webview of, in id order; a status event replaces the Message it names. */
  messages: Accessor<readonly Message[]>;
  /** The message of the last failed `message.list`, until a later list succeeds. */
  failure: Accessor<string | null>;
};

/** Subscribes before it lists, so a Message sent in between is in the list either way; one id keeps one entry. */
export const createMessageFeed = (app: AppSeam, events: Events): MessageFeed => {
  const [byId, setById] = createSignal<ReadonlyMap<number, Message>>(new Map());
  const [failure, setFailure] = createSignal<string | null>(null);

  const keep = (message: Message): void => {
    setById((known) => new Map(known).set(message.id, message));
  };

  const unsubscribe = events.subscribe((event) => {
    if (
      event.name === "message.sent" ||
      event.name === "message.held" ||
      event.name === "message.delivered" ||
      event.name === "message.dropped"
    ) {
      keep(event.data);
    }
  });

  onCleanup(unsubscribe);

  app.rpc("message.list", { to: null, status: null }).then(
    (listed) => {
      setFailure(null);
      setById((known) => {
        const next = new Map(known);

        // An event is newer than the list that raced it.
        for (const message of listed) if (!next.has(message.id)) next.set(message.id, message);

        return next;
      });
    },
    (error) => {
      if (!(error instanceof Error)) throw error;

      setFailure(error.message);
    },
  );

  return { messages: () => [...byId().values()].toSorted((a, b) => a.id - b.id), failure };
};
