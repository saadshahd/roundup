import { cleanup, render, screen } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { App } from "../App";
import { runAxe } from "./axe";
import { fakeEmulators } from "../terminal/paneHarness";
import { seedApp } from "./seeds";
import type { SeedName } from "./seeds";

afterEach(() => {
  cleanup();
  document.body.innerHTML = "";
});

// jsdom has no layout, so axe cannot measure colour there and answers `incomplete` for the grey line: these tests prove it is never a pass.
// The `violation` for the grey line, the `incomplete` for text over an image and the seeds' contrast are the real-browser run of `window.__axe()` in the PR.
describe("y1 axe-core runs on the harness page", () => {
  it("y1_a_grey_line_on_white_is_reported_as_color_contrast_and_never_a_pass", async () => {
    document.body.innerHTML = `<main><p id="low" style="color:#aaa;background:#fff">hard to read</p></main>`;

    const { violations, incomplete } = await runAxe();
    const contrast = [...violations, ...incomplete].filter((found) => found.rule === "color-contrast");

    expect(contrast).toHaveLength(1);
    expect(contrast[0]!.selector).toBe("#low");
  });

  it("y1_text_over_an_image_is_incomplete_and_never_a_pass", async () => {
    document.body.innerHTML = `<main><div style="background-image:url(x.png);width:100px;height:40px"><p id="over" style="color:#777">over image</p></div></main>`;

    const { violations, incomplete } = await runAxe();

    expect(violations.filter((found) => found.rule === "color-contrast")).toEqual([]);
    expect(incomplete.map((found) => found.rule)).toContain("color-contrast");
  });

  it("y1_a_button_with_no_accessible_name_is_an_aria_violation", async () => {
    document.body.innerHTML = `<main><button id="bare"></button></main>`;

    const { violations } = await runAxe();

    expect(violations.map((found) => found.selector)).toContain("#bare");
  });

  const seedViolations = async (seed: SeedName) => {
    const controls = seedApp(seed, Date.now());
    render(() => <App app={controls.app} reducedMotion={() => false} clock={Date.now} createEmulator={fakeEmulators().factory} />);
    await screen.findByRole("region", { name: "rail" });

    return (await runAxe()).violations.filter((found) => found.rule !== "color-contrast");
  };

  it("y1_the_shipped_first_run_seed_returns_none", async () => {
    expect(await seedViolations("first-run")).toEqual([]);
  });

  // agents-10 and tree-40 fail on the Rail's `role=tree` holding buttons (#629); `it.fails` turns red when that is fixed, so the marker comes off then.
  it.fails("y1_the_shipped_agents_10_seed_returns_none", async () => {
    expect(await seedViolations("agents-10")).toEqual([]);
  });

  it.fails("y1_the_shipped_tree_40_seed_returns_none", async () => {
    expect(await seedViolations("tree-40")).toEqual([]);
  });

  it("y1_a_missing_axe_bundle_throws_never_returns_empty", async () => {
    await expect(runAxe(document, null)).rejects.toThrow("axe-core");
  });
});
