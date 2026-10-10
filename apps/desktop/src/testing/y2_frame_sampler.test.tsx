import { cleanup, fireEvent, screen, waitFor } from "@solidjs/testing-library";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import styles from "../styles.css?inline";
import { mountTodos, todo } from "../todos/testHarness";
import { sampleFrames } from "./frameSampler";

// jsdom has no layout: it never scrolls an element and records no `layout-shift`. The sampler tests prove its reading and failures, the stylesheet test proves `.columns` cannot scroll sideways; a Pad Drawer and the shipped App in a real window are the `just harness` run in the PR.
class FakeObserver {
  static supportedEntryTypes = ["layout-shift"];
  observe() {}
  disconnect() {}
  takeRecords() {
    return [];
  }
}

beforeEach(() => vi.stubGlobal("PerformanceObserver", FakeObserver));

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

const sized = (width: number, height: number) => {
  vi.stubGlobal("innerWidth", width);
  vi.stubGlobal("innerHeight", height);
};

const mount = async () => {
  const sheet = document.head.appendChild(document.createElement("style"));
  sheet.textContent = styles;
  await mountTodos([todo(1)]);

  return { columns: document.querySelector(".columns")!, sheet };
};

const openAndClose = async () => {
  fireEvent.click(await screen.findByRole("button", { name: /todo 1/ }));
  await waitFor(() => expect(screen.getByLabelText("drawer").inert).toBe(false));
  fireEvent.click(screen.getByRole("button", { name: "close" }));
  await waitFor(() => expect(screen.getByLabelText("drawer").inert).toBe(true));
};

describe("y2 the Drawer's laws are sampled every frame", () => {
  it.each([[1280, 800], [640, 400]])("y2_the_sampler_reads_a_still_shipped_app_as_clean_at_%i_by_%i", async (width, height) => {
    sized(width, height);
    const { columns, sheet } = await mount();
    const watched = [...document.querySelectorAll(".rail, .centre, .shelf")];

    const sampled = await sampleFrames({ columns, watched, act: openAndClose });
    sheet.remove();

    expect(sampled.frames.length).toBeGreaterThan(0);
    expect(sampled.firstBad).toBeNull();
    expect(sampled.shift).toBe(0);
  });

  it("y2_the_shipped_columns_stylesheet_clips_sideways_overflow", async () => {
    const { columns, sheet } = await mount();

    // `hidden` still scrolls under a focus or `scrollIntoView`; only `clip` cannot.
    expect(getComputedStyle(columns).overflowX).toBe("clip");
    sheet.remove();
  });

  it("y2_the_sampler_names_the_first_frame_whose_scrollLeft_is_above_0", async () => {
    const { columns, sheet } = await mount();
    let scrolled = false;
    Object.defineProperty(columns, "scrollLeft", { get: () => (scrolled ? 12 : 0) });

    const sampled = await sampleFrames({
      columns,
      watched: [],
      act: async () => {
        await new Promise((resolve) => setTimeout(resolve, 50));
        scrolled = true;
      },
    });

    sheet.remove();

    expect(sampled.firstBad).not.toBeNull();
    expect(sampled.firstBad!.scrollLeft).toBe(12);
    expect(sampled.frames.indexOf(sampled.firstBad!)).toBeGreaterThan(0);
  });

  it("y2_a_sampler_that_read_no_frames_fails", async () => {
    const { columns, sheet } = await mount();

    await expect(sampleFrames({ columns, watched: [], act: async () => undefined, afterMs: 0 })).rejects.toThrow("no frames");
    sheet.remove();
  });

  it("y2_a_browser_without_layout_shift_entries_fails", async () => {
    vi.stubGlobal("PerformanceObserver", class { static supportedEntryTypes = []; });

    const { columns, sheet } = await mount();

    await expect(sampleFrames({ columns, watched: [], act: openAndClose })).rejects.toThrow("layout-shift");
    sheet.remove();
  });
});
