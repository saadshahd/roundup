import type { EmulatorFactory } from "../terminal/emulator";

export type SelectionSource = {
  /** The same factory the Pane builds its Terminals with, so each emulator can be asked what its user selected. */
  create: EmulatorFactory;
  /** The text selected in Terminal `id`'s output; empty when there is no such emulator or nothing is selected. */
  selectionOf(id: string | null): string;
};

/** U110 reads a Terminal's selection through the emulator the Pane already owns, so the Pane needs no change. */
export const trackSelections = (create: EmulatorFactory): SelectionSource => {
  const made = new Map<string, ReturnType<EmulatorFactory>>();

  return {
    create: (id) => {
      const emulator = create(id);

      made.set(id, emulator);

      return emulator;
    },
    selectionOf: (id) => (id === null ? "" : (made.get(id)?.selection() ?? "")),
  };
};
