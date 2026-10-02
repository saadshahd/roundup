import { cleanup, fireEvent, render, waitFor } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { App } from "../App";
import { rowOf } from "../rail/railFixture";
import { createFakeApp } from "../testing/fakeApp";
import { agent } from "../testing/nodes";

afterEach(cleanup);

describe("u41 the App mounts Keys", () => {
  it("u41_cmd_1_focuses_the_rail_in_the_real_app", async () => {
    const app = createFakeApp();
    app.opened.project = { name: "p", path: "/p" };
    app.handlers["rail.tree"] = () => [agent("a", "idle", "x")];

    render(() => <App app={app} reducedMotion={() => false} clock={() => 0} />);

    await waitFor(() => rowOf("a"));

    fireEvent.keyDown(document, { key: "1", metaKey: true });

    expect(document.activeElement).toBe(rowOf("a"));
  });
});
