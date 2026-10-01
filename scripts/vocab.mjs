// AGENTS.md rule 6: names we declare must not use a word CONTEXT.md says to avoid.
// Checks declarations only (pub items, TS exports, RPC method strings), not uses of std or library names.
import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative } from "node:path";

const root = new URL("..", import.meta.url).pathname;

const exempt = ["crates/agents/src/claude_code/", "crates/agents/claude_code/"];

const skipDirs = new Set(["node_modules", "target", ".git", "tools", "spikes", "generated"]);

const avoid = new Set(
  [...readFileSync(join(root, "CONTEXT.md"), "utf8").matchAll(/_Avoid:_ ([^.]*)\./g)]
    .flatMap((match) => match[1].split(",").flatMap((word) => {
      const single = word.trim().toLowerCase();

      return /^[a-z]+$/.test(single) ? [single] : [];
    })),
);

const declarations = [
  /\bpub(?:\([a-z]+\))?\s+(?:async\s+)?(?:fn|struct|enum|const|type|mod|trait|static)\s+([A-Za-z_][A-Za-z0-9_]*)/g,
  /^\s*pub\s+([a-z_][a-z0-9_]*)\s*:/gm,
  /\bexport\s+(?:type|const|function|interface)\s+([A-Za-z_][A-Za-z0-9_]*)/g,
  /"([a-z]+(?:\.[A-Za-z]+)+)"/g,
];

function* sources(dir) {
  for (const name of readdirSync(dir)) {
    if (skipDirs.has(name)) continue;
    const path = join(dir, name);

    if (statSync(path).isDirectory()) yield* sources(path);
    else if (/\.(rs|ts)$/.test(name)) yield path;
  }
}

const words = (name) =>
  name
    .replace(/([a-z0-9])([A-Z])/g, "$1 $2")
    .toLowerCase()
    .split(/[^a-z]+/)
    .filter(Boolean)
    .map((word) => (word.endsWith("s") ? word.slice(0, -1) : word));

const failures = [];

for (const file of sources(root)) {
  const rel = relative(root, file);

  if (exempt.some((prefix) => rel.startsWith(prefix))) continue;
  const text = readFileSync(file, "utf8");

  for (const pattern of declarations) {
    for (const match of text.matchAll(pattern)) {
      const bad = words(match[1]).find((word) => avoid.has(word));

      if (bad) failures.push(`${rel}: "${match[1]}" uses the avoided word "${bad}" (see CONTEXT.md)`);
    }
  }
}

if (failures.length > 0) {
  console.error(failures.join("\n"));
  process.exit(1);
}

console.log(`vocab: ok (${avoid.size} avoided words)`);
