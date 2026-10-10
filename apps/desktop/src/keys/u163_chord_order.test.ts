import { readFileSync } from "node:fs";
import { join } from "node:path";
import { expect, it } from "vitest";

it("u163_the_chord_list_names_cmd_n_before_cmd_t", () => {
  const wireframes = readFileSync(join(process.cwd(), "../../docs/wireframes.md"), "utf8");
  const listed = wireframes.split("\n").filter((line) => line.includes("⌘N") && line.includes("⌘T"));

  expect(listed.length).toBeGreaterThan(0);

  for (const line of listed) expect(line.indexOf("⌘N")).toBeLessThan(line.indexOf("⌘T"));
});
