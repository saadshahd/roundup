// U135: jsdom never evaluates a media query, so a test applies one by unwrapping the `@media` blocks whose conditions are all among the features it turns on and dropping the rest: the cascade then answers as it would in a browser with those settings.
export type Setting = "dark" | "more-contrast" | "reduced-transparency" | "reduced-motion";

const FEATURE: Record<Setting, string> = {
  dark: "prefers-color-scheme: dark",
  "more-contrast": "prefers-contrast: more",
  "reduced-transparency": "prefers-reduced-transparency: reduce",
  "reduced-motion": "prefers-reduced-motion: reduce",
};

const conditionsOf = (prelude: string): string[] => [...prelude.matchAll(/\(([^)]+)\)/g)].map((one) => one[1]!.trim());

/** `css` as the browser would see it with `settings` on; any other `@media` block (a width query) is kept as written. */
export const underSettings = (css: string, settings: Setting[]): string => {
  const on = new Set(settings.map((setting) => FEATURE[setting]));
  const settingConditions = new Set(Object.values(FEATURE));
  let out = "";
  let at = 0;

  for (const start of css.matchAll(/@media([^{]*)\{/g)) {
    if (start.index! < at) continue;

    const conditions = conditionsOf(start[1]!);

    if (!conditions.some((condition) => settingConditions.has(condition))) continue;

    let depth = 1;
    let end = start.index! + start[0].length;

    while (depth > 0 && end < css.length) depth += css[end++] === "{" ? 1 : css[end - 1] === "}" ? -1 : 0;

    out += css.slice(at, start.index!);

    if (conditions.every((condition) => !settingConditions.has(condition) || on.has(condition))) out += css.slice(start.index! + start[0].length, end - 1);
    at = end;
  }

  return out + css.slice(at);
};

/** Every custom property the `:root` rules of `css` set, a later rule winning. */
export const rootTokensOf = (css: string): ReadonlyMap<string, string> => {
  const tokens = new Map<string, string>();

  for (const root of css.matchAll(/:root\s*\{([^}]*)\}/g))
    for (const declaration of root[1]!.matchAll(/--([\w-]+):\s*([^;\s][^;]*);/g)) tokens.set(declaration[1]!, declaration[2]!.trim());

  return tokens;
};
