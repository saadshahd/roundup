import { cleanup, fireEvent, waitFor } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import type { RailNode } from "@contracts/agent/RailNode";
import { chord, mountRail } from "./railFixture";
import { agent, event, terminal } from "../testing/nodes";

afterEach(cleanup);

const SPAWNED_AGENT = agent("fresh-agent", "idle", "starting");

const SPAWNED_TERMINAL = terminal("fresh-terminal");

describe("u47 a spawned terminal is selected and shown", () => {
  it("u47_plus_terminal_selects_the_new_terminal_once_the_tree_has_it", async () => {
    const tree: RailNode[] = [];
    const mounted = await mountRail(tree);
    mounted.app.handlers["rail.spawnTerminal"] = () => {
      tree.push(SPAWNED_TERMINAL);
      mounted.app.emit(event({ name: "rail.changed" }));

      return SPAWNED_TERMINAL;
    };

    chord("t");

    await waitFor(() => expect(mounted.rail.selected()).toBe("fresh-terminal"));
  });

  it("u47_cmd_t_after_cmd_n_selects_the_new_terminal_not_the_agent", async () => {
    const tree: RailNode[] = [];
    const mounted = await mountRail(tree);
    mounted.app.handlers["agent.spawn"] = () => {
      tree.push(SPAWNED_AGENT);
      mounted.app.emit(event({ name: "rail.changed" }));

      return SPAWNED_AGENT;
    };

    mounted.app.handlers["rail.spawnTerminal"] = () => {
      tree.push(SPAWNED_TERMINAL);
      mounted.app.emit(event({ name: "rail.changed" }));

      return SPAWNED_TERMINAL;
    };

    fireEvent.keyDown(document, { key: "n", metaKey: true });
    await waitFor(() => expect(mounted.rail.selected()).toBe("fresh-agent"));

    fireEvent.keyDown(document, { key: "t", metaKey: true });

    await waitFor(() => expect(mounted.rail.selected()).toBe("fresh-terminal"));
  });
});
