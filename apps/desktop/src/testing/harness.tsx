import { createRoot } from "solid-js";
import { render } from "solid-js/web";
import { App } from "../App";
import { railStorage } from "../rail/persist/storage";
import { createReducedMotion } from "../app/reducedMotion";
import { runAxe } from "./axe";
import type { AxeResult } from "./axe";
import { runChecks } from "./checks";
import type { Checks } from "./checks";
import { createServedApp } from "./realSeam";
import { isSeedName, SEEDS, seedApp } from "./seeds";
import type { Controls } from "./seeds";
import type { AppSeam } from "../app/seam";
import "../tokens.css";
import "../styles.css";

declare global {
  interface Window {
    __fake: Controls;
    __checks: () => Checks;
    __axe: () => Promise<AxeResult>;
  }
}

const root = document.getElementById("root");

if (!root) throw new Error("harness.html has no #root element");

const query = new URLSearchParams(location.search);

const mount = (app: AppSeam) => {
  const reducedMotion = createRoot(() => createReducedMotion((media) => window.matchMedia(media)));

  render(() => <App app={app} reducedMotion={reducedMotion} clock={Date.now} />, root);
};

window.__checks = () => runChecks();

window.__axe = () => runAxe();

if (query.get("daemon") === "1") {
  // `just harness-real` (U144): the same App on a running `rupd`, so there is no `window.__fake`.
  mount(createServedApp(location.origin));
} else {
  const seed = query.get("seed") ?? "tree-40";

  if (!isSeedName(seed)) throw new Error(`unknown seed "${seed}"; one of ${SEEDS.join(", ")}`);

  const controls = seedApp(seed, Date.now());

  const SELECTED = new Map([["door-stopped", "first-workstream"], ["earlier-run", "earlier-agent"], ["decisions", "agent-1"]]);

  const selected = SELECTED.get(seed);

  if (selected) {
    const failure = railStorage(controls.app.opened.project!.path).write({ selected, collapsed: [] });

    if (failure !== null) throw new Error(failure);
  }

  window.__fake = controls;
  mount(controls.app);
}

