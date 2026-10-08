import { fireEvent, waitFor, within } from "@solidjs/testing-library";
import { describe, expect, it } from "vitest";
import { RpcError } from "../app/seam";
import { card, callsTo, cleared, decision, gate, mountApp, opened, select } from "./decisionsFixture";

describe("u113 listing and events reconcile by id", () => {
  it("u113_the_event_stream_is_subscribed_before_decision_list_is_called", async () => {
    const order: string[] = [];

    await mountApp([], (app) => {
      const subscribe = app.subscribe;
      const list = app.handlers["decision.list"]!;

      app.subscribe = async (listener) => {
        await subscribe(listener);
        order.push("subscribed");
      };
      app.handlers["decision.list"] = (params) => {
        order.push("listed");

        return list(params);
      };
    });

    expect(order.slice(0, 2)).toEqual(["subscribed", "listed"]);
  });

  it("u113_a_decision_cleared_during_the_list_read_is_not_resurrected", async () => {
    const listing = gate<ReturnType<typeof decision>[]>();
    const { app } = await mountApp(listing.promise);

    select("alpha");
    app.emit(opened(decision("d1", "a")));
    app.emit(cleared("d1"));
    listing.resolve([decision("d1", "a")]);
    await listing.promise;
    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(card()).toBeNull();
  });

  it("u113_an_old_list_never_overwrites_a_replacement", async () => {
    const listing = gate<ReturnType<typeof decision>[]>();
    const { app } = await mountApp(listing.promise);

    select("alpha");
    app.emit(opened(decision("d2", "a", { tool: "Write", opened_at: 2 })));
    app.emit(cleared("d1", "replaced"));
    listing.resolve([decision("d1", "a")]);
    await listing.promise;

    await waitFor(() => expect(card()?.textContent).toContain("Write"));
    expect(document.querySelectorAll(".decision-card")).toHaveLength(1);
  });

  it("u113_a_decision_opened_during_the_read_shows_without_waiting_for_the_list", async () => {
    const listing = gate<ReturnType<typeof decision>[]>();
    const { app } = await mountApp(listing.promise);

    select("alpha");
    app.emit(opened(decision("d1", "a")));

    await waitFor(() => expect(card()).not.toBeNull());
    expect(callsTo(app, "decision.list")).toHaveLength(1);
  });

  it("u113_a_failed_list_shows_its_error_with_a_retry_that_lists_again", async () => {
    let calls = 0;

    const { container } = await mountApp([], (app) => {
      app.handlers["decision.list"] = () => {
        calls += 1;

        if (calls === 1) throw new RpcError(-32603, "list failed");

        return [decision("d1", "a")];
      };
    });

    await waitFor(() => expect(container.querySelector(".decision-list-failure")?.textContent).toContain("list failed"));

    fireEvent.click(within(container.querySelector<HTMLElement>(".decision-list-failure")!).getByRole("button", { name: "retry" }));
    select("alpha");

    await waitFor(() => expect(card()).not.toBeNull());
    expect(container.querySelector(".decision-list-failure")).toBeNull();
  });

  it("u113_empty_results_leave_no_card", async () => {
    await mountApp([]);
    select("alpha");

    expect(card()).toBeNull();
  });
});
