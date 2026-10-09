import { cleanup, fireEvent, screen } from "@solidjs/testing-library";
import { afterEach, describe, expect, it, vi } from "vitest";
import { agent, door, room, terminal, USER } from "../testing/nodes";
import { message, mountThread, threadInput } from "./threadFixture";

const input = () => document.querySelector("[data-thread-input]");

const sendButton = () => screen.getByRole("button", { name: "send" });

const sends = (app: { calls: { method: string; params: unknown }[] }) => app.calls.filter((call) => call.method === "message.send");

const [sheet = ""] = Object.values(import.meta.glob<string>("./styles.css", { query: "?raw", import: "default", eager: true }));

afterEach(cleanup);

describe("u145 the Thread's input shows only where it has a Door", () => {
  it("u145_an_empty_project_has_no_thread", async () => {
    await mountThread([]);

    expect(input()).toBeNull();
    expect(document.querySelector(".thread")).toBeNull();
    expect(document.body.textContent).not.toContain("select a Room to talk to its Door");
  });

  it("u145_a_project_whose_only_row_is_a_root_terminal_has_no_thread", async () => {
    await mountThread([terminal("sh", { name: "shell" })]);

    expect(input()).toBeNull();
    expect(screen.queryByRole("button", { name: "send" })).toBeNull();
  });

  it("u145_no_row_selected_has_no_thread_and_alt_0_focuses_nothing", async () => {
    await mountThread([door("room", "idle", "idle")], { select: null });

    expect(input()).toBeNull();

    fireEvent.keyDown(document.body, { key: "¡", code: "Digit0", altKey: true });

    expect(document.activeElement).toBe(document.body);
  });

  it("u145_a_selected_room_shows_the_thread_and_an_agent_inside_it_too", async () => {
    const { connected } = await mountThread([door("room", "idle", "idle"), agent("a", "working", "x", { parent: "room" })]);

    expect(input()).not.toBeNull();

    connected.rail.select("a");

    expect(input()).not.toBeNull();

    connected.rail.select(null);

    expect(input()).toBeNull();
  });

  it("u145_the_composer_is_inset_and_rests_on_its_own_surface_with_one_focus_ring", async () => {
    await mountThread([door("room", "idle", "idle")]);

    expect(sheet).toMatch(/\.thread-composer\s*{[^}]*margin:\s*0 var\(--space-3\) var\(--space-3\)/);
    expect(sheet).toMatch(/\.thread-composer\s*{[^}]*background-color:\s*var\(--sunken\)/);
    expect(sheet).toMatch(/\.thread-composer\s*{[^}]*border:\s*1px solid var\(--hairline\)/);
    expect(sheet).toMatch(/\.thread-composer\s*{[^}]*border-radius:\s*var\(--radius-control\)/);
    expect(sheet).toMatch(/\.thread-composer:has\(input:focus-visible\)\s*{[^}]*outline-color:\s*var\(--accent\)/);
    expect(sheet).toMatch(/\.thread-composer input:focus-visible\s*{[^}]*outline-width:\s*0/);
    expect(sheet).not.toMatch(/#[0-9a-f]{3,8}\b/i);
  });

  it("u145_send_by_pointer_sends_what_enter_sends", async () => {
    const { app } = await mountThread([door("room", "idle", "idle")]);

    fireEvent.input(threadInput(), { target: { value: "hello" } });
    fireEvent.click(sendButton());

    await vi.waitFor(() => expect(threadInput().value).toBe(""));

    expect(sends(app).map((call) => call.params)).toEqual([{ to: "room", kind: "note", body: "hello", replyTo: null }]);
  });

  it("u145_send_is_a_native_button_that_keyboard_activation_reaches", async () => {
    await mountThread([door("room", "idle", "idle")]);

    expect(sendButton().tagName).toBe("BUTTON");
    expect(sendButton().getAttribute("type")).toBe("button");
  });

  it("u145_send_is_disabled_when_there_is_nothing_to_send_and_calls_nothing", async () => {
    const { app } = await mountThread([door("room", "idle", "idle")]);

    expect(sendButton().getAttribute("aria-disabled")).toBe("true");

    fireEvent.click(sendButton());

    expect(sends(app)).toEqual([]);
    expect(screen.queryByRole("alert")).toBeNull();

    fireEvent.input(threadInput(), { target: { value: "x" } });

    expect(sendButton().getAttribute("aria-disabled")).toBeNull();
  });

  it("u145_send_is_disabled_while_a_message_send_is_pending_and_a_repeat_sends_no_duplicate", async () => {
    const { app } = await mountThread([door("room", "idle", "idle")]);
    let release: () => void = () => {};

    app.handlers["message.send"] = (params) => new Promise((resolve) => (release = () => resolve(message(99, { from: USER, to: params.to, body: params.body }))));
    fireEvent.input(threadInput(), { target: { value: "once" } });
    fireEvent.click(sendButton());
    await vi.waitFor(() => expect(sendButton().getAttribute("aria-disabled")).toBe("true"));
    fireEvent.click(sendButton());
    fireEvent.keyDown(threadInput(), { key: "Enter" });

    expect(sends(app)).toHaveLength(1);

    release();
    await vi.waitFor(() => expect(threadInput().value).toBe(""));

    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("u145_opening_selecting_creating_and_starting_move_no_focus_into_the_thread", async () => {
    const { connected } = await mountThread([door("room", "idle", "idle"), agent("a", "working", "x", { parent: "room" })]);

    connected.rail.select("a");
    connected.rail.select("room");

    expect(document.activeElement).not.toBe(input());
  });

  it("u145_start_a_room_moves_no_focus_into_the_thread", async () => {
    const { app } = await mountThread([]);

    app.handlers["rail.createRoom"] = () => room("g");
    fireEvent.click(screen.getByText("start a Room"));
    await vi.waitFor(() => expect(app.calls.some((call) => call.method === "rail.createRoom")).toBe(true));

    expect(document.activeElement).not.toBe(input());
  });

  it("u145_start_door_moves_no_focus_into_the_thread", async () => {
    const { app } = await mountThread([room("g")]);

    app.handlers["rail.startDoor"] = () => door("g", "idle", "idle");
    fireEvent.click(screen.getByText("start Door", { selector: ".pane-action" }));
    await vi.waitFor(() => expect(app.calls.some((call) => call.method === "rail.startDoor")).toBe(true));

    expect(document.activeElement).not.toBe(input());
  });

  it("u145_a_rejected_message_send_shows_its_message_on_the_one_line", async () => {
    const { app } = await mountThread([door("room", "idle", "idle")]);

    app.handlers["message.send"] = () => {
      throw new Error("Door gone");
    };

    fireEvent.input(threadInput(), { target: { value: "hi" } });
    fireEvent.click(sendButton());

    await vi.waitFor(() => expect(screen.getByRole("alert").textContent).toContain("Door gone"));

    expect(sendButton().getAttribute("aria-disabled")).toBeNull();
    expect(threadInput().value).toBe("hi");
  });
});
