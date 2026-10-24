/** Appearance focus belongs to its requesting action, never to a later reply or mount.
 * Focus choices invalidate it permanently, including a trip away and back to the same element.
 * `choose` invalidates older requests before an asynchronous destination has an element to focus.
 */
const revisions = new WeakMap<Document, { value: number; pending: Map<AbortController, ((target: EventTarget | null) => boolean) | undefined>; moving: boolean; native: HTMLElement | null }>();

export const captureFocus = (choose = false): ((move: () => void) => boolean) & { signal: (contains?: (target: EventTarget | null) => boolean) => AbortSignal; target: (destination: () => HTMLElement | null | undefined) => HTMLElement } => {
  let revision = revisions.get(document);

  if (!revision) {
    revision = { value: 0, pending: new Map(), moving: false, native: null };
    revisions.set(document, revision);
    const current = revision;

    const invalidate = (event: Event) => {
      if (event.type === "focusin" && (current.moving || current.native === event.target)) {
        current.native = null;

        return;
      }

      current.native = null;
      current.value++;

      for (const [pending, contains] of current.pending) {
        if (contains?.(event.target)) continue;
        current.pending.delete(pending);
        pending.abort();
      }
    };

    document.addEventListener("focusin", invalidate, true);
    document.addEventListener("pointerdown", invalidate, true);
  }

  if (choose) {
    revision.native = null;
    revision.value++;

    for (const pending of revision.pending.keys()) { revision.pending.delete(pending); pending.abort(); }
  }

  const captured = revision.value;
  let used = false;
  let cancellation: AbortController | undefined;

  const focus = (move: () => void): boolean => {
    if (used) return false;
    used = true;

    if (revision.value !== captured) return false;
    const previous = revision.moving;
    revision.moving = true;

    try { move(); } finally { revision.moving = previous; }

    return true;
  };

  // Library focus callbacks cannot all be cancelled directly. Their owner can dismiss the stale
  // appearance through its normal library control before that callback finds a connected input.
  return Object.assign(focus, {
    target: (destination: () => HTMLElement | null | undefined) => {
      let target = document.activeElement instanceof HTMLElement ? document.activeElement : document.body;
      focus(() => {
        target = destination() ?? target;

        if (target !== document.activeElement) {
          revision.native = target;
          queueMicrotask(() => { if (revision.native === target) revision.native = null; });
        }
      });

      return target;
    },
    signal: (contains?: (target: EventTarget | null) => boolean) => {
      if (!cancellation) {
        cancellation = new AbortController();

        if (revision.value !== captured) cancellation.abort();
        else revision.pending.set(cancellation, contains);
      }

      return cancellation.signal;
    },
  });
};

/** A mount/return callback keeps the permission it had when queued. */
export const queueFocus = (move: () => void): void => {
  const focus = captureFocus();

  queueMicrotask(() => focus(move));
};
