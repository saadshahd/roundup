import { fireEvent, screen, waitFor } from "@solidjs/testing-library";
import { describe, expect, it } from "vitest";
import { mountApp, select } from "../decisions/decisionsFixture";
import { agent, clarifying, door, terminal } from "../testing/nodes";

const work = { kind: "work" as const, ask: "fix the build", limits: ["touch main", "skip tests"] };

const mount = () =>
  mountApp([], (app) => {
    app.handlers["rail.tree"] = () => [
      agent("a", "working", "editing", { name: "alpha", order: 0, work }),
      agent("b", "working", "editing", { name: "beta", order: 1, work: clarifying("which build?") }),
      door("r", "working", "editing", { name: "harbor", order: 2, work: clarifying("what is this Room for?") }),
    ];
  });

const line = () => document.querySelector<HTMLElement>(".order-line");

describe("O6: the order on one line above the Terminal", () => {
  it("o6_a_work_order_agent_shows_its_ask_and_the_count_of_its_limits", async () => {
    await mount();
    select("alpha");

    await waitFor(() => expect(line()?.textContent).toBe("fix the build must not: 2"));
  });

  it("o6_a_clarification_order_agent_and_a_door_show_their_question", async () => {
    await mount();

    select("beta");
    await waitFor(() => expect(line()?.textContent).toBe("clarifying: which build?"));

    select("harbor");
    await waitFor(() => expect(line()?.textContent).toBe("clarifying: what is this Room for?"));
  });

  it("o6_the_line_sits_below_the_decision_card_and_above_the_terminal", async () => {
    await mount();
    select("alpha");

    await waitFor(() => expect(line()).not.toBeNull());
    const body = document.querySelector(".pane-body")!;

    expect(line()!.compareDocumentPosition(body) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it("o6_activating_the_line_opens_the_whole_order_in_the_drawer", async () => {
    await mount();
    select("alpha");

    await waitFor(() => expect(line()).not.toBeNull());
    fireEvent.click(line()!);

    await waitFor(() => expect(screen.getByLabelText("full order of alpha").textContent).toContain("fix the build"));
    expect(screen.getByLabelText("full order of alpha").textContent).toContain("must not: skip tests");
  });

  it("o6_a_terminal_shows_no_order_line", async () => {
    await mountApp([], (app) => {
      app.handlers["rail.tree"] = () => [agent("a", "working", "editing", { name: "alpha", order: 0 }), terminal("t", { name: "zsh", order: 1 }), door("r", "working", "editing", { name: "harbor", order: 2 })];
    });
    select("zsh");

    await waitFor(() => expect(document.querySelector(".pane-body")).not.toBeNull());
    expect(line()).toBeNull();
  });
});
