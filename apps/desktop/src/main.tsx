import { render } from "solid-js/web";
import { App } from "./App";
import { createReducedMotion } from "./app/reducedMotion";
import { createTauriApp } from "./app/tauri";
import "./styles.css";

const root = document.getElementById("root");

if (!root) throw new Error("index.html has no #root element");

render(
  () => (
    <App
      app={createTauriApp()}
      reducedMotion={createReducedMotion((query) => window.matchMedia(query))}
      clock={Date.now}
    />
  ),
  root,
);
