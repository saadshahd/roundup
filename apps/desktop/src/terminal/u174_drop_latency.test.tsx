import { cleanup } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { info, terminal } from "../testing/nodes";
import { toBase64 } from "./base64";
import { callsTo, mountPane } from "./paneHarness";

afterEach(cleanup);

const quoted = (paths: string[]): string => `${paths.map((path) => `'${path}'`).join(" ")} `;

describe("u174 a File drop adds no wait of roundup's", () => {
  it.each([1, 50])("u174_a_drop_reaches_terminal_write_in_the_same_task (%i paths)", async (count) => {
    const { app, emulators, container, connected } = await mountPane([terminal("a"), terminal("b")], [info("t-a"), info("t-b")], 0);

    connected.rail.select("a");
    const body = container.querySelector<HTMLElement>(".pane-body")!;
    const paths = Array.from({ length: count }, (_, index) => `/p/image-${index}.png`);

    body.getBoundingClientRect = () => new DOMRect(0, 0, 100, 100);
    await Promise.resolve();

    app.dropFiles({ phase: "drop", paths, x: 50, y: 50 });

    // No timer advanced and no microtask awaited since the listener's call.
    const writes = callsTo(app, "terminal.write");

    expect(emulators.get("t-a")!.pasted).toEqual([quoted(paths)]);
    expect(writes).toHaveLength(1);
    expect(writes[0]).toEqual({ id: "t-a", data: toBase64(new TextEncoder().encode(quoted(paths))) });
  });
});
