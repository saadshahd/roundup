import { cleanup, render, screen, within } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { App } from "../App";
import { SEEDS, seedApp } from "./seeds";
import type { SeedName } from "./seeds";

const NOW = 1_700_000_000_000;

const mount = (seed: SeedName) => {
  const controls = seedApp(seed, NOW);
  render(() => <App app={controls.app} reducedMotion={() => false} clock={() => NOW} />);

  return controls;
};

const rail = () => screen.findByRole("region", { name: "rail" });

afterEach(cleanup);

describe("u26 the harness seeds", () => {
  it("u26_the_seeds_are_the_five_the_recipe_documents", () => {
    expect(SEEDS).toEqual(["first-run", "agents-10", "tree-40", "daemon-exits", "conflict"]);
  });

  it("u26_first_run_shows_the_empty_rail_text", async () => {
    mount("first-run");

    await screen.findByText("open a folder to start");

    expect((await rail()).textContent).toBe("agents and terminalsappear here, one per row,nested by indent");
  });

  it("u26_agents_10_shows_its_first_and_last_agent_in_the_rail", async () => {
    mount("agents-10");

    const region = within(await rail());

    expect(await region.findByText("agent-1")).toBeTruthy();
    expect(region.getByText("agent-10")).toBeTruthy();
  });

  it("u26_tree_40_shows_an_agent_two_groups_deep_and_its_last_agent_in_the_rail", async () => {
    mount("tree-40");

    const region = within(await rail());

    expect(await region.findByText("token-rotation")).toBeTruthy();
    expect(region.getByText("agent-31")).toBeTruthy();
  });

  it("u26_daemon_exits_shows_the_exit_in_the_centre", async () => {
    mount("daemon-exits");

    await screen.findByText("✕ daemon exited 1");
  });

  it("u26_conflict_fails_the_next_call_with_conflict_and_the_one_after_succeeds", async () => {
    const { app } = mount("conflict");
    await within(await rail()).findByText("agent-1");
    await new Promise((resolve) => setTimeout(resolve, 0));

    await expect(app.rpc("rail.createGroup", { name: "x", parent: null })).rejects.toMatchObject({ code: -32003 });
    await expect(app.rpc("rail.createGroup", { name: "x", parent: null })).resolves.toMatchObject({ name: "x" });
  });
});
