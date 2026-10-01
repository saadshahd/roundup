import { createRoot } from "solid-js";
import { render } from "solid-js/web";
import { App } from "../App";
import { createReducedMotion } from "../app/reducedMotion";
import { isSeedName, SEEDS, seedApp } from "./seeds";
import type { Controls } from "./seeds";
import "../styles.css";

declare global {
  interface Window {
    __fake: Controls;
  }
}

const root = document.getElementById("root");

if (!root) throw new Error("harness.html has no #root element");

const seed = new URLSearchParams(location.search).get("seed") ?? "tree-40";

if (!isSeedName(seed)) throw new Error(`unknown seed "${seed}"; one of ${SEEDS.join(", ")}`);

const controls = seedApp(seed, Date.now());

window.__fake = controls;

const reducedMotion = createRoot(() => createReducedMotion((query) => window.matchMedia(query)));

render(() => <App app={controls.app} reducedMotion={reducedMotion} clock={Date.now} />, root);
