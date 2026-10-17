import { chordsFor, leaderChord } from "../keys/chords";
import { registerKeys } from "../keys/registry";
import { Editable } from "../ui/Editable";
import { FlatAdd, FlatTab } from "../ui/FlatTab";
import { IconButton } from "../ui/IconButton";
import { Menu } from "../ui/Menu";
import { Panel } from "../ui/Panel";
import { copy } from "../copy";
import { createEffect, createMemo, createSignal, For, on, onCleanup, onMount, Show, untrack } from "solid-js";
import { Portal } from "solid-js/web";
import type { TerminalInfo } from "@contracts/terminal/TerminalInfo";
import { ErrorLine } from "../ink/ErrorLine";
import { Icon } from "../ink/Icon";
import { KindGlyph } from "../ink/KindGlyph";
import type { Kind } from "@contracts/Kind";
import { createCut, Tip } from "../ink/Tip";
import { isWorkstream, resolvedWorkstream, siblingsOf } from "../rail/layout";
import { useConnectedProject } from "../state/connectedProject";
import type { Screens } from "../terminal/screens";
import {
  clamped, defaultPlace, placesStorage, resizedAbout, REVEAL_BUBBLE, shownFor, shownName, STEP, toggledFill, ZONES,
} from "./places";
import type { Place, Extent, Zone } from "./places";
import "./styles.css";

/** U2236 call 2: `--space-2` strips centred on the edges, `--space-4` squares over the corners. */
const EDGE = 8;

const CORNER = 16;

const zoneBox = (place: Place, zone: Zone) => {
  const thick = zone.sx !== 0 && zone.sy !== 0 ? CORNER : EDGE;

  const along = (low: number, size: number, sign: number, span: number): [number, number] =>
    sign < 0 ? [low - span / 2, span] : sign > 0 ? [low + size - span / 2, span] : [low + CORNER / 2, size - CORNER];

  const [left, width] = along(place.x, place.w, zone.sx, thick);
  const [top, height] = along(place.y, place.h, zone.sy, thick);

  return { left: `${left}px`, top: `${top}px`, width: `${width}px`, height: `${height}px` };
};

/** U421 engineering call 1: past the 3 to 4 px a hand wobbles while it clicks. */
const PULL_DISTANCE = 8;

/** U3704: the Kind each way a Terminal can be drawn as. */
const KIND_OF: Record<"running" | "clean" | "failed", Kind> = { running: "working", clean: "done", failed: "error" };

const FALLBACK_EXTENT: Extent = { width: 800, height: 600 };

const focusPane = (): void => {
  const target = document.querySelector<HTMLElement>(".pane-screen textarea, .pane-screen [tabindex]");

  target?.focus();

  if (document.activeElement !== target) document.querySelector<HTMLElement>(".terminal-new")?.focus();
};

const ESCAPE = "Escape".split(" ");

const SHIFT = "Shift+".split(" ")[0]!;

const ARROWS = new Map<string, [number, number]>(Object.entries({ ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1] }));

/** U469: the selected Terminals, their Bubble geometry, bottom tabs and opening/cycling chords. */
export const Bubbles = (props: { screens: Screens }) => {
  const connected = useConnectedProject();
  const { rail, app } = connected;
  const path = connected.project.path;
  const store = placesStorage(path);

  const persistPlaces = (value: Record<string, Place>): void => {
    if (connected.daemonExit() === null) store.write(value);
  };

  const [places, setPlaces] = createSignal<Record<string, Place>>(store.read());
  const [failure, setFailure] = createSignal<string | null>(null);
  const [opening, setOpening] = createSignal(false);
  const [wanted, setWanted] = createSignal<string | null>(null);
  const [layer, setLayer] = createSignal<HTMLElement>();
  const [bar, setBar] = createSignal<HTMLElement>();
  const [origin, setOrigin] = createSignal({ left: 0, top: 0 });
  const [selectionMessage, setSelectionMessage] = createSignal(false);
  const [editing, setEditing] = createSignal<string | null>(null);
  onMount(() => setBar(layer()?.parentElement?.querySelector<HTMLElement>(".resource-strip") ?? undefined));
  const closing = new Set<string>();
  let before: Element | null = null;

  const extent = (): Extent => {
    const box = layer()?.getBoundingClientRect();

    return box && box.width > 0 && box.height > 0 ? { width: box.width, height: box.height } : FALLBACK_EXTENT;
  };

  // U524: the Terminal opened from the Project root stays shown while the selection stays there.
  const [fromRoot, setFromRoot] = createSignal<string | null>(null);

  const shown = createMemo(() => rail.selected() === null && fromRoot() !== null
    ? rail.terminals().filter((terminal) => terminal.id === fromRoot())
    : shownFor(rail.terminals(), rail.nodes, rail.selected()));

  // U524: the pane placeholder gives way while a Terminal opened from the root is shown.
  createEffect(() => {
    const stack = layer()?.parentElement;

    if (!stack) return;

    stack.toggleAttribute("data-root-terminal", fromRoot() !== null && shown().length > 0);
    onCleanup(() => stack.removeAttribute("data-root-terminal"));
  });

  const placeOf = (id: string): Place => places()[id] ?? defaultPlace(Object.keys(places()).length, extent());

  // A bubble's default place is stored when it first shows, so hiding it, moving another or resizing the window never shifts it (U164).
  // U2224: selecting an Agent row folds every bubble to its chip, so none covers the output it selects.
  let lastSelected: string | null | undefined;

  createEffect(() => {
    const eligible = shown();
    const chosen = rail.selected();
    const folds = lastSelected !== undefined && chosen !== lastSelected && rail.nodes.some((candidate) => candidate.id === chosen && candidate.kind === "agent");

    lastSelected = chosen;

    const held = untrack(places);
    const missing = eligible.filter((terminal) => held[terminal.id] === undefined);
    const visible = eligible.filter((terminal) => held[terminal.id] && !held[terminal.id]!.min);
    const next = { ...held };
    const latest = folds ? undefined : visible.at(-1)?.id ?? missing.at(-1)?.id;

    for (const terminal of missing) next[terminal.id] = { ...defaultPlace(Object.keys(next).length, untrack(extent)), min: terminal.id !== latest };

    for (const terminal of visible) next[terminal.id] = { ...next[terminal.id]!, min: terminal.id !== latest };

    if (folds) for (const terminal of eligible) next[terminal.id] = { ...next[terminal.id]!, min: true };

    if (JSON.stringify(next) === JSON.stringify(held)) return;
    setPlaces(next);
    persistPlaces(next);
  });

  createEffect(on(() => [rail.selected(), rail.nodes.some(isWorkstream)] as const, () => setSelectionMessage(false), { defer: true }));

  const setPlace = (id: string, place: Place): void => {
    const next = { ...places(), [id]: place };

    setPlaces(next);
    persistPlaces(next);
  };

  const nameFor = (terminal: TerminalInfo): string => shownName(terminal, places()[terminal.id]);

  /** U511: a trimmed name that differs from the shown one is saved on the Terminal's place; an empty one clears it. */
  const rename = (terminal: TerminalInfo, value: string): void => {
    const name = value.trim();

    if (name === nameFor(terminal) || (name === "" && places()[terminal.id]?.name === undefined)) return;

    const { name: _previous, ...place } = placeOf(terminal.id);

    setPlace(terminal.id, name === "" ? place : { ...place, name });
  };

  const show = (id: string | null): void => {
    if (id !== null && !shown().some((terminal) => terminal.id === id)) return;

    const next = { ...places() };

    for (const terminal of shown()) next[terminal.id] = { ...placeOf(terminal.id), min: terminal.id !== id };
    setPlaces(next);
    persistPlaces(next);
  };

  // Saved places end with the Daemon; its last readable screens keep their current geometry and visibility until the view ends.
  createEffect(() => {
    if (connected.daemonExit() !== null) {
      store.clear();

      return;
    }

    const listed = new Set(rail.terminals().map((terminal) => terminal.id));
    const held = untrack(places);
    const kept = Object.fromEntries(Object.entries(held).filter(([id]) => listed.has(id)));

    if (Object.keys(kept).length !== Object.keys(held).length) {
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
      const corner = host.getBoundingClientRect();

      setOrigin((held) => (held.left === corner.left && held.top === corner.top ? held : { left: corner.left, top: corner.top }));

      host.style.bottom = stack && composer && composer.height > 0 ? `${Math.max(0, stack.bottom - composer.top)}px` : "";
    };

    const watched = new ResizeObserver(() => {
      sync();

      const held = untrack(places);
      const next = Object.fromEntries(Object.entries(held).map(([id, place]) => [id, clamped(place, extent())]));

      if (JSON.stringify(next) === JSON.stringify(held)) return;

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

  const running = (terminal: TerminalInfo): boolean => terminal.running && rail.terminalExit(terminal.id) === null;

  const stateText = (terminal: TerminalInfo): string => {
    const exit = rail.terminalExit(terminal.id);

    if (running(terminal)) return copy.running;

    return exit?.kind === "code" ? copy.exit(exit.code) : exit?.kind === "signal" ? copy.exitSignal : copy.exit(terminal.exit_code ?? copy.questionMark);
  };

  /** U3704: the Kind the Bubble bar draws for a Terminal: working while it runs, done after a clean exit, else error. */
  const stateKind = (terminal: TerminalInfo): Kind => {
    const exit = rail.terminalExit(terminal.id);
    const ended = exit?.kind === "code" ? exit.code : exit === null ? terminal.exit_code : null;

    return KIND_OF[running(terminal) ? "running" : ended === 0 ? "clean" : "failed"];
  };

  const close = async (id: string): Promise<void> => {
    if (closing.has(id)) return;

    const active = document.activeElement;
    const controlHeldFocus = active instanceof Element && active.closest("[data-bubble]")?.getAttribute("data-bubble") === id;
    closing.add(id);

    try {
      await app.rpc("terminal.close", { id });
      await rail.refresh({ terminals: true });
      setFailure(null);

      if (controlHeldFocus) queueMicrotask(() => {
        const remaining = shown();
        const nextId = remaining.find((terminal) => Number(terminal.id) > Number(id))?.id ?? remaining.at(-1)?.id;
        const control = nextId === undefined ? document.querySelector<HTMLElement>('.terminal-new') : document.querySelector<HTMLElement>(`.terminal-tab[data-bubble="${nextId}"]`);
        control?.focus();
      });
    } catch (thrown) {
      if (!(thrown instanceof Error)) throw thrown;

      setFailure(thrown.message);
    } finally {
      closing.delete(id);
    }
  };

  const bubbleElement = (id: string): HTMLElement | null => document.querySelector<HTMLElement>(`[data-bubble="${id}"]`);

  const focusItem = (id: string): void => {
    const place = places()[id];

    if (place?.min) document.querySelector<HTMLElement>(`.terminal-tab[data-bubble="${id}"]`)?.focus();
    else props.screens.emulatorFor(id).focus();
  };

  const focusedId = (): string | null => {
    const active = document.activeElement;

    return active instanceof Element ? (active.closest<HTMLElement>("[data-bubble]")?.dataset.bubble ?? null) : null;
  };

  const toggleMinimised = (id: string): void => {
    const place = placeOf(id);

    if (place.min) show(id);
    else setPlace(id, { ...place, min: true });
    queueMicrotask(() => focusItem(id));
  };

  const closeFocused = (): void => {
    const id = focusedId();

    if (id === null) return;

    const back = before;
    const fromTab = document.activeElement?.closest(".terminal-tabs") !== null;

    void close(id).then(() => {
      if (failure() !== null || fromTab) return;

      if (back instanceof HTMLElement && back.isConnected) back.focus();
      else focusPane();
    });
  };

  /** The id `terminal.open` answered, else null when there was nothing to open or the call failed. */
  const openTerminal = async (): Promise<string | null> => {
    // U524: a selection that resolves to no Workstream (the Project root) opens in the first Workstream in Rail order.
    const workstream = resolvedWorkstream(rail.nodes, rail.selected()) ?? siblingsOf(rail.nodes, null).find(isWorkstream)?.id ?? null;

    if (opening() || readOnly()) return null;

    if (workstream === null) {
      setSelectionMessage(true);
      setFailure(null);
      document.querySelector<HTMLElement>(".rail-actions [data-button='add']")?.focus();

      return null;
    }

    setOpening(true);

    try {
      const opened = await app.rpc("terminal.open", { workstream });

      setFromRoot(rail.selected() === null ? opened.terminal_id : null);
      setWanted(opened.terminal_id);
      setFailure(null);
      await rail.refresh({ terminals: true });
      show(opened.terminal_id);
      setSelectionMessage(false);

      return opened.terminal_id;
    } catch (thrown) {
      if (!(thrown instanceof Error)) throw thrown;

      setFailure(thrown.message);

      return null;
    } finally {
      setOpening(false);
    }
  };

  const open = async (): Promise<void> => {
    await openTerminal();
  };

  // U421: the bubble last focused, else the latest started, is the one a Pull takes.
  let lastFocused: string | null = null;

  const pullTarget = (): string | null => {
    const ids = shown().map((terminal) => terminal.id);

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
    let held: MouseEvent | null = null;
    let replaying = false;

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

      const target = pullTarget();

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
      held = null;
      press = null;

      if (event.pointerType !== "mouse" || event.button !== 2 || event.metaKey || event.ctrlKey || event.altKey || event.shiftKey || blocked()) return;

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

      // A Terminal's mouse reporting (U11) hears nothing of a press that became a Pull.
      event.stopPropagation();

      if (id !== null) schedule();
    };

    const up = (event: PointerEvent): void => {
      if (press === null) return;

      if (active) {
        event.stopPropagation();
        swallowMenu = true;
        swallowUp = true;
        held = null;
      } else if (held !== null) {
        // The press never became a Pull: the Terminal hears it now, before its release.
        const target = held.target;

        replaying = true;
        target?.dispatchEvent(new MouseEvent("mousedown", held));
        replaying = false;
        held = null;
      }

      end();
    };

    // A press is held back until it is known not to be a Pull, so a Terminal's mouse reporting (U11) never hears a Pull's press.
    const hold = (event: MouseEvent): void => {
      if (replaying || press === null || event.button !== 2) return;

      held = event;
      event.stopPropagation();
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
    listen("mousedown", hold);
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
    const id = wanted();

    if (id === null || !shown().some((terminal) => terminal.id === id)) return;

    queueMicrotask(() => {
      if (bubbleElement(id) === null) return;

      show(id);
      focusItem(id);
      setWanted(null);
    });
  });

  // U511: the name's editor owns its keys. Help and the Switcher are modal layers, so no chord acts under them (U54); a held key repeats nothing.
  const outsideEditor = (press: KeyboardEvent) => !(press.target instanceof Element && press.target.closest(".terminal-tab-editor"));

  const once = (act: (press: KeyboardEvent) => void) => (press: KeyboardEvent) => {
    if (!press.repeat) act(press);
  };

  registerKeys({ kind: "app" }, [
    { chords: chordsFor("⌘T"), when: outsideEditor, run: once(() => void open()) },
  ]);

  // U165: leader `M` minimises or restores the focused Bubble, leader `W` closes it; with none focused they do nothing.
  registerKeys({ kind: "leader" }, [
    { chords: leaderChord("M"), run: () => {
      const id = focusedId();

      if (id !== null) toggleMinimised(id);
    } },
    { chords: leaderChord("W"), run: closeFocused },
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

    if (!(target instanceof Element) || target.closest("[data-bubble], .bubble-frame, .terminal-tab-pair, [role=menu], [role=menuitem]")) return;

    for (const { id } of shown()) if (!placeOf(id).min) setPlace(id, { ...placeOf(id), min: true });
  };

  // U529: the Switcher chose a Terminal; its owner is selected by now, so the bubble is listed.
  const onReveal = (event: Event): void => {
    const id = event instanceof CustomEvent ? String(event.detail) : null;

    if (id === null || !shown().some((terminal) => terminal.id === id)) return;

    show(id);
    queueMicrotask(() => focusItem(id));
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

  const Bubble = (bubble: { id: string }) => {
    const id = bubble.id;
    const terminal = createMemo(() => shown().find((candidate) => candidate.id === id) ?? rail.terminals().find((candidate) => candidate.id === id));
    let body: HTMLDivElement | undefined;
    let frame: number | null = null;

    const place = () => placeOf(id);

    const refit = () => {
      frame = null;

      if (body && !place().min && !readOnly()) props.screens.resize(id, props.screens.emulatorFor(id).fit(), true);
    };

    const refitNextFrame = () => {
      frame ??= requestAnimationFrame(refit);
    };

    onCleanup(() => {
      if (frame !== null) cancelAnimationFrame(frame);

      props.screens.hide(id);
    });

    const mount = (host: HTMLDivElement) => {
      body = host;

      props.screens.resize(id, props.screens.show(id, host, false));

      if (typeof ResizeObserver !== "undefined") {
        const watched = new ResizeObserver(refitNextFrame);

        watched.observe(host);
        onCleanup(() => watched.disconnect());
      }
    };

    // A bubble that was folded is fitted again when it comes back.
    createEffect(on(() => place().min, (folded) => { if (!folded) refitNextFrame(); }, { defer: true }));

    const drag = (start: PointerEvent, change: (from: Place, dx: number, dy: number) => Place) => {
      if (start.button !== 0) return;

      start.preventDefault();

      const from = place();
      const move = (event: PointerEvent) => setPlace(id, clamped(change(from, event.clientX - start.clientX, event.clientY - start.clientY), extent()));

      const stop = () => {
        window.removeEventListener("pointermove", move);
        window.removeEventListener("pointerup", stop);
      };

      window.addEventListener("pointermove", move);
      window.addEventListener("pointerup", stop);
    };

    // The restore of the zone resize in flight, if any; Escape runs it through the registry.
    let restoreResize: (() => void) | undefined;

    registerKeys({ kind: "app" }, [{ chords: ESCAPE, when: () => restoreResize !== undefined, run: () => restoreResize?.() }]);

    /** U2236: a press on a zone resizes about the centre, one style write per frame; Escape puts the place back. */
    const resize = (start: PointerEvent, zone: Zone) => {
      if (start.button !== 0) return;

      start.preventDefault();
      start.stopPropagation();

      const from = place();
      let pointer = { x: start.clientX, y: start.clientY };
      let pending: number | null = null;

      const apply = () => {
        pending = null;
        setPlace(id, resizedAbout(from, zone, pointer.x - start.clientX, pointer.y - start.clientY, extent()));
      };

      const move = (event: PointerEvent) => {
        pointer = { x: event.clientX, y: event.clientY };
        pending ??= requestAnimationFrame(apply);
      };

      const finish = (restore: boolean) => {
        window.removeEventListener("pointermove", move);
        window.removeEventListener("pointerup", up);
        window.removeEventListener("pointercancel", cancel);
        restoreResize = undefined;

        if (pending !== null) cancelAnimationFrame(pending);

        if (restore) setPlace(id, from);
        else if (pending !== null) apply();

        pending = null;
      };

      const up = () => finish(false);
      const cancel = () => finish(true);

      try {
        if (start.target instanceof Element) start.target.setPointerCapture?.(start.pointerId);
      } catch {
        // The window listeners still follow the pointer.
      }

      window.addEventListener("pointermove", move);
      window.addEventListener("pointerup", up);
      window.addEventListener("pointercancel", cancel);
      restoreResize = cancel;
    };

    let header: HTMLElement | undefined;

    // Arrows on the focused header move the bubble, Shift arrows resize it; the registry matches the exact modifiers.
    registerKeys({ kind: "region", element: () => header }, [{
      chords: [...ARROWS.keys(), ...[...ARROWS.keys()].map((key) => `${SHIFT}${key}`)],
      when: (press) => press.target === header,
      run: (press) => {
        const arrow = ARROWS.get(press.key)!;
        const from = place();

        setPlace(id, clamped(press.shiftKey ? { ...from, w: from.w + arrow[0] * STEP, h: from.h + arrow[1] * STEP } : { ...from, x: from.x + arrow[0] * STEP, y: from.y + arrow[1] * STEP }, extent()));
      },
    }]);

    let panel: HTMLElement | undefined;

    // The browser top layer orders by the last `showPopover`, so a restored bubble is the topmost layer and no z-index ranks it (U2307b).
    createEffect(() => {
      const open = !place().min;

      if (!panel?.showPopover) return;

      if (open && !panel.matches(":popover-open")) panel.showPopover();
      else if (!open && panel.matches(":popover-open")) panel.hidePopover();
    });

    /** U2307a: a primary press outside the bubble minimises it; its own tab or chip toggles it, and the press still reaches its target. */
    const dismissed = (event: { detail: { originalEvent: Event } }): void => {
      const press = event.detail.originalEvent;
      const target = press.target;

      if (!(press instanceof PointerEvent) || press.button !== 0 || (target instanceof Element && target.closest(".terminal-tab-pair"))) return;

      if (!place().min) setPlace(id, { ...place(), min: true });
    };

    return (
      <Panel.Root open={!place().min} onPointerDownOutside={dismissed}>
        <Portal>
          <Panel.Content ref={(element: HTMLElement) => (panel = element)} class="bubble-frame" role="presentation" popover="manual"
            style={{ left: `${origin().left}px`, top: `${origin().top}px` }}>
            <section class="bubble" data-bubble={id} role="group" aria-label={nameFor(terminal()!)} hidden={place().min}
              style={{ left: `${place().x}px`, top: `${place().y}px`, width: `${place().w}px`, height: `${place().h}px` }}>
              <header class="bubble-header" tabindex="0" ref={header} onPointerDown={(event) => {
                  // SAFETY: a pointer event on the header always targets an Element.
                  if (!(event.target as Element).closest("button")) drag(event, (from, dx, dy) => ({ ...from, x: from.x + dx, y: from.y + dy }));
                }} onDblClick={(event) => {
                  // SAFETY: a mouse event on the header always targets an Element.
                  if (!(event.target as Element).closest("button")) setPlace(id, toggledFill(place(), extent()));
                }}>
                <span class="bubble-name">{nameFor(terminal()!)}</span>
                <span class="bubble-state" data-state={stateKind(terminal()!)} role="img" aria-label={stateText(terminal()!)} data-running={running(terminal()!)}><KindGlyph kind={stateKind(terminal()!)} decorative /></span>
                <span class="bubble-controls">
                  <IconButton data-feedback="ink" aria-label={copy.close} onClick={() => void close(id)}><span class="bubble-dot"><Icon name="x" /></span></IconButton>
                  <IconButton data-feedback="ink" aria-label={copy.minimise} onClick={() => toggleMinimised(id)}><span class="bubble-dot"><Icon name="minus" /></span></IconButton>
                </span>
              </header>
              <div class="bubble-body" ref={mount} />
            </section>
            <For each={ZONES}>{(zone) => (
              <span class="bubble-zone" data-zone={zone.name} data-zone-of={id} aria-hidden="true" hidden={place().min}
                style={{ cursor: zone.cursor, ...zoneBox(place(), zone) }} onPointerDown={(event) => resize(event, zone)} />
            )}</For>
          </Panel.Content>
        </Portal>
      </Panel.Root>
    );
  };

  return (
    <div class="bubbles" ref={setLayer}>
      <For each={shownIds()}>{(id) => <Bubble id={id} />}</For>
      <Show when={bar()}>{(host) => <Portal mount={host()} ref={(element) => { element.classList.add("terminal-tabs-host"); }}>
        <div class="terminal-tabs" role="group" aria-label={copy.terminals} onFocusIn={(event) => {
          if (event.target instanceof HTMLElement) event.target.scrollIntoView({ block: "nearest", inline: "nearest" });
        }}>
          <For each={shownIds()}>{(id) => {
            const terminal = () => shown().find((candidate) => candidate.id === id)!;

            const name = () => nameFor(terminal());
            const [nameRef, nameCut] = createCut();
            let renameChosen = false;

            const tab = () => document.querySelector<HTMLElement>(`button.terminal-tab[data-bubble="${id}"]`);

            return <Menu.Root positioning={{ strategy: "fixed" }} onSelect={({ value }) => { renameChosen = value === "rename"; }}
              onOpenChange={({ open }) => {
                // The menu hands focus back to its trigger as it closes; the editor starts after that.
                if (open || !renameChosen) return;

                renameChosen = false;
                setTimeout(() => setEditing(id));
              }}>
              <Menu.ContextTrigger asChild={(trigger) => <FlatTab {...trigger({ class: "terminal-tab-pair" })} selected={!placeOf(id).min} data-bubble={id}>
                <Show when={editing() === id} fallback={
                  <Tip content={name()} cut={nameCut}>
                    {(trigger) => (
                      <button {...trigger()} type="button" data-button="quiet" data-reads={!placeOf(id).min ? "text" : undefined} class="terminal-tab" data-bubble={id}
                        aria-label={name()} aria-describedby={`terminal-state-${id}`} aria-pressed={!placeOf(id).min}
                        onClick={() => toggleMinimised(id)}
                        onDblClick={(event) => { if (event.target instanceof Element && event.target.closest(".terminal-tab-name")) setEditing(id); }}>
                        <span class="terminal-tab-name" ref={nameRef}>{name()}</span>
                        <span id={`terminal-state-${id}`} data-state={running(terminal()) ? "running" : "exited"} role="img" aria-label={stateText(terminal())}>{running(terminal()) ? copy.runningMark : copy.exitedMark}</span>
                      </button>
                    )}
                  </Tip>
                }>
                  <div class="terminal-tab terminal-tab-editor" data-bubble={id} data-pressed={!placeOf(id).min}>
                    <Editable.Root defaultValue={name()} activationMode="none" submitMode="both" finalFocusEl={tab}
                      onValueCommit={({ value }) => rename(terminal(), value)}
                      onEditChange={({ edit }) => { if (!edit) setEditing(null); }}>
                      <Editable.Context>{(api) => {
                        onMount(() => api().edit());

                        return null;
                      }}</Editable.Context>
                      <Editable.Area><Editable.Preview /><Editable.Input aria-label={copy.terminalName} ref={(input: HTMLInputElement) => {
                        // Chromium's select() leaves focus where it was; the editor takes it, with the whole name selected, after the library's own step.
                        onMount(() => requestAnimationFrame(() => { input.focus({ preventScroll: true }); input.select(); }));
                      }} /></Editable.Area>
                    </Editable.Root>
                    <span id={`terminal-state-${id}`} data-state={running(terminal()) ? "running" : "exited"} role="img" aria-label={stateText(terminal())}>{running(terminal()) ? copy.runningMark : copy.exitedMark}</span>
                  </div>
                </Show>
                <IconButton aria-label={copy.closeTerminal(name())} onClick={() => void close(id)}><Icon name="x" /></IconButton>
              </FlatTab>} />
              <Portal><Menu.Positioner><Menu.Content>
                <Menu.Item value="rename">{copy.rename}</Menu.Item>
              </Menu.Content></Menu.Positioner></Portal>
            </Menu.Root>;
          }}</For>
          <FlatAdd data-button="add" class="terminal-new" aria-label={copy.newTerminal} onClick={() => void open()}>{copy.plusTerminal}</FlatAdd>
        </div>
      </Portal>}</Show>
      <Show when={selectionMessage() ? bar() : undefined}>{(host) => <Portal mount={host()} ref={(element) => element.classList.add("terminal-note-host")}><span class="terminal-note" role="status">{copy.selectAWorkstreamFirst}</span></Portal>}</Show>
      <Show when={failure()}>{(message) => <div class="bubble-failure"><ErrorLine message={message()} /></div>}</Show>
    </div>
  );
};
