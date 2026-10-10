import { cleanup, render, screen } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { App } from "../App";
import { CHECK_IDS, runChecks } from "../testing/checks";
import { underSettings } from "../testing/media";
import type { Setting } from "../testing/media";
import { seedApp } from "../testing/seeds";
import type { SeedName } from "../testing/seeds";
import tokenCss from "../tokens.css?inline";
import appCss from "../styles.css?inline";
import inkCss from "../ink/styles.css?inline";
import padsCss from "../pads/styles.css?inline";
import railCss from "../rail/styles.css?inline";
import terminalCss from "../terminal/styles.css?inline";
import rowButtonCss from "../todos/rowButton.styles.css?inline";
import chipCss from "../rail/attentionChip.styles.css?inline";

const SHEETS = [tokenCss, appCss, inkCss, padsCss, railCss, terminalCss, rowButtonCss, chipCss].join("\n");

const SEEDS: SeedName[] = ["first-run", "agents-10", "tree-40", "daemon-exits", "conflict"];

const SCHEMES: Setting[][] = [[], ["dark"]];

let sheet: HTMLStyleElement | undefined;

let matchMedia: typeof window.matchMedia | undefined;

afterEach(() => {
  sheet?.remove();

  if (matchMedia) window.matchMedia = matchMedia;
  matchMedia = undefined;
  cleanup();
});

const mount = async (seed: SeedName, settings: Setting[]) => {
  sheet = document.head.appendChild(document.createElement("style"));
  sheet.textContent = underSettings(SHEETS, settings);
  matchMedia = window.matchMedia;
  window.matchMedia = (query) => ({
    matches: settings.includes("dark") && query.includes("prefers-color-scheme: dark"),
    media: query,
    onchange: null,
    addListener() {},
    removeListener() {},
    addEventListener() {},
    removeEventListener() {},
    dispatchEvent: () => false,
  });

  const controls = seedApp(seed, Date.now());
  render(() => <App app={controls.app} reducedMotion={() => false} clock={Date.now} />);
  await screen.findByRole("tree", {}, { timeout: 2000 }).catch(() => undefined);
  await new Promise((done) => setTimeout(done, 50));
};

describe("u136 the existing screens pass", () => {
  for (const seed of SEEDS) {
    for (const settings of SCHEMES) {
      it(`u136_${seed.replace("-", "_")}_${settings.join("") || "light"}_passes_d1_to_d10`, async () => {
        await mount(seed, settings);
        const checks = runChecks();

        for (const id of CHECK_IDS) expect(checks[id], `${id} ${JSON.stringify(checks[id])} on ${seed} ${settings.join() || "light"}`).toMatchObject({ status: "pass" });
      });
    }
  }
});
