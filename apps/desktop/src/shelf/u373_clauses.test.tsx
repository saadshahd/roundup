import { leaderKey } from "../keys/leaderFixture";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { App } from "../App";
import { fakeEmulators } from "../terminal/paneHarness";
import { seedApp } from "../testing/seeds";
import type { SeedName } from "../testing/seeds";
import tokenCss from "../tokens.css?raw";
import appCss from "../styles.css?inline";
import railCss from "../rail/styles.css?inline";
import buttonCss from "../buttons.css?inline";
import inkCss from "../ink/styles.css?inline";
import rowCss from "../todos/rowButton.styles.css?inline";
import stripCss from "../header/strip.styles.css?inline";
import shelfCss from "./styles.css?inline";

/** The then-clauses of U368, U370 and U371 that `u373_frame.test.tsx` names but does not assert, each read on the rendered tree. */

let sheet: HTMLStyleElement | undefined;

afterEach(() => {
  sheet?.remove();
  cleanup();
  localStorage.clear();
  document.querySelectorAll("[data-probe]").forEach((probe) => probe.remove());
});

const mountApp = async (seed: SeedName = "tree-40") => {
  window.matchMedia ??= (query) => ({ matches: false, media: query, onchange: null, addListener() {}, removeListener() {}, addEventListener() {}, removeEventListener() {}, dispatchEvent: () => false });
  sheet = document.head.appendChild(document.createElement("style"));
  sheet.textContent = [tokenCss, appCss, railCss, buttonCss, inkCss, rowCss, stripCss, shelfCss].join("\n");
  const controls = seedApp(seed, Date.now());

  render(() => <App app={controls.app} reducedMotion={() => true} clock={Date.now} createEmulator={fakeEmulators().factory} />);
  await waitFor(() => expect(shelf()).not.toBeNull());
  await within(shelf()).findByRole("tablist", { name: "Work items" });

  return controls;
};

const shelf = () => document.querySelector<HTMLElement>(".shelf")!;

const tabs = () => within(screen.getByRole("tablist", { name: "Work items" })).getAllByRole("tab");

const tabNamed = (name: string) => tabs().find((tab) => tab.textContent!.startsWith(name))!;

const chord = (init: KeyboardEventInit = {}, target: Element | Document = document.body) =>
  leaderKey("KeyS", init, target);

const selectRow = (id: string) => {
  fireEvent.click(document.querySelector(`[data-id="${id}"]`)!);
};

const scopeButton = () => within(shelf()).queryByRole("button", { name: /Show (all Workstreams|only this Workstream)/ });

const probe = (role: string, attributes: Record<string, string> = {}) => {
  const element = document.body.appendChild(document.createElement("div"));

  element.dataset["probe"] = "";
  element.setAttribute("role", role);

  for (const [name, value] of Object.entries(attributes)) element.setAttribute(name, value);

  return element;
};

describe("u368 the control is absent unless a Workstream is selected", () => {
  it("u368_absent_with_an_agent_or_terminal_selected", async () => {
    await mountApp();

    selectRow("backend");
    expect(await within(shelf()).findByRole("button", { name: "Show all Workstreams" })).toBeTruthy();

    for (const id of ["migrate"]) {
      selectRow(id);
      await waitFor(() => expect(document.querySelector(`[data-id="${id}"]`)!.getAttribute("aria-selected")).toBe("true"));
      expect(scopeButton(), id).toBeNull();
    }
  });

  it("u368_this_one_narrows_to_the_selected_workstreams_todos", async () => {
    const controls = await mountApp();

    await controls.app.rpc("rail.createWorkstream", { name: "elsewhere", parent: null });
    selectRow("backend");

    const toggle = await within(shelf()).findByRole("button", { name: "Show all Workstreams" });

    fireEvent.click(toggle);
    const wide = shelf().querySelectorAll("[data-todo]").length;

    fireEvent.click(toggle);
    const homed = shelf().querySelectorAll("[data-todo]").length;

    // The seed's Todos belong to the Agent `tokens` under `auth`, so `backend` is not their Home: the list narrows.
    expect(wide).toBeGreaterThan(0);
    expect(homed).toBeLessThan(wide);
    fireEvent.click(toggle);
    expect(shelf().querySelectorAll("[data-todo]")).toHaveLength(wide);
  });
});

describe("u370 the chord and esc", () => {
  it("u370_leader_s_toggles_from_a_terminal_and_never_reaches_it", async () => {
    await mountApp();
    const field = document.body.appendChild(document.createElement("textarea"));
    const reached: KeyboardEvent[] = [];

    field.dataset["probe"] = "";
    field.className = "xterm-helper-textarea";
    field.addEventListener("keydown", (press) => reached.push(press));
    field.focus();

    chord({}, field);
    expect(shelf().hidden).toBe(true);
    chord({}, field);
    expect(shelf().hidden).toBe(false);
    expect(reached).toEqual([]);
  });

  it("u370_leader_s_toggles_with_a_drawer_open", async () => {
    await mountApp();
    fireEvent.click(tabNamed("Pads"));
    await within(shelf()).findByRole("button", { name: "Add Pad" });

    const drawer = probe("dialog", { "aria-label": "drawer" });

    drawer.className = "drawer";
    drawer.dataset["open"] = "true";
    expect(shelf().hidden).toBe(false);
    chord({}, drawer);
    await waitFor(() => expect(shelf().hidden).toBe(true));
  });

  it("u370_caps_lock_is_neutral", async () => {
    await mountApp();
    chord({ key: "|", modifierCapsLock: true });
    expect(shelf().hidden).toBe(true);
    chord({ key: "\\", modifierCapsLock: true });
    expect(shelf().hidden).toBe(false);
  });

  it("u370_esc_does_nothing_with_a_drawer_or_dialog_open", async () => {
    await mountApp();

    const drawer = probe("region");

    drawer.className = "drawer";
    drawer.dataset["open"] = "true";
    fireEvent.keyDown(tabNamed("Todos"), { key: "Escape" });
    expect(shelf().hidden).toBe(false);
    drawer.remove();

    const dialog = probe("alertdialog");

    fireEvent.keyDown(tabNamed("Todos"), { key: "Escape" });
    expect(shelf().hidden).toBe(false);
    dialog.remove();
    fireEvent.keyDown(tabNamed("Todos"), { key: "Escape" });
    expect(shelf().hidden).toBe(true);
  });

  it("u370_esc_does_not_close_from_a_text_field_inside", async () => {
    await mountApp();
    fireEvent.click(tabNamed("Pads"));
    fireEvent.click(await within(shelf()).findByRole("button", { name: "Add Pad" }));

    const field = await within(shelf()).findByLabelText("Pad name");

    fireEvent.keyDown(field, { key: "Escape" });
    expect(shelf().hidden).toBe(false);
  });
});
