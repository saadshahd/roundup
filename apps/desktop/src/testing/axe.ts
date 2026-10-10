// Y1: axe-core's `color-contrast` and `aria-*` rules on what is on screen, as one more source beside D1 to D10 (checks.ts). It writes nothing the user sees.
import axe from "axe-core";

export type AxeFinding = { rule: string; selector: string; value: unknown };

export type AxeResult = { violations: AxeFinding[]; incomplete: AxeFinding[] };

const RULES = ["color-contrast", "button-name"];

const findings = (results: axe.Result[]): AxeFinding[] =>
  results.flatMap((result) =>
    result.nodes.map((node) => ({ rule: result.id, selector: node.target.join(" "), value: node.any[0]?.data ?? node.all[0]?.data ?? node.none[0]?.data ?? null })),
  );

/** Runs axe on `context`; a missing axe bundle throws, never returns empty. */
export const runAxe = async (context: Document | Element = document, engine: typeof axe | null = axe): Promise<AxeResult> => {
  if (!engine) throw new Error("axe-core is not loaded");

  const results = await engine.run(context, {
    runOnly: { type: "rule", values: [...RULES, ...engine.getRules(["cat.aria"]).map((rule) => rule.ruleId)] },
    resultTypes: ["violations", "incomplete"],
  });

  return { violations: findings(results.violations), incomplete: findings(results.incomplete) };
};
