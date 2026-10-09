/** `reopen` after the Daemon's exit text (U37): asks the App for a new Daemon for the same Project; off while one is starting. */
export const Reopen = (props: { onReopen: () => void; busy: boolean }) => (
  <>
    {"   "}
    <button class="word" disabled={props.busy} aria-disabled={props.busy ? true : undefined} onClick={props.onReopen}>
      reopen
    </button>
  </>
);
