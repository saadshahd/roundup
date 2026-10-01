import { createSignal } from "solid-js";
import type { Accessor } from "solid-js";

export type Failure = {
  message: Accessor<string | null>;
  clear(): void;
  /** Clears the last failure, runs `work`, and keeps the message of an Error it throws; anything else is rethrown. */
  run(work: () => Promise<void>): Promise<void>;
};

export const createFailure = (): Failure => {
  const [message, setMessage] = createSignal<string | null>(null);

  return {
    message,
    clear: () => setMessage(null),
    run: async (work) => {
      setMessage(null);

      try {
        await work();
      } catch (thrown) {
        if (!(thrown instanceof Error)) throw thrown;

        setMessage(thrown.message);
      }
    },
  };
};
