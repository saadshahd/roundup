import { captureFocus } from "../keys/focus";
import { writeText } from "../app/clipboard";
import { createTerminalClose } from "./close";
import { ReservedLine } from "../ink/ReservedLine";
import { CHORDS, LEADER } from "../keys/chords";
import { registerKeys } from "../keys/registry";
import * as v from "valibot";
import { Dialog } from "../ui/Dialog";
import { Editable } from "../ui/Editable";
import { FlatAdd, FlatTab } from "../ui/FlatTab";
import { IconButton } from "../ui/IconButton";
import { contextMenu, linkEntries, openOnRelease, textTarget, PULL_DISTANCE } from "../ui/contextMenu";
import { Bubble } from "./Bubble";
import { copy } from "../copy";
import { createEffect, createMemo, createSignal, For, on, onCleanup, onMount, Show, untrack } from "solid-js";
import { Portal } from "solid-js/web";
import type { TerminalInfo } from "@contracts/terminal/TerminalInfo";
import { Icon } from "../ink/Icon";
import { createCut } from "../ink/Tip";
import { Tip } from "../ui/Tip";
import { isWorkstream, resolvedWorkstream, siblingsOf } from "../rail/layout";
import { useConnectedProject } from "../state/connectedProject";
import type { Screens } from "../terminal/screens";
import {
  clamped, defaultPlace, placesStorage, REVEAL_BUBBLE, shownFor, shownName,
} from "./places";
import type { Place, Extent } from "./places";
import "./styles.css";

/** U421 engineering call 1: past the 3 to 4 px a hand wobbles while it clicks. */

const FALLBACK_EXTENT: Extent = { width: 800, height: 600 };

const focusPane = (): void => {
  const target = document.querySelector<HTMLElement>(".pane-screen textarea, .pane-screen [tabindex]");

  target?.focus();

  if (document.activeElement !== target) document.querySelector<HTMLElement>(".terminal-new")?.focus();
};

const ESCAPE = "Escape".split(" ");

/** U469: the selected Terminals, their Bubble geometry, bottom tabs and opening/cycling chords. */
export const Bubbles = (props: {
  screens: Screens;
  /** Open only this Project's original, non-pending revision in a shown Terminal tab.
   * Refuse foreign/unavailable work; restore never sends or retries storage.
   */
  recovery?: { project: string; revision: string } | undefined;
}) => {
  const connected = useConnectedProject();
  const { rail, app } = connected;
  const path = connected.project.path;
  const store = placesStorage(path);
  const drafts = connected.drafts;
  const schema = v.strictObject({ name: v.string(), base: v.string(), recovery: v.optional(v.literal(true)) });
  drafts.validate("bubbles", schema);
  drafts.registerRecovery("bubbles", (draft, facts) => {
    return facts.daemonExit() !== null ? "unavailable" : facts.rail.failure() ? "unverified"
      : facts.rail.terminals().some(terminal => terminal.incarnation === draft.destination.object && terminal.id === draft.destination.recipient && terminal.running) ? "available" : "unavailable";
  });
  const records = createMemo(() => drafts.list().filter(draft => draft.destination.surface === "bubbles"));
  const [recovering, recover] = createSignal(false);
  const [inspected, inspect] = createSignal<string>();
  const [copyFailure, failCopy] = createSignal<{ revision: string; message: string } | null>(null);
  createEffect(() => {
    const listed = records();
    const prior = untrack(copyFailure);

    if (prior && !listed.some(draft => draft.revision === prior.revision)) failCopy(null);
  });
  const shownDraft = () => records().find(draft => draft.revision === inspected()) ?? records().filter(draft => draft.status === "draft" && !draft.value.recovery).at(-1) ?? records().at(-1);

  const problem = () => {
    const failure = drafts.problem();

    return failure?.destination && failure.destination.surface !== "bubbles" ? null : failure;
  };

  const original = (draft: ReturnType<typeof drafts.list>[number]) => rail.terminals().find(terminal => terminal.incarnation === draft.destination.object && terminal.id === draft.destination.recipient && terminal.running);

  const copyName = async () => {
    const draft = shownDraft();
    const parsed = draft && v.safeParse(schema, draft.value);

    if (!parsed?.success) return;

    try {
      await writeText(connected.app, parsed.output.name);
      failCopy(prior => prior?.revision === draft!.revision ? null : prior);
    } catch (error) {
      failCopy({ revision: draft!.revision, message: copy.copyDraftFailure(error instanceof Error ? error.message : String(error)) });
    }
  };

  let recoveryPanel: HTMLDivElement | undefined;
  registerKeys({ kind: "layer", element: () => recoveryPanel, active: recovering, modal: true }, []);

  const refreshPlaces = (event: StorageEvent) => {
    if (event.storageArea === window.sessionStorage && event.key === `roundup:bubble-places:${path}`) setPlaces(store.read());
  };

  window.addEventListener("storage", refreshPlaces);
  onCleanup(() => window.removeEventListener("storage", refreshPlaces));

  const persistPlaces = (value: Record<string, Place>): void => {
    if (connected.daemonExit() === null) store.write(value);
  };

  const [places, setPlaces] = createSignal<Record<string, Place>>(store.read());
  const [failure, setFailure] = createSignal<{ message: string; terminal: { id: string; incarnation: string } | null } | null>(null);

  const clearItemFailure = (terminal: { id: string; incarnation: string }) => {
    setFailure(prior => prior?.terminal?.id === terminal.id && prior.terminal.incarnation === terminal.incarnation ? null : prior);
  };

  createEffect(() => {
    const terminals = rail.terminals();

    if (connected.daemonExit() !== null) return;
    const prior = untrack(failure);

    if (prior?.terminal && !terminals.some(terminal => terminal.id === prior.terminal!.id && terminal.incarnation === prior.terminal!.incarnation)) setFailure(null);
  });
  const [opening, setOpening] = createSignal(false);
  const [wanted, setWanted] = createSignal<{ id: string; focus: ReturnType<typeof captureFocus> } | null>(null);
  const [layer, setLayer] = createSignal<HTMLElement>();
  const [bar, setBar] = createSignal<HTMLElement>();
  const [measured, setMeasured] = createSignal(false);
  const [origin, setOrigin] = createSignal({ left: 0, top: 0, width: 0, height: 0 });
  const [editing, setEditing] = createSignal<{ id: string } | null>(null);
  createEffect(on(() => rail.selected(), () => setEditing(null), { defer: true }));
  onMount(() => setBar(layer()?.parentElement?.querySelector<HTMLElement>(".resource-strip") ?? undefined));
  let before: Element | null = null;

  const extent = (): Extent => {
    const box = layer()?.getBoundingClientRect();

    return box && box.width > 0 && box.height > 0 ? { width: box.width, height: box.height } : FALLBACK_EXTENT;
  };

  // U524: the Terminal opened from the Project root stays shown while the selection stays there.
  const [fromRoot, setFromRoot] = createSignal<string | null>(null);

  const shown = createMemo(() => rail.selected() === null
    ? rail.terminals().filter((terminal) => terminal.owner === path || terminal.id === fromRoot())
    : shownFor(rail.terminals(), rail.nodes, rail.selected()));

  // U524: the pane placeholder gives way while a Terminal opened from the root is shown.
  createEffect(() => {
    const stack = layer()?.parentElement;

    if (!stack) return;

    stack.toggleAttribute("data-root-terminal", rail.selected() === null && shown().length > 0);
    onCleanup(() => stack.removeAttribute("data-root-terminal"));
  });

  const placeOf = (id: string): Place => places()[id] ?? defaultPlace(extent());

  // A bubble's default place is stored when it first shows, so hiding it, moving another or resizing the window never shifts it (U164).
  // U2224: selecting an Agent row folds every bubble to its chip, so none covers the output it selects.
  let lastSelected: string | null | undefined;

  createEffect(() => {
    // Save a fresh default only after the layer excludes the resource strip; an early rectangle would persist the wrong inset.
    if (!measured()) return;

    const eligible = shown();
    const chosen = rail.selected();
    const folds = lastSelected !== undefined && chosen !== lastSelected && rail.nodes.some((candidate) => candidate.id === chosen && candidate.kind === "agent");

    lastSelected = chosen;

    const prior = untrack(places);
    const missing = eligible.filter((terminal) => prior[terminal.id] === undefined);
    const visible = eligible.filter((terminal) => prior[terminal.id] && !prior[terminal.id]!.min);
    const next = { ...prior };
    const latest = folds ? undefined : visible.at(-1)?.id ?? missing.at(-1)?.id;

    for (const terminal of missing) next[terminal.id] = { ...defaultPlace(untrack(extent)), min: opening() || terminal.id !== latest };

    for (const terminal of visible) next[terminal.id] = { ...next[terminal.id]!, min: terminal.id !== latest };

    if (folds) for (const terminal of eligible) next[terminal.id] = { ...next[terminal.id]!, min: true };

    if (JSON.stringify(next) === JSON.stringify(prior)) return;
    setPlaces(next);
    persistPlaces(next);
  });

  const setPlace = (id: string, place: Place): void => {
    const next = { ...places(), [id]: place };

    setPlaces(next);
    persistPlaces(next);
  };

  const nameFor = (terminal: TerminalInfo): string => shownName(terminal, places()[terminal.id]);

  /** U511: a trimmed name that differs from the shown one is saved on the Terminal's place; an empty one clears it. */
  const rename = (terminal: TerminalInfo, value: string): string | null => {
    const name = value.trim();

    if (name === nameFor(terminal) || (name === "" && places()[terminal.id]?.name === undefined)) return null;

    const { name: _previous, ...place } = placeOf(terminal.id);

    const next = { ...places(), [terminal.id]: name === "" ? place : { ...place, name } };
    const failure = store.write(next);

    if (failure === null) setPlaces(next);

    return failure;
  };

  const show = (id: string | null): void => {
    if (id !== null && !shown().some((terminal) => terminal.id === id)) return;

    const next = { ...places() };

    for (const terminal of shown()) next[terminal.id] = { ...placeOf(terminal.id), min: terminal.id !== id };
    setPlaces(next);
    persistPlaces(next);
  };

  const openRecovery = () => {
    show(null);
    // Let the Terminal's dismissable layer close before the recovery layer takes focus.
    const focus = captureFocus(true);
    setTimeout(() => focus(() => recover(true)));
  };

  // Saved places end with the Daemon; its last readable screens keep their current geometry and visibility until the view ends.
  createEffect(() => {
    if (connected.daemonExit() !== null) {
      store.clear();

      return;
    }

    const listed = new Set(rail.terminals().map((terminal) => terminal.id));
    const prior = untrack(places);
    const kept = Object.fromEntries(Object.entries(prior).filter(([id]) => listed.has(id)));

    if (Object.keys(kept).length !== Object.keys(prior).length) {
      setPlaces(kept);
      persistPlaces(kept);
    }
  });

  // The layer shrinks when the Shelf opens or the window narrows: every place is kept inside it (U164).
  createEffect(() => {
    const host = layer();

    if (!host || typeof ResizeObserver === "undefined") return;

    // U469: the layer ends above the bar.
    const sync = (): void => {
      const stack = host.parentElement?.getBoundingClientRect();
      const composer = host.parentElement?.querySelector(".resource-strip")?.getBoundingClientRect();
      host.style.bottom = stack && composer && composer.height > 0 ? `${Math.max(0, stack.bottom - composer.top)}px` : "";

      const corner = host.getBoundingClientRect();

      setOrigin((prior) => (prior.left === corner.left && prior.top === corner.top && prior.width === corner.width && prior.height === corner.height ? prior : { left: corner.left, top: corner.top, width: corner.width, height: corner.height }));
      setMeasured(true);
    };

    const watched = new ResizeObserver(() => {
      sync();

      const prior = untrack(places);
      const next = Object.fromEntries(Object.entries(prior).map(([id, place]) => [id, clamped(place, extent())]));

      if (JSON.stringify(next) === JSON.stringify(prior)) return;

      setPlaces(next);
      persistPlaces(next);
    });

    watched.observe(host);
    window.addEventListener("resize", sync);
    onCleanup(() => window.removeEventListener("resize", sync));

    if (host.parentElement) watched.observe(host.parentElement);

    // The composer comes and goes with the selection, which moves no size the observer sees.
    const arrivals = new MutationObserver((records) => {
      if (records.some((record) => !host.contains(record.target))) sync();
    });

    if (host.parentElement) arrivals.observe(host.parentElement, { childList: true, subtree: true });
    sync();
    onCleanup(() => {
      watched.disconnect();
      arrivals.disconnect();
    });
  });

  const readOnly = () => connected.daemonExit() !== null;

  const { close, closing, Failure: CloseFailure } = createTerminalClose(connected, shown, nameFor, (message, terminal) => setFailure(message === null ? null : { message, terminal: terminal ?? null }), clearItemFailure);

  const bubbleElement = (id: string): HTMLElement | null => document.querySelector<HTMLElement>(`[data-bubble="${id}"]`);

  const focusItem = (id: string): void => {
    const place = places()[id];

    if (place?.min) document.querySelector<HTMLElement>(`.terminal-tab[data-bubble="${id}"]`)?.focus();
    else props.screens.emulatorFor(id).focus();
  };

  const focusedId = (active: Element | null = document.activeElement): string | null => {
    return active instanceof Element ? (active.closest<HTMLElement>("[data-bubble]")?.dataset.bubble ?? null) : null;
  };

  const toggleMinimised = (id: string, wasMin = placeOf(id).min): void => {
    const focus = captureFocus(true);
    const place = { ...placeOf(id), min: wasMin };

    if (place.min) show(id);
    else setPlace(id, { ...place, min: true });
    queueMicrotask(() => focus(() => focusItem(id)));
  };

  const closeFocused = (focus: Element | null): void => {
    const id = focusedId(focus);

    if (id === null) return;

    const focusReturn = captureFocus();
    const back = before;
    const fromTab = focus?.closest(".terminal-tabs") !== null;

    void close(id).then(() => {
      if (failure() !== null || fromTab || shown().some(terminal => terminal.id === id)) return;

      focusReturn(() => {
        if (back instanceof HTMLElement && back.isConnected) back.focus();
        else focusPane();
      });
    });
  };

  const terminalOwner = () => resolvedWorkstream(rail.nodes, rail.selected()) ?? siblingsOf(rail.nodes, null).find(isWorkstream)?.id ?? null;

  /** The id `terminal.open` answered, else null when there was nothing to open or the call failed. */
  const openTerminal = async (): Promise<string | null> => {
    // U524: a selection that resolves to no Workstream (the Project root) opens in the first Workstream in Rail order.
    const workstream = terminalOwner();

    if (opening() || readOnly()) return null;

    const focus = captureFocus(true);
    const root = rail.selected() === null;
    setOpening(true);

    try {
      const opened = await app.rpc("terminal.open", { workstream });

      setFromRoot(root ? opened.terminal_id : null);
      setWanted({ id: opened.terminal_id, focus });
      setFailure(prior => prior?.terminal === null ? null : prior);
      await rail.refresh({ terminals: true });


      return opened.terminal_id;
    } catch (thrown) {
      if (!(thrown instanceof Error)) throw thrown;

      setFailure({ message: thrown.message, terminal: null });

      return null;
    } finally {
      setOpening(false);

      if (failure() !== null && shown().length === 0) focus(() => document.querySelector<HTMLElement>(".terminal-new")?.focus({ preventScroll: true }));
    }
  };

  const open = async (): Promise<void> => {
    await openTerminal();
  };

  // Pull and ⌘B share the last eligible focus, falling back to the latest started Terminal.
  let lastFocused: string | null = null;

  const lastTarget = (): string | null => {
    const ids = shown().filter((terminal) => !closing().has(terminal.id)).map((terminal) => terminal.id);

    return lastFocused !== null && ids.includes(lastFocused) ? lastFocused : (ids.at(-1) ?? null);
  };

  /** Holds the middle of `id`'s header under the pointer, kept inside the pane by `clamped`. */
  const holdAt = (id: string, pointer: { x: number; y: number }): void => {
    const box = layer()?.getBoundingClientRect();
    const header = bubbleElement(id)?.querySelector<HTMLElement>(".bubble-header")?.offsetHeight ?? 0;
    const place = placeOf(id);

    show(id);
    setPlace(id, clamped({ ...place, min: false, x: pointer.x - (box?.left ?? 0) - place.w / 2, y: pointer.y - (box?.top ?? 0) - header / 2 }, extent()));
  };

  /** U421: a right-button drag of `PULL_DISTANCE` pulls the selected Workstream's Terminal out under the pointer. */
  const pulling = () => {
    let press: { x: number; y: number } | null = null;
    let active = false;
    let pointer = { x: 0, y: 0 };
    let id: string | null = null;
    let frame: number | null = null;
    let swallowMenu = false;
    let swallowUp = false;

    const apply = (): void => {
      frame = null;

      if (id !== null) holdAt(id, pointer);
    };

    const schedule = (): void => {
      frame ??= requestAnimationFrame(apply);
    };

    const end = (): void => {
      if (frame !== null) {
        cancelAnimationFrame(frame);
        apply();
      }

      press = null;
      active = false;
    };

    const blocked = (): boolean =>
      resolvedWorkstream(rail.nodes, rail.selected()) === null || opening() || readOnly() || document.querySelector('[role="menu"], .switcher') !== null;

    const start = (): void => {
      active = true;

      const target = lastTarget();

      if (target !== null) {
        id = target;
        holdAt(target, pointer);

        return;
      }

      id = null;
      void openTerminal().then((opened) => {
        if (opened === null) return;

        // The pointer may have moved or let go while the call was in flight: the bubble goes to where it is now.
        requestAnimationFrame(() => holdAt(opened, pointer));
      });
    };

    const down = (event: PointerEvent): void => {
      swallowMenu = false;
      swallowUp = false;
      press = null;

      if (event.pointerType !== "mouse" || event.button !== 2 || event.metaKey || event.ctrlKey || event.altKey || event.shiftKey || textTarget(event.target) || blocked()) return;

      press = { x: event.clientX, y: event.clientY };
      pointer = press;
    };

    const move = (event: PointerEvent): void => {
      if (press === null) return;

      pointer = { x: event.clientX, y: event.clientY };

      if (!active) {
        if ((event.buttons & 2) === 0 || Math.hypot(pointer.x - press.x, pointer.y - press.y) < PULL_DISTANCE) return;

        start();
      }

      // A Pull crossing a Terminal keeps its move and release out of that program.
      event.stopPropagation();

      if (id !== null) schedule();
    };

    const up = (event: PointerEvent): void => {
      if (press === null) return;

      if (active) {
        event.stopPropagation();
        swallowMenu = true;
        swallowUp = true;
      }

      end();
    };

    const swallow = (event: Event): void => {
      if (active) event.stopPropagation();
    };

    const swallowRelease = (event: Event): void => {
      if (!active && !swallowUp) return;

      swallowUp = false;
      event.stopPropagation();
    };

    const menu = (event: Event): void => {
      if (!active && !swallowMenu) return;

      swallowMenu = false;
      event.preventDefault();
      event.stopPropagation();
    };

    const listen = <K extends keyof WindowEventMap>(name: K, listener: (event: WindowEventMap[K]) => void): void => {
      window.addEventListener(name, listener, true);
      onCleanup(() => window.removeEventListener(name, listener, true));
    };

    listen("pointerdown", down);
    listen("pointermove", move);
    listen("pointerup", up);
    listen("pointercancel", end);
    listen("mousemove", swallow);
    listen("mouseup", swallowRelease);
    listen("contextmenu", menu);
    registerKeys({ kind: "app" }, [{
      chords: ESCAPE,
      when: () => active,
      run: () => {
        swallowMenu = true;
        end();
      },
    }]);
  };

  pulling();

  // The bubble the user opened takes the keys, whether its Terminal is listed before or after `terminal.open` answers.
  createEffect(() => {
    const request = wanted();

    if (request === null || !shown().some((terminal) => terminal.id === request.id)) return;
    const { id, focus } = request;

    queueMicrotask(() => {
      if (wanted() !== request || bubbleElement(id) === null) return;

      if (!focus(() => { show(id); focusItem(id); })) setPlace(id, { ...placeOf(id), min: true });

      if (wanted() === request) setWanted(null);
    });
  });

  // U511: the name's editor owns its keys. Help and the Switcher are modal layers, so no chord acts under them (U54); a pressed key repeats nothing.
  const outsideEditor = (focus: EventTarget | null): boolean => !(focus instanceof Element && focus.closest(".terminal-tab-editor"));

  registerKeys({ kind: "app" }, [
    { action: CHORDS.find((row) => row.keys === "⌘T")!,
      when: (press) => outsideEditor(press.target),
      capture: (focus) => {
        const owner = terminalOwner();

        return {
          available: () => outsideEditor(focus) && !opening() && !readOnly() && terminalOwner() === owner,
          invoke: () => void open(),
        };
      },
    },
  ]);

  const mainTerminal = (): string | null => {
    const id = rail.nodes.find((node) => node.id === rail.selected())?.terminal_id ?? null;

    return id !== null && document.querySelector(".pane-screen textarea, .pane-screen [tabindex]") ? id : null;
  };

  const hasTerminalFocus = (focus: Element | null): boolean => focus?.isConnected === true && focus.closest(".bubble, button.terminal-tab, .pane-screen") !== null;

  const switchPair = (focus: Element | null) => {
    const selected = rail.selected();
    const main = mainTerminal();
    const fromBubble = focusedId(focus);
    const target = fromBubble ?? lastTarget();
    const inTerminal = hasTerminalFocus(focus);
    const toAgent = fromBubble !== null && focus?.closest(".bubble") !== null;

    return {
      available: () => focus?.isConnected === true && inTerminal && !readOnly() && main !== null && mainTerminal() === main
        && rail.selected() === selected && target !== null && shown().some((terminal) => terminal.id === target) && !closing().has(target),
      invoke: () => {
        if (toAgent) {
          show(null);
          props.screens.emulatorFor(main!).focus();
        } else {
          const permission = captureFocus(true);
          show(target);
          // The manual popover becomes focusable when Solid commits this turn's visibility change.
          queueMicrotask(() => permission(() => {
            if (focus?.isConnected && !readOnly() && rail.selected() === selected && mainTerminal() === main && shown().some((terminal) => terminal.id === target) && !closing().has(target!) && !placeOf(target!).min) props.screens.emulatorFor(target!).focus();
          }));
        }
      },
    };
  };

  registerKeys({ kind: "app" }, [{
    action: CHORDS.find((row) => row.keys === "⌘B")!,
    when: () => hasTerminalFocus(document.activeElement),
    capture: switchPair,
  }]);

  const hasFocusedBubble = (focus: Element | null): boolean => {
    const id = focusedId(focus);

    return focus?.isConnected === true && id !== null && shown().some((terminal) => terminal.id === id) && !readOnly() && !closing().has(id);
  };

  registerKeys({ kind: "leader" }, [
    { action: LEADER.find((row) => row.keys === "M")!, capture: (focus) => {
      const id = focusedId(focus);
      const wasMin = id !== null && placeOf(id).min;

      return { available: () => hasFocusedBubble(focus), invoke: () => toggleMinimised(id!, wasMin) };
    } },
    { action: LEADER.find((row) => row.keys === "W")!, capture: (focus) => ({ available: () => hasFocusedBubble(focus), invoke: () => closeFocused(focus) }) },
  ]);

  const onFocusIn = (event: FocusEvent): void => {
    if (!(event.target instanceof Element) || event.target.closest("[data-bubble]")) return;

    before = event.target;
  };

  const onBubbleFocus = (event: FocusEvent): void => {
    if (!(event.target instanceof Element)) return;

    const id = event.target.closest<HTMLElement>("[data-bubble]")?.dataset.bubble;

    if (id !== undefined) lastFocused = id;
  };

  // Focus landing on a control outside the bubbles (⌘K's Switcher, ⌘/'s Ask bar) minimises every open bubble, so the focused control is never under the top layer, whether or not focus began inside a bubble.
  const onFocusLeave = (event: FocusEvent): void => {
    const { target } = event;

    if (!(target instanceof Element) || target.closest("[data-bubble], .bubble-frame, .terminal-tab-pair, .terminal-recover, [role=menu], [role=menuitem]")) return;

    for (const { id } of shown()) if (!placeOf(id).min) setPlace(id, { ...placeOf(id), min: true });
  };

  // U529: the Switcher chose a Terminal; its owner is selected by now, so the bubble is listed.
  const onReveal = (event: Event): void => {
    const id = event instanceof CustomEvent ? String(event.detail) : null;

    if (id === null || !shown().some((terminal) => terminal.id === id)) return;

    const focus = captureFocus(true);
    show(id);
    queueMicrotask(() => focus(() => focusItem(id)));
  };

  window.addEventListener(REVEAL_BUBBLE, onReveal);
  onCleanup(() => window.removeEventListener(REVEAL_BUBBLE, onReveal));
  document.addEventListener("focusin", onFocusIn);
  document.addEventListener("focusin", onBubbleFocus);
  document.addEventListener("focusin", onFocusLeave);
  onCleanup(() => {
    document.removeEventListener("focusin", onFocusLeave);
    document.removeEventListener("focusin", onBubbleFocus);
    document.removeEventListener("focusin", onFocusIn);
  });

  // Bubbles are keyed by id: a refreshed `terminal.list` brings new objects for the same Terminals, and a remount would drop the emulator's focus.
  const shownIds = createMemo(() => shown().map((terminal) => terminal.id), [], { equals: (a, b) => a.length === b.length && a.every((id, at) => id === b[at]) });

  const Tab = (tabProps: { id: string }) => {
    const id = tabProps.id;
    const terminal = () => shown().find((candidate) => candidate.id === id)!;

    const name = () => nameFor(terminal());
    const [nameRef, nameCut] = createCut();
    let destination = Object.freeze({ surface: "bubbles", object: terminal().incarnation, recipient: id });
    createEffect(() => { if (editing()?.id === id) onCleanup(drafts.editor(destination)); });
    const current = () => records().filter(draft => draft.destination.object === terminal().incarnation && draft.destination.recipient === id && draft.status === "draft" && !draft.value.recovery).at(-1);
    let base = "";
    let initial = "";
    let revision: string | undefined;
    let selection: ReturnType<typeof drafts.list>[number]["selection"] = null;
    let restored: "none" | "current" | "selected" | "branch" = "none";
    const [changed, change] = createSignal(false);
    const requiresConfirmation = () => restored === "selected" || restored === "branch";

    const start = (selected?: { project: string; revision: string }) => {
      const target = Object.freeze({ surface: "bubbles", object: terminal().incarnation, recipient: id });

      const retained = selected ? records().find(draft => draft.revision === selected.revision)
        : current() ?? records().filter(draft => draft.destination.object === target.object && draft.destination.recipient === id && draft.status === "failed").at(-1);

      const parsed = retained && v.safeParse(schema, retained.value);

      if (selected && (selected.project !== path || !retained || retained.status === "pending" || !parsed?.success
        || retained.destination.object !== target.object || retained.destination.recipient !== id
        || !terminal().running || readOnly())) return;

      if (!terminal().running || readOnly() || (retained && !parsed?.success)) {
        openRecovery();

        return;
      }

      destination = target;
      restored = retained?.value.recovery === true || (selected && retained?.revision !== current()?.revision) ? "branch"
        : selected ? "selected" : retained ? "current" : "none";
      initial = parsed?.success ? parsed.output.name : name();
      base = parsed?.success ? parsed.output.base : name();
      revision = retained?.revision;
      selection = retained?.selection ?? null;
      change(false);
      setEditing({ id });
    };

    const capture = (input: HTMLInputElement) => {
      const caret = { start: input.selectionStart ?? 0, end: input.selectionEnd ?? 0, direction: input.selectionDirection ?? "none" };
      const previous = records().find(draft => draft.revision === revision);

      if (previous?.value.name === input.value) {
        if (JSON.stringify(previous.selection) !== JSON.stringify(caret)) drafts.setSelection(previous.revision, caret);

        return;
      }

      // A selected edit is a separate recovery branch; the ordinary current draft stays current.
      const draftValue = restored === "branch" ? { name: input.value, base, recovery: true } : { name: input.value, base };
      revision = drafts.edit(destination, draftValue, caret, "ordinary", revision ?? null).draft.revision;
    };

    createEffect(on(() => props.recovery, selected => { if (selected) start(selected); }));

    createEffect(() => {
      if (terminal().incarnation !== destination.object || !terminal().running || readOnly()) setEditing(prior => prior?.id === id ? null : prior);
    });

    const tab = () => document.querySelector<HTMLElement>(`button.terminal-tab[data-bubble="${id}"]`);

    /** U2694b: Rename, Send to Agent ▸, Copy Link and Close, as the system menu of the tab. */
    const tabMenu = (x: number, y: number) => {
      const agents = rail.nodes;
      const target = terminal();

      return contextMenu(app, [
        { id: "rename", label: copy.rename, run: start },
        ...linkEntries(app, { kind: "terminal", id: id }, agents, (node) => rail.exitOf(node) === null, message => setFailure({ message, terminal: target })),
        { id: "close", label: copy.close, shortcut: lastTarget() === id ? copy.closeChord : undefined, run: () => close(id) },
      ], x, y);
    };

    return <>
      <FlatTab class="terminal-tab-pair" onContextMenu={(event: MouseEvent) => openOnRelease(event, (x, y) => void tabMenu(x, y))} selected={!placeOf(id).min} data-bubble={id}>
        <Show when={editing()?.id === id ? editing() : null} keyed fallback={
          <Tip content={name()} cut={nameCut}>
            {(trigger) => (
              <button {...trigger()} type="button" data-button="quiet" data-reads={!placeOf(id).min ? "text" : undefined} class="terminal-tab" data-bubble={id}
                aria-label={name()} aria-describedby={props.screens.unseen(id) ? `terminal-state-${id}` : undefined} aria-pressed={!placeOf(id).min}
                onClick={() => toggleMinimised(id)}
                onDblClick={(event) => { if (event.target instanceof Element && event.target.closest(".terminal-tab-name")) start(); }}>
                <span class="terminal-tab-name" ref={nameRef}>{name()}</span>
                <span class="terminal-tab-slot"><Show when={props.screens.unseen(id)}><span id={`terminal-state-${id}`} class="terminal-tab-unseen" data-state="" role="img" aria-label={copy.unseenOutput}>{copy.unseenMark}</span></Show></span>
              </button>
            )}
          </Tip>
        }>{_edit =>
          <div class="terminal-tab terminal-tab-editor" data-bubble={id} data-pressed={!placeOf(id).min}>
            <Editable.Root selectOnFocus={false} defaultValue={initial} activationMode="none" submitMode={requiresConfirmation() || (restored === "current" && !changed()) ? "enter" : "both"} finalFocusEl={tab}
              onValueCommit={({ value }) => {
                const submitted = revision;
                const live = rail.terminals().find(candidate => candidate.incarnation === destination.object && candidate.id === id && candidate.running);

                if (!live || readOnly() || base !== nameFor(live)) {
                  openRecovery();

                  return;
                }

                if (submitted && !drafts.submit(submitted)) {
                  openRecovery();

                  return;
                }

                const failure = rename(live, value);

                if (submitted) drafts.settle(submitted, failure === null ? "acknowledged" : "failed");

                if (failure === null) clearItemFailure(live);
                else setFailure({ message: failure, terminal: live });

                if (failure !== null) openRecovery();
              }}
              onEditChange={({ edit }) => { if (!edit) setEditing(null); }}>
              <Editable.Context>{(api) => {
                onMount(() => api().edit());

                return null;
              }}</Editable.Context>
              <Editable.Area><Editable.Preview /><Editable.Input aria-label={copy.terminalName}
                onBlur={() => { if (requiresConfirmation()) setEditing(null); }}
                onInput={event => { change(true); capture(event.currentTarget); }}
                onSelect={event => { if (document.activeElement === event.currentTarget && revision) capture(event.currentTarget); }} ref={(input: HTMLInputElement) => {
                // Chromium's select() leaves focus where it was; the editor takes it, with the whole name selected, after the library's own step.
                const permission = captureFocus();
                onMount(() => requestAnimationFrame(() => permission(() => { if (!input.isConnected) return;
                  input.focus({ preventScroll: true });

                  if (selection) input.setSelectionRange(selection.start, selection.end, selection.direction);
                  else input.select(); })));
              }} /></Editable.Area>
            </Editable.Root>
            <span class="terminal-tab-slot"><Show when={props.screens.unseen(id)}><span id={`terminal-state-${id}`} class="terminal-tab-unseen" data-state="" role="img" aria-label={copy.unseenOutput}>{copy.unseenMark}</span></Show></span>
          </div>}
        </Show>
        <IconButton aria-label={copy.closeTerminal(name())} onClick={() => void close(id)}><Icon name="x" /></IconButton>
      </FlatTab>
    </>;
  };

  const recoveryNotice = createMemo(() => shownDraft() ? !original(shownDraft()!) || readOnly() ? copy.originalUnavailable : shownDraft()!.value.base !== nameFor(original(shownDraft()!)!) ? copy.railDraftConflict : null : null);

  return (
    <div class="bubbles" ref={setLayer}>
      <For each={shownIds()}>{(id) => <Bubble id={id} screens={props.screens} name={nameFor(shown().find(terminal => terminal.id === id)!)} place={placeOf(id)} origin={origin()} extent={extent} setPlace={setPlace} readOnly={readOnly} close={close} toggleMinimised={toggleMinimised} />}</For>
      <Show when={bar()}>{(host) => <Portal mount={host()} ref={(element) => { element.classList.add("terminal-tabs-host"); }}>
        <div class="terminal-tabs" role="group" aria-label={copy.terminals} onFocusIn={(event) => {
          if (event.target instanceof HTMLElement) event.target.scrollIntoView({ block: "nearest", inline: "nearest" });
        }}>
          <For each={shownIds()}>{id => <Tab id={id} />}</For>
          <Show when={records().length || problem()}><IconButton class="terminal-recover" aria-label={copy.draftIdentity(copy.recoverDrafts, copy.terminalName)} onClick={() => openRecovery()}><Icon name="edit" /></IconButton></Show>
          <FlatAdd data-button="add" class="terminal-new" aria-label={copy.newTerminal} onClick={() => void open()}>{copy.plusTerminal}</FlatAdd>
        </div>
      </Portal>}</Show>
      <Dialog.Root instant open={recovering()} onOpenChange={details => recover(details.open)}>
        <Portal><Dialog.Positioner asChild={positioner => <aside {...positioner({})} />}><Dialog.Content class="draft-recovery bubble-recovery" ref={element => { recoveryPanel = element; }}>
          <Dialog.Title asChild={title => <h2 {...title({})}>{copy.unfinishedWork}</h2>} />
          <Dialog.Description class="draft-identity">{copy.draftIdentity(connected.project.name, copy.terminalName)}</Dialog.Description>
          <ReservedLine message={(copyFailure()?.message) || (problem() ? (copy.draftRetention(connected.project.name, copy.terminalName, problem()!.cause)) : null) || null}><Show when={copyFailure()}><button data-button="quiet" class="word" type="button" onClick={() => failCopy(null)}>{copy.dismissFailure}</button></Show></ReservedLine>
          <Show when={records().length > 1}><For each={[...records()].reverse().sort((a, b) => Number(b.status === "draft") - Number(a.status === "draft"))}>{(draft, index) => <button type="button" data-button="quiet" aria-pressed={shownDraft()?.revision === draft.revision} onClick={() => inspect(draft.revision)}>{copy.draftIdentity(copy.inspectDraft, draft.status === "draft" ? copy.currentDraft : copy.draftStatus[draft.status], String(draft.value.base ?? ""), String(index() + 1))}</button>}</For></Show>
          <Show when={shownDraft()}>{draft => <>
            <p class="draft-identity">{copy.draftIdentity(String(draft().value.base ?? ""), draft().destination.recipient ?? copy.noDraftRecipient)}</p>
            <ReservedLine status={recoveryNotice() ?? copy.draftStatus[draft().status]} data-state={draft().status} />
            <pre class="draft-value">{String(draft().value.name ?? "")}</pre>
          </>}</Show>
          <div class="draft-actions">
            <Show when={shownDraft()}>{draft => <><button type="button" data-button="quiet" onClick={() => void copyName()}>{copy.copyProjectName}</button>
            <button type="button" data-button="quiet" onClick={() => { drafts.discard(draft().revision); setEditing(null); }}>{copy.discardRevision}</button></>}</Show>
            <Show when={problem()}><button type="button" data-button="quiet" onClick={() => drafts.retry()}>{copy.storageRetry}</button></Show>
            <Dialog.CloseTrigger data-button="quiet">{copy.close}</Dialog.CloseTrigger>
          </div>
        </Dialog.Content></Dialog.Positioner></Portal>
      </Dialog.Root>
      <CloseFailure message={failure()?.message ?? null}><button data-button="quiet" class="word" type="button" onClick={() => setFailure(null)}>{copy.dismissFailure}</button></CloseFailure>
    </div>
  );
};
