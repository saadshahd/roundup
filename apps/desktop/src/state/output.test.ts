import { describe, expect, it } from "vitest";
import type { Event as DaemonEvent } from "@contracts/Event";
import type { OutputEvent } from "@contracts/terminal/OutputEvent";
import { connectEvents } from "../app/events";
import { createFakeApp } from "../testing/fakeApp";
import { connectProject } from "./connectedProject";
import { createOutputFeed, MAX_HELD_CHARS } from "./output";

const output = (id: string, data: string): DaemonEvent => ({
  actor: { kind: "user", id: "you", parent: null },
  name: "terminal.output",
  data: { id, data },
});

const feed = async () => {
  const app = createFakeApp();
  const events = await connectEvents(app);

  return { app, output: createOutputFeed(events) };
};

describe("u11 output before the Pane mounts", () => {
  it("u11_output_that_arrived_before_the_first_listener_is_replayed_in_order", async () => {
    const { app, output: outputFeed } = await feed();
    const heard: OutputEvent[] = [];

    app.emit(output("t1", "YQ=="));
    app.emit(output("t2", "Yg=="));
    app.emit(output("t1", "Yw=="));
    outputFeed.subscribe((event) => heard.push(event));

    expect(heard).toEqual([
      { id: "t1", data: "YQ==" },
      { id: "t2", data: "Yg==" },
      { id: "t1", data: "Yw==" },
    ]);
  });

  it("u11_a_listener_hears_live_output_after_the_replay_exactly_once", async () => {
    const { app, output: outputFeed } = await feed();
    const heard: string[] = [];

    app.emit(output("t1", "YQ=="));
    outputFeed.subscribe((event) => heard.push(event.data));
    app.emit(output("t1", "Yg=="));

    expect(heard).toEqual(["YQ==", "Yg=="]);
  });

  it("u11_output_is_not_held_while_a_listener_is_attached", async () => {
    const { app, output: outputFeed } = await feed();
    outputFeed.subscribe(() => {});
    app.emit(output("t1", "YQ=="));
    const late: string[] = [];

    outputFeed.subscribe((event) => late.push(event.data));

    expect(late).toEqual([]);
  });

  it("u11_output_is_held_again_after_the_last_listener_leaves", async () => {
    const { app, output: outputFeed } = await feed();
    const unsubscribe = outputFeed.subscribe(() => {});
    unsubscribe();
    app.emit(output("t1", "YQ=="));
    const heard: string[] = [];

    outputFeed.subscribe((event) => heard.push(event.data));

    expect(heard).toEqual(["YQ=="]);
  });

  it("u11_held_output_is_bounded_per_terminal_and_drops_the_oldest", async () => {
    const { app, output: outputFeed } = await feed();
    const chunk = "A".repeat(MAX_HELD_CHARS / 2 - 1);
    const heard: OutputEvent[] = [];

    app.emit(output("t1", `${chunk}1`));
    app.emit(output("t1", `${chunk}2`));
    app.emit(output("t1", `${chunk}3`));
    app.emit(output("t2", "Yg=="));
    outputFeed.subscribe((event) => heard.push(event));

    expect(heard.map((event) => event.data.slice(-1))).toEqual(["2", "3", "="]);
  });

  it("u11_replayed_output_is_cleared_so_a_remount_does_not_replay_it_again", async () => {
    const { app, output: outputFeed } = await feed();
    app.emit(output("t1", "YQ=="));
    outputFeed.subscribe(() => {})();
    const remounted: string[] = [];

    outputFeed.subscribe((event) => remounted.push(event.data));

    expect(remounted).toEqual([]);
  });

  it("u11_the_bound_drops_the_oldest_of_the_terminal_that_overflowed_not_of_another", async () => {
    const { app, output: outputFeed } = await feed();
    const chunk = "A".repeat(MAX_HELD_CHARS / 2 - 1);
    const heard: OutputEvent[] = [];

    app.emit(output("t2", "Yg=="));
    app.emit(output("t1", `${chunk}1`));
    app.emit(output("t1", `${chunk}2`));
    app.emit(output("t1", `${chunk}3`));
    outputFeed.subscribe((event) => heard.push(event));

    expect(heard.map((event) => `${event.id}${event.data.slice(-1)}`)).toEqual(["t2=", "t12", "t13"]);
  });

  it("u11_output_that_arrives_during_the_first_fetches_reaches_the_pane_that_mounts_later", async () => {
    const app = createFakeApp();
    app.handlers["rail.tree"] = () => {
      app.emit(output("t1", "YQ=="));

      return [];
    };

    const heard: string[] = [];

    const connected = await connectProject(app, { name: "p", path: "/p" }, () => false, () => 0);
    connected.output.subscribe((event) => heard.push(event.data));

    expect(heard).toEqual(["YQ=="]);
  });
});
