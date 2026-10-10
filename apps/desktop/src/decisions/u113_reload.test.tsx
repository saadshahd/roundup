import { cleanup, fireEvent, screen, waitFor, within } from "@solidjs/testing-library";
import { describe, expect, it } from "vitest";
import { card, decision, mountApp, rpcCalls, select } from "./decisionsFixture";

describe("u113 a reload and a reopen", () => {
  it("u113_a_webview_reload_restores_the_live_decisions", async () => {
    const live = [decision("d1", "a")];
    const first = await mountApp(live);

    select("alpha");
    await waitFor(() => expect(card()).not.toBeNull());

    cleanup();
    expect(card()).toBeNull();

    await mountApp(live);
    select("alpha");

    await waitFor(() => expect(card()?.textContent).toContain("rm -rf build"));
    expect(rpcCalls(first.app, "decision.list").length).toBeGreaterThan(0);
  });

  it("u113_empty_results_leave_no_card", async () => {
    await mountApp([]);
    select("alpha");

    expect(card()).toBeNull();
    expect(document.querySelector(".decision-card")).toBeNull();
  });

  it("u113_reopen_answers_with_the_new_connections_proof_and_the_proof_never_shows", async () => {
    const { app } = await mountApp([decision("d1", "a")]);

    app.proof.value = "proof-old-7f3a";
    app.exitDaemon({ code: 1 });
    app.proof.value = "proof-new-9c1d";
    fireEvent.click(within(screen.getByRole("banner")).getByRole("button", { name: "reopen" }));

    await waitFor(() => expect(screen.getByRole("banner").textContent).not.toContain("daemon exited"));
    select("alpha");
    await waitFor(() => expect(card()).not.toBeNull());
    fireEvent.click(within(card()!).getByRole("button", { name: "allow" }));

    await waitFor(() => expect(rpcCalls(app, "decision.answer")).toHaveLength(1));
    expect(rpcCalls(app, "decision.answer")).toEqual([{ id: "d1", answer: "allow", proof: "proof-new-9c1d" }]);

    const seen = [document.documentElement.outerHTML, JSON.stringify({ ...localStorage }), JSON.stringify({ ...sessionStorage })];

    for (const text of seen) {
      expect(text).not.toContain("proof-new-9c1d");
      expect(text).not.toContain("proof-old-7f3a");
    }

    expect(rpcCalls(app, "terminal.write")).toEqual([]);
  });
});
