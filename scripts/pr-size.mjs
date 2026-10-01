// AGENTS.md rule 3: a small PR is at most 400 changed lines (lockfiles and generated files excluded)
// and touches exactly one module directory, or only contracts/.
// Usage: node scripts/pr-size.mjs <base-ref>
import { execFileSync } from "node:child_process";

const base = process.argv[2] ?? "origin/main";

const limit = 400;

const numstat = execFileSync("git", ["diff", "--numstat", `${base}...HEAD`], { encoding: "utf8" });

const ignored = (path) => /(^|\/)(Cargo\.lock|pnpm-lock\.yaml)$/.test(path) || path.startsWith("contracts/generated/");

const moduleOf = (path) => {
  const [top, name] = path.split("/");

  if (top === "contracts" || top === "crates" || top === "apps" || top === "ext") {
    return top === "contracts" ? "contracts" : `${top}/${name}`;
  }

  return null;
};

let changed = 0;

const modules = new Set();

const outside = [];

for (const line of numstat.split("\n").filter(Boolean)) {
  const [added, removed, path] = line.split("\t");

  if (ignored(path)) continue;
  changed += (Number(added) || 0) + (Number(removed) || 0);
  const module = moduleOf(path);

  if (module) modules.add(module);
  else outside.push(path);
}

const problems = [];

if (changed > limit) problems.push(`${changed} changed lines; the limit is ${limit}`);

if (modules.size > 1) problems.push(`touches ${modules.size} modules: ${[...modules].join(", ")}`);

if (outside.length > 0 && modules.size > 0) problems.push(`mixes module code with files outside any module: ${outside.join(", ")}`);

if (problems.length > 0) {
  console.error(`pr-size: not a small PR:\n- ${problems.join("\n- ")}`);
  process.exit(1);
}

console.log(`pr-size: ok (${changed} lines, ${modules.size} module)`);
