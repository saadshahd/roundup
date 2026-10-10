import { fireEvent, screen, waitFor, within } from "@solidjs/testing-library";
import { describe, expect, it } from "vitest";
import { RpcError } from "../app/seam";
import { asking, rpcCalls, card, cleared, decision, gate, mountApp, opened, select } from "./decisionsFixture";

const buttons = () => within(card()!).getAllByRole("button").map((button) => button.textContent?.trim());

describe("u113 the Card for the selected Agent or Workstream's Door", () => {
  it("u113_selecting_an_agent_shows_its_decision_above_its_terminal", async () => {
    await mountApp([decision("d1", "a")]);

    expect(card()).toBeNull();

    select("alpha");

    await waitFor(() => expect(card()).not.toBeNull());

    const text = card()!.textContent ?? "";

    expect(text).toContain("alpha");
    expect(text).toContain("Bash");
    expect(text).toContain("rm -rf build");
    expect(card()!.nextElementSibling?.className).toContain("pane-body");

    select("beta");

    expect(card()).toBeNull();
  });

  it("u113_selecting_a_workstream_shows_its_doors_decision", async () => {
    await mountApp([decision("d1", "r", { tool: "Edit" })]);
    select("harbor");

    await waitFor(() => expect(card()?.textContent).toContain("Edit"));
    expect(card()!.textContent).toContain("harbor");
  });

  it("u113_an_event_never_steals_focus_or_changes_the_selection", async () => {
    const { app } = await mountApp([]);

    select("beta");

    const focused = document.activeElement;

    app.emit(opened(decision("d1", "a")));

    expect(card()).toBeNull();
    expect(document.activeElement).toBe(focused);
    expect(screen.getAllByRole("treeitem").find((row) => row.getAttribute("aria-selected") === "true")?.textContent).toContain("beta");
  });

  it("u113_a_permission_decision_offers_allow_and_deny", async () => {
    await mountApp([decision("d1", "a")]);
    select("alpha");

    await waitFor(() => expect(card()).not.toBeNull());
    expect(buttons()).toEqual(["allow", "deny"]);
  });

  it("u113_an_ask_user_decision_shows_its_question_and_exactly_its_answers", async () => {
    await mountApp([asking("d1", "a", "keep v1 routes?", ["keep", "drop", "ask me later"])]);
    select("alpha");

    await waitFor(() => expect(card()).not.toBeNull());
    expect(card()!.textContent).toContain("keep v1 routes?");
    expect(buttons()).toEqual(["keep", "drop", "ask me later"]);
  });

  it("u113_an_unanswerable_decision_points_at_the_terminal_and_has_no_answer_button", async () => {
    const { app, emulators } = await mountApp([decision("d1", "a", { tool: "AskUserQuestion", answerable: false })]);
    select("alpha");

    await waitFor(() => expect(card()).not.toBeNull());
    expect(card()!.textContent).toContain("answer in the Terminal");
    expect(buttons()).toEqual(["focus Terminal"]);

    let focused = 0;
    const emulator = emulators.made.get("t-a")!;
    emulator.focus = () => { focused += 1; };

    fireEvent.click(within(card()!).getByRole("button", { name: "focus Terminal" }));

    expect(focused).toBe(1);
    expect(rpcCalls(app, "decision.answer")).toEqual([]);
    expect(rpcCalls(app, "terminal.write")).toEqual([]);
  });

  it("u113_an_unreadable_question_shows_an_error_and_terminal_access_and_never_allows", async () => {
    const { app } = await mountApp([decision("d1", "a", { tool: "ask_user", args: "{not json" })]);
    select("alpha");

    await waitFor(() => expect(card()).not.toBeNull());
    expect(within(card()!).getByRole("alert").textContent).toContain("unreadable");
    expect(buttons()).toEqual(["focus Terminal"]);
    expect(rpcCalls(app, "decision.answer")).toEqual([]);
  });

  it("u113_activating_an_answer_sends_one_decision_answer_with_the_id_and_the_proof", async () => {
    const { app } = await mountApp([decision("d1", "a")]);
    const pending = gate<null>();

    app.proof.value = "proof-1";
    app.handlers["decision.answer"] = () => pending.promise;
    select("alpha");
    await waitFor(() => expect(card()).not.toBeNull());

    const allow = within(card()!).getByRole("button", { name: "allow" });

    fireEvent.click(allow);
    await waitFor(() => expect(rpcCalls(app, "decision.answer")).toHaveLength(1));
    fireEvent.click(allow);
    fireEvent.click(allow);

    expect(rpcCalls(app, "decision.answer")).toEqual([{ id: "d1", answer: "allow", proof: "proof-1" }]);
    expect(within(card()!).getByRole("button", { name: "allow" }).hasAttribute("disabled")).toBe(true);
    expect(within(card()!).getByRole("button", { name: "deny" }).hasAttribute("disabled")).toBe(true);

    pending.resolve(null);

    await waitFor(() => expect(card()).toBeNull());
    expect(document.body.textContent).not.toContain("proof-1");
    expect(rpcCalls(app, "terminal.write")).toEqual([]);
  });

  it("u113_a_rejected_answer_keeps_the_decision_with_its_error_and_a_retry", async () => {
    const { app } = await mountApp([decision("d1", "a")]);
    let attempts = 0;

    app.handlers["decision.answer"] = () => {
      attempts += 1;

      if (attempts === 1) throw new RpcError(-32000, "the hook is gone");

      return null;
    };

    select("alpha");
    await waitFor(() => expect(card()).not.toBeNull());
    fireEvent.click(within(card()!).getByRole("button", { name: "deny" }));

    await waitFor(() => expect(within(card()!).getByRole("alert").textContent).toContain("the hook is gone"));

    fireEvent.click(within(card()!).getByRole("button", { name: "deny" }));

    await waitFor(() => expect(card()).toBeNull());
    expect(rpcCalls(app, "decision.answer")).toHaveLength(2);
  });

  it("u113_a_failed_proof_keeps_the_decision_and_sends_no_answer", async () => {
    const { app } = await mountApp([decision("d1", "a")]);

    app.proof.failure = new RpcError(-32603, "no proof yet");
    select("alpha");
    await waitFor(() => expect(card()).not.toBeNull());
    fireEvent.click(within(card()!).getByRole("button", { name: "allow" }));

    await waitFor(() => expect(within(card()!).getByRole("alert").textContent).toContain("no proof yet"));
    expect(rpcCalls(app, "decision.answer")).toEqual([]);
  });

  it("u113_an_older_completion_never_removes_a_replacement", async () => {
    const { app } = await mountApp([decision("d1", "a")]);
    const pending = gate<null>();

    app.handlers["decision.answer"] = () => pending.promise;
    select("alpha");
    await waitFor(() => expect(card()).not.toBeNull());
    fireEvent.click(within(card()!).getByRole("button", { name: "allow" }));
    await waitFor(() => expect(rpcCalls(app, "decision.answer")).toHaveLength(1));

    app.emit(opened(decision("d2", "a", { tool: "Write", opened_at: 2 })));
    app.emit(cleared("d1", "replaced"));
    pending.resolve(null);
    await pending.promise;

    await waitFor(() => expect(card()?.textContent).toContain("Write"));
    expect(within(card()!).getByRole("button", { name: "allow" }).hasAttribute("disabled")).toBe(false);
  });

  it("u113_an_external_clear_removes_the_card", async () => {
    const { app } = await mountApp([decision("d1", "a")]);

    select("alpha");
    await waitFor(() => expect(card()).not.toBeNull());
    app.emit(cleared("d1"));

    await waitFor(() => expect(card()).toBeNull());
  });

  it("u113_clearing_a_focused_card_returns_focus_to_the_terminal_only_when_focus_was_inside", async () => {
    const { app, emulators } = await mountApp([decision("d1", "a")]);

    select("alpha");
    await waitFor(() => expect(card()).not.toBeNull());

    let focused = 0;
    emulators.made.get("t-a")!.focus = () => { focused += 1; };

    within(card()!).getByRole("button", { name: "allow" }).focus();
    app.emit(cleared("d1"));
    await waitFor(() => expect(card()).toBeNull());

    expect(focused).toBe(1);

    app.emit(opened(decision("d2", "a")));
    await waitFor(() => expect(card()).not.toBeNull());
    screen.getAllByRole("treeitem")[0]!.focus();
    app.emit(cleared("d2"));
    await waitFor(() => expect(card()).toBeNull());

    expect(focused).toBe(1);
  });

  it("u113_a_daemon_exit_disables_answering", async () => {
    const { app } = await mountApp([decision("d1", "a")]);

    select("alpha");
    await waitFor(() => expect(card()).not.toBeNull());
    app.exitDaemon({ code: 1 });

    await waitFor(() => expect(within(card()!).getByRole("button", { name: "allow" }).hasAttribute("disabled")).toBe(true));
    expect(screen.getByRole("banner").textContent).toContain("daemon exited 1");
  });
});
