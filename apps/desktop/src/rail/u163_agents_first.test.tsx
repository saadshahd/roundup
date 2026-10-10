import { readFileSync } from "node:fs";
import { join } from "node:path";
import { cleanup, screen, waitFor } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { workstream } from "../testing/nodes";
import { mountRail } from "./railFixture";

afterEach(cleanup);

const wireframes = readFileSync(join(process.cwd(), "../../docs/wireframes.md"), "utf8");

describe("u163 Agents come before Terminals", () => {
  it("u163_the_inside_line_lists_agent_then_terminal", async () => {
    const mounted = await mountRail([workstream("g")]);
    mounted.rail.select("g");

    await waitFor(() => expect([...document.querySelectorAll(".rail-adds button")].map((button) => button.textContent?.trim())).toEqual(["agent", "terminal"]));
  });

  it("u163_on_an_empty_project_the_first_tab_stop_in_the_rail_is_new_workstream", async () => {
    await mountRail([]);

    const stops = [...document.querySelectorAll<HTMLElement>(".rail-tree button, .rail-tree [tabindex='0']")];

    expect(stops[0]?.textContent?.trim()).toBe("new Workstream");
    expect(screen.queryByText("terminal", { selector: "button" })).toBeNull();
  });

  it("u163_a_document_names_agent_before_terminal_in_every_add_line", () => {
    const lines = wireframes.split("\n").filter((line) => line.includes("+ terminal"));

    expect(lines.length).toBeGreaterThan(0);

    for (const line of lines.filter((each) => each.includes("+ agent"))) expect(line.indexOf("+ agent")).toBeLessThan(line.indexOf("+ terminal"));

    expect(wireframes).toContain("`+ agent  + terminal`");
  });
});
