import { createSignal } from "solid-js";
import { render } from "solid-js/web";
import { App } from "./App";
import { createTauriApp } from "./app/tauri";
import "./styles.css";

const root = document.getElementById("root");

if (!root) throw new Error("index.html has no #root element");

const reduced = window.matchMedia("(prefers-reduced-motion: reduce)");

const [reducedMotion, setReducedMotion] = createSignal(reduced.matches);

reduced.addEventListener("change", (change) => setReducedMotion(change.matches));

render(() => <App app={createTauriApp()} reducedMotion={reducedMotion} />, root);
