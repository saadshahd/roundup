import { captureFocus } from "../keys/focus";
import { createSignal, For, onCleanup, onMount } from "solid-js";
import type { ChangedFile } from "@contracts/agent/ChangedFile";
import { copy } from "../copy";
import { ReservedLine } from "../ink/ReservedLine";
import { failureOf } from "../todos/failureOf";
import { useConnectedProject } from "../state/connectedProject";
import { changedPath as absolutePath, statusText } from "./paths";
import treeStyles from "./tree.styles.css?inline";

/** The library is fetched when the panel first has files to draw (rule 7). */
let library: Promise<typeof import("@pierre/trees")> | undefined;

const loadTrees = () => (library ??= import("@pierre/trees"));

/** Whether the library has been asked for yet. */
export const treesLoaded = (): boolean => library !== undefined;

/** U2539b, U2539d: the changed files as a tree of `@pierre/trees`; `Enter` or a double click on a file opens it in the editor at its line. */
export const ChangedTree = (props: { initialFocus?: ReturnType<typeof captureFocus>; root: string; files: readonly ChangedFile[]; selected?: string | undefined; onSelect?: (path: string) => void }) => {
  const { app } = useConnectedProject();
  const [failures, setFailures] = createSignal<ReadonlyMap<string, string>>(new Map());

  const clear = (path: string) => setFailures(previous => {
    const next = new Map(previous);

    next.delete(path);

    return next;
  });

  const byPath = new Map(props.files.map((file) => [file.path, file]));
  const [host, setHost] = createSignal<HTMLDivElement>();

  const choose = (path: string | null) => {
    const file = path === null ? undefined : byPath.get(path);

    if (!file || file.status === "deleted") return;

    void failureOf(() => app.openInEditor(absolutePath(props.root, file), file.line ?? undefined)).then(error => {
      if (error) setFailures(previous => new Map(previous).set(file.path, error));
      else clear(file.path);
    });
  };

  const initialFocus = props.initialFocus ?? captureFocus();
  onMount(() => {
    let tree: InstanceType<Awaited<ReturnType<typeof loadTrees>>["FileTree"]> | undefined;
    let gone = false;
    let stopScroll: (() => void) | undefined;
    let stopRows: (() => void) | undefined;
    let restoreFocus = props.selected !== undefined;

    void loadTrees().then(({ FileTree }) => {
      const mount = host();

      if (gone || !mount) return;

      tree = new FileTree({
        paths: props.files.map((file) => file.path),
        initialExpansion: "open",
        itemHeight: parseFloat(getComputedStyle(mount).getPropertyValue("--space-5")) || 24,
        stickyFolders: false,
        icons: { set: "minimal", colored: false },
        // The library defines its palette on :host, inside its shadow root.
        // Keep the overrides there too so inheritance cannot restore its defaults.
        unsafeCSS: treeStyles,
        gitStatus: props.files.map((file) => ({ path: file.path, status: file.status })),
        renderRowDecoration: ({ item }) => {
          const file = byPath.get(item.path);

          return file ? { text: statusText[file.status], title: file.from ? `${file.from} → ${file.path}` : file.path } : null;
        },
      });
      tree.render({ containerWrapper: mount });

      if (props.selected) tree.scrollToPath(props.selected);
      // Ark cannot discover tabbable rows inside Pierre’s shadow root; enter its roving row after render.
      const shadow = mount.querySelector("file-tree-container")?.shadowRoot;

      if (shadow) {
        // Virtualization replaces rows; restore accessible names on every rendered batch.
        const describeRows = () => {
          for (const row of shadow.querySelectorAll<HTMLElement>('[data-item-type="file"]')) {
            const file = byPath.get(row.dataset.itemPath ?? "");

            if (!file) continue;

            const detail = file.from ? `, ${file.from} → ${file.path}` : file.status === "deleted" ? `, ${copy.fileUnavailableInEditor}` : "";
            const label = `${file.path}, ${statusText[file.status]}${detail}`;

            if (row.getAttribute("aria-label") !== label) row.setAttribute("aria-label", label);
          }

          if (restoreFocus) {
            const selected = [...shadow.querySelectorAll<HTMLElement>("[data-item-path]")].find(row => row.dataset.itemPath === props.selected);

            if (selected) { restoreFocus = false; initialFocus(() => selected.focus()); }
          }

          mount.style.setProperty("--worktree-rows", String(tree?.getVisibleCount() ?? 0));
        };

        const observer = new MutationObserver(describeRows);

        observer.observe(shadow, { subtree: true, childList: true, attributes: true, attributeFilter: ["aria-label", "data-item-path"] });
        describeRows();
        const unsubscribe = tree.subscribe(describeRows);

        stopRows = () => { observer.disconnect(); unsubscribe(); };
      }

      if (!props.selected) initialFocus(() => shadow?.querySelector<HTMLElement>('[tabindex="0"]')?.focus());
      const scroller = shadow?.querySelector<HTMLElement>("[data-file-tree-virtualized-scroll]");

      if (scroller) {
        const measure = () => {
          mount.toggleAttribute("data-clipped-top", scroller.scrollTop > 0);
          mount.toggleAttribute("data-clipped-bottom", scroller.scrollTop + scroller.clientHeight < scroller.scrollHeight - 1);
        };

        const observer = new ResizeObserver(measure);

        observer.observe(scroller);

        for (const child of scroller.children) observer.observe(child);

        scroller.addEventListener("scroll", measure);
        measure();
        stopScroll = () => {
          observer.disconnect();
          scroller.removeEventListener("scroll", measure);
        };
      }
    });

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Enter") choose(tree?.getFocusedPath() ?? null);
    };

    const onSelect = (event: Event) => {
      const row = event.composedPath().find((target): target is HTMLElement => target instanceof HTMLElement && target.dataset.itemType === "file");

      if (row?.dataset.itemPath) props.onSelect?.(row.dataset.itemPath);
    };

    const onDoubleClick = (event: MouseEvent) => {
      const row = event.composedPath().find((target): target is HTMLElement => target instanceof HTMLElement && target.dataset.itemType === "file");

      choose(row?.dataset.itemPath ?? null);
    };

    const element = host();

    element?.addEventListener("click", onSelect);
    element?.addEventListener("focusin", onSelect);
    element?.addEventListener("keydown", onKeyDown);
    element?.addEventListener("dblclick", onDoubleClick);
    onCleanup(() => {
      gone = true;
      element?.removeEventListener("click", onSelect);
      element?.removeEventListener("focusin", onSelect);
      element?.removeEventListener("keydown", onKeyDown);
      element?.removeEventListener("dblclick", onDoubleClick);
      stopScroll?.();
      stopRows?.();
      tree?.cleanUp();
    });
  });

  return (
    <>
      <div ref={setHost} class="worktree-tree" />
      <For each={[...failures()]} fallback={<ReservedLine />}>{([path, error]) => <ReservedLine message={`${path.split("/").pop()}: ${error}`}><button type="button" data-button="quiet" onClick={() => clear(path)}>{copy.dismissFailure}</button></ReservedLine>}</For>
    </>
  );
};
