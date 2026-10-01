# App

The App is the Tauri 2 shell in `crates/desktop` (ADR 0001). It starts a Daemon for one Project and passes calls and events between the webview (`apps/desktop`, see `ui.md`) and that Daemon. It holds no state of its own beyond which Project is open. Tests use Tauri's mock runtime (`tauri::test`) and need no window. A fake `rupd` comes from `ROUNDUP_RUPD_BIN`; a real Daemon runs in-process on a temp socket (`rupd` as a dev-dependency).

## App seam

`crates/desktop` and `apps/desktop` meet only here. It is not a Daemon contract (`contracts/` is), so changing it means changing both sides in one PR.

| Webview calls (Tauri command) | Arguments | Result |
|---|---|---|
| `project` | none | `{name, path}` of the open Project, or `null` |
| `open_project` | `{path}` (absolute) | `{name, path}` |
| `rpc` | `{method, params}` | the method's JSON result, unchanged |
| `subscribe` | `{channel}` (a Tauri `Channel`) | `null`; then every Daemon `Event` is sent on the channel, in the Daemon's order |

Every command fails with `{code, message}`, using the codes in `rpc::code`; an error from the Daemon passes through unchanged. When the Daemon ends while the App runs, the App emits the Tauri event `daemon-exited` with `{code: number | null}`.

**S1 open a Project.** Given the App started as `roundup <project>`, or a webview call `open_project {path}`, then the App starts `rupd <path> --attached` (D1). It puts `RUPD_SOCKET` in that Daemon's environment, set to a socket path unique to this App run and under the 104-byte limit; Terminals inherit it, so hooks and MCP servers find this Daemon and no other. The App waits until `daemon.ping` answers, for at most a bound it names, then returns `{name: <basename>, path}`; afterwards `project` returns the same. The Daemon is `ROUNDUP_RUPD_BIN`, else the `rupd` beside the App's executable. A path that is not a directory is `INVALID_PARAMS` naming it. A Daemon that exits or stays silent past the bound is `INTERNAL`, and the message carries the last lines of its stderr. A second `open_project` while one is open is `CONFLICT`. The App grants the webview exactly these permissions and nothing else: `dialog:allow-open`, `dialog:allow-save`, `core:window:allow-set-badge-count`, `core:event:allow-listen` and `core:event:allow-unlisten`. The last two are what let it hear `daemon-exited`. A release build (`just app-release`, W1) loads the webview's built files instead of the dev server; it is not bundled, signed or notarized.

**S2 calls and events.** Given an open Project, when the webview calls `rpc {method: "daemon.ping", params: null}`, then it gets `{pong: true}`. A call that fails in the Daemon (`todo.get {id: 99}`) fails with the same `{code, message}`. After `subscribe`, an event the Daemon emits (`todo.created`) arrives on the channel, and several arrive in order. Before any Project is open, `rpc` and `subscribe` fail at once with `CONFLICT` saying no Project is open; they never wait.

**S3 lifetime.** When the App exits, the Daemon's stdin closes, so the Daemon exits and its Agents and Terminals stop (D1). When the Daemon exits on its own, the webview gets `daemon-exited` with its exit code (`null` if a signal ended it), and later `rpc` calls fail with `INTERNAL`.

**S4 the window loads the webview.** Given `just app <project>`, then the App's window shows the webview served by the dev server on port 5173, and the dev server fails rather than take another port. Given a release build (`just app-release`, W1), then the window shows the webview's built files embedded from `apps/desktop/dist`. Neither needs an environment override: `crates/desktop/tauri.conf.json` names `devUrl` and `frontendDist`.
