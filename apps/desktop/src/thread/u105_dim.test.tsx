import { cleanup, fireEvent } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { agent, door } from "../testing/nodes";
import { effectiveOpacity, mountThread, threadInput } from "./threadFixture";

const tree = () => [door("workstream", "idle", "idle"), agent("calm", "working", "typing", { parent: "workstream" }), agent("asks", "needs-you", "asks: keep v1?", { parent: "workstream" })];

const row = (id: string) => document.querySelector<HTMLElement>(`[role="tree"] [data-id="${id}"]`)!;

const [tokens = ""] = Object.values(import.meta.glob<string>("../tokens.css", { query: "?raw", import: "default", eager: true }));

const [sheet = ""] = Object.values(import.meta.glob<string>("./styles.css", { query: "?raw", import: "default", eager: true }));

afterEach(cleanup);

describe("u105 typing dims the periphery", () => {
  it("u105_typing_dims_every_row_but_a_needs_you_row_and_every_live_line_to_0_35", async () => {
    await mountThread(tree());
    threadInput().focus();

    expect(effectiveOpacity(row("calm"))).toBe(1);

    fireEvent.input(threadInput(), { target: { value: "h" } });

    expect(effectiveOpacity(row("calm"))).toBe(0.35);
    expect(effectiveOpacity(row("workstream"))).toBe(0.35);
    expect(effectiveOpacity(row("asks"))).toBe(1);
    expect(effectiveOpacity(document.querySelector("[data-todo]")!)).toBe(0.35);
    expect(effectiveOpacity(document.querySelector("[aria-label=pads] [data-shelf-row]")!)).toBe(0.35);

    const lives = [...document.querySelectorAll(".live")];

    expect(lives.length).toBeGreaterThan(0);

    for (const live of lives) expect(effectiveOpacity(live)).toBe(0.35);
  });

  it("u105_enter_brings_the_periphery_back", async () => {
    await mountThread(tree());
    threadInput().focus();
    fireEvent.input(threadInput(), { target: { value: "h" } });
    fireEvent.keyDown(threadInput(), { key: "Enter" });

    expect(effectiveOpacity(row("calm"))).toBe(1);
  });

  it("u105_losing_focus_brings_the_periphery_back", async () => {
    await mountThread(tree());
    threadInput().focus();
    fireEvent.input(threadInput(), { target: { value: "h" } });
    fireEvent.blur(threadInput());

    expect(effectiveOpacity(row("calm"))).toBe(1);
  });

  it("u105_the_crossfade_is_the_120_ms_token_and_reduced_motion_changes_both_at_once", async () => {
    expect(tokens).toMatch(/--duration-crossfade:\s*120ms/);
    expect(sheet).toMatch(/transition:\s*opacity var\(--duration-crossfade\)/);

    await mountThread(tree(), { reducedMotion: true });
    threadInput().focus();
    fireEvent.input(threadInput(), { target: { value: "h" } });

    expect(document.querySelector<HTMLElement>(".window")!.dataset.reducedMotion).toBe("true");
    expect(getComputedStyle(row("calm")).transition).toBe("none");
  });

  it("u105_a_dimmed_row_still_takes_clicks", async () => {
    const { connected } = await mountThread(tree());
    threadInput().focus();
    fireEvent.input(threadInput(), { target: { value: "h" } });

    expect(getComputedStyle(row("calm")).pointerEvents).not.toBe("none");

    fireEvent.click(row("calm"));

    expect(connected.rail.selected()).toBe("calm");
  });
});
