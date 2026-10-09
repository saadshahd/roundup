import { Terminal } from "@xterm/xterm";
import { WebglAddon } from "@xterm/addon-webgl";
import { Channel, invoke } from "@tauri-apps/api/core";
import "@xterm/xterm/css/xterm.css";

const N = 1500;
const WARMUP = 100;
const epoch = () => performance.timeOrigin + performance.now();
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

const out_counts: number[][] = [];
type Sample = Record<string, number>;

async function run(webgl: boolean): Promise<Sample[]> {
  const host = document.getElementById("t")!;
  host.innerHTML = "";
  const term = new Terminal({ cols: 80, rows: 24, fontSize: 14 });
  term.open(host);
  if (webgl) term.loadAddon(new WebglAddon());
  term.focus();

  let resolveEcho: ((s: Sample) => void) | null = null;
  let cur: Sample = {};
  const ch = new Channel<ArrayBuffer>();
  ch.onmessage = (buf) => {
    const t_ipc_out_recv = epoch();
    (window as any).__nm = ((window as any).__nm ?? 0) + 1;
    const f = new Float64Array(buf.slice(0, 32));
    cur.r_recv = f[0]; cur.r_written = f[1]; cur.r_read = f[2]; cur.r_sent = f[3];
    cur.w_in = t_ipc_out_recv;
    const data = new Uint8Array(buf, 32);
    term.write(data, () => {
      cur.w_cb = epoch();
      // next rendered frame after write
      const d = term.onRender(() => { d.dispose(); cur.w_render = epoch(); resolveEcho!(cur); });
    });
  };
  term.onData((d) => {
    cur.t_ipc_in_start = epoch();
    (window as any).__nd = ((window as any).__nd ?? 0) + 1;
    invoke("pty_write", { data: d });
  });
  await invoke("pty_start", { onOut: ch });
  await sleep(300);

  const samples: Sample[] = [];
  const chars = "abcdefghijklmnopqrstuvwxyz0123456789";
  for (let i = 0; i < N + WARMUP; i++) {
    const key = chars[i % chars.length];
    if (i % 60 === 59) term.clear(); // keep cursor on-screen; clear is done between samples
    cur = {};
    const p = new Promise<Sample>((r) => (resolveEcho = r));
    cur.t_key = epoch();
    // xterm 6 emits onData for printable keys straight from keydown (dispatching keypress too would double-send)
    const cc = key.charCodeAt(0);
    const init = { key, code: "Key" + key.toUpperCase(), keyCode: cc, which: cc, charCode: cc, bubbles: true, cancelable: true };
    const kd = new KeyboardEvent("keydown", init);
    for (const k of ["keyCode", "which", "charCode"] as const) Object.defineProperty(kd, k, { value: cc });
    term.textarea!.dispatchEvent(kd);
    if (!cur.t_ipc_in_start) throw new Error("synthetic key produced no onData at " + i);
    const s = await Promise.race([p, sleep(2000).then(() => null)]);
    if (!s) throw new Error("timeout waiting echo at " + i);
    if (i >= WARMUP) samples.push(s);
    await sleep(20 + Math.random() * 20); // idle gap, desynchronise from frame phase
  }
  console.log(`ondata=${(window as any).__nd} msgs=${(window as any).__nm}`);
  out_counts.push([(window as any).__nd, (window as any).__nm]); (window as any).__nd = 0; (window as any).__nm = 0;
  term.dispose();
  return samples;
}

const q = (a: number[], p: number) => { const s = [...a].sort((x, y) => x - y); return s[Math.min(s.length - 1, Math.floor(p * s.length))]; };
const stat = (a: number[]) => ({ p50: q(a, .5), p95: q(a, .95), p99: q(a, .99), max: Math.max(...a), mean: a.reduce((x, y) => x + y, 0) / a.length });

function summarize(ss: Sample[]) {
  const col = (f: (s: Sample) => number) => stat(ss.map(f));
  return {
    n: ss.length,
    // a. keydown -> invoke call made (xterm key handling + onData)
    key_to_invoke: col((s) => s.t_ipc_in_start - s.t_key),
    // b. invoke call -> Rust command entry (IPC in)
    ipc_in: col((s) => s.r_recv - s.t_ipc_in_start),
    // c. Rust entry -> echo read in Rust (PTY write + cat + PTY read)
    pty_roundtrip: col((s) => s.r_read - s.r_recv),
    // d. Rust read -> JS channel onmessage (IPC out)
    ipc_out: col((s) => s.w_in - s.r_read),
    // e. onmessage -> xterm.write callback (parse)
    xterm_parse: col((s) => s.w_cb - s.w_in),
    // f. write callback -> next onRender (frame wait + render)
    xterm_render: col((s) => s.w_render - s.w_cb),
    total_to_write_cb: col((s) => s.w_cb - s.t_key),
    total_to_render: col((s) => s.w_render - s.t_key),
  };
}

(async () => {
  const out: Record<string, unknown> = { counts: out_counts, ua: navigator.userAgent, dpr: devicePixelRatio, n: N };
  try {
    for (const [name, gl] of [["dom", false], ["webgl", true], ["dom2", false], ["webgl2", true]] as const) {
      const ss = await run(gl);
      out[name] = summarize(ss);
      out[name + "_raw"] = ss;
    }
  } catch (e) { out.error = String(e); }
  await invoke("save_results", { path: "results.json", json: JSON.stringify(out) });
  await invoke("quit");
})();
