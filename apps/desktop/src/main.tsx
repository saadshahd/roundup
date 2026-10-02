import { createRoot } from "solid-js";
import { render } from "solid-js/web";
import { App } from "./App";
import { createReducedMotion } from "./app/reducedMotion";
import { createTauriApp } from "./app/tauri";
import "./styles.css";

const root = document.getElementById("root");

if (!root) throw new Error("index.html has no #root element");

const app = createTauriApp();

const reducedMotion = createRoot(() => createReducedMotion((query) => window.matchMedia(query)));

// Only a build made with VITE_ROUNDUP_PERF carries the keystroke run; Vite drops the branch and the import from every other build (K1).
const keystrokes = import.meta.env.VITE_ROUNDUP_PERF
  ? await import("./perf/run").then(({ createKeystrokeRun }) => createKeystrokeRun(app))
  : null;

render(() => <App app={app} reducedMotion={reducedMotion} clock={Date.now} createEmulator={keystrokes?.createEmulator} />, root);

void keystrokes?.start();
