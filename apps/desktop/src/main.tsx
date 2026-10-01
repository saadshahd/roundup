import { createRoot } from "solid-js";
import { render } from "solid-js/web";
import { App } from "./App";
import { createReducedMotion } from "./app/reducedMotion";
import { createTauriApp } from "./app/tauri";
import { installPerfHook } from "./perf/install";
import "./styles.css";

const root = document.getElementById("root");

if (!root) throw new Error("index.html has no #root element");

const app = installPerfHook(createTauriApp(), window.location.search, () => performance.now(), requestAnimationFrame);

const reducedMotion = createRoot(() => createReducedMotion((query) => window.matchMedia(query)));

render(() => <App app={app} reducedMotion={reducedMotion} clock={Date.now} />, root);
