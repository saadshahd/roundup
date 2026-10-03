// U130: the one scanner for look values (docs/design-system.md, "Tokens"; D1) that every stylesheet and inline-style test shares.

const COLOUR_PROPERTY = /(?:^|-)color$|^background(?:-color)?$|^(?:fill|stroke)$/;

const RADIUS_PROPERTY = /(?:^|-)radius$/;

const DURATION_PROPERTY = /(?:^|-)duration$|^(?:transition|animation)$/;

const SHADOW_PROPERTY = /^box-shadow$/;

const FONT_SIZE_PROPERTY = /^font(?:-size)?$/;

const COLOUR_FUNCTION = /#[0-9a-f]{3,8}\b|\b(?:rgb|hsl)a?\(/i;

const COLOUR_KEYWORDS_ALLOWED = new Set(["transparent", "inherit", "initial", "unset", "none", "currentcolor"]);

const DURATION = /\b\d*\.?\d+m?s\b/;

const FONT_LENGTH = /\b\d*\.?\d+(?:px|em|rem|pt|%)/;

// A value is a Token read when nothing is left once every `var(--…)` is taken out.
const withoutVars = (value: string): string => value.replace(/var\(--[\w-]+\)/g, " ").trim();

const isLiteral = (property: string, value: string): boolean => {
  const rest = withoutVars(value);

  if (COLOUR_FUNCTION.test(rest)) return true;

  if (COLOUR_PROPERTY.test(property)) return /[a-z]/i.test(rest) && !rest.split(/\s+/).every((word) => COLOUR_KEYWORDS_ALLOWED.has(word.toLowerCase()));

  if (RADIUS_PROPERTY.test(property)) return rest !== "" && rest !== "0";

  if (SHADOW_PROPERTY.test(property)) return rest !== "" && rest !== "none";

  if (DURATION_PROPERTY.test(property)) return DURATION.test(rest) && !/^0m?s$/.test(rest);

  if (FONT_SIZE_PROPERTY.test(property)) return FONT_LENGTH.test(rest) || (property === "font-size" && rest !== "" && rest !== "inherit");

  return false;
};

// `[;}]` or the end ends a declaration, so the last one in a rule needs no `;`. A custom property (`--x`) is a definition, not a read.
const DECLARATION = /(?:^|[\s;{])([a-z][\w-]*)\s*:\s*([^;}]+)(?=[;}]|$)/gi;

/** Every look-property declaration in `css` that is not a `var(--…)` read, as `"property: value"`. */
export function lookLiterals(css: string): string[] {
  const uncommented = css.replace(/\/\*[\s\S]*?\*\//g, "");

  return [...uncommented.matchAll(DECLARATION)].flatMap(([, property = "", value = ""]) =>
    isLiteral(property.toLowerCase(), value.trim()) ? [`${property}: ${value.trim()}`] : [],
  );
}

/**
 * The look pairs of every `style={{ … }}` object and exported `JSX.CSSProperties` object in `source`, written as CSS
 * (`"font-size": "13px"` becomes `font-size: 13px;`) so `lookLiterals` reads both with one rule.
 */
export function inlineStyleCss(source: string): string {
  const bodies = [...source.matchAll(/style=\{\{([\s\S]*?)\}\}/g), ...source.matchAll(/:\s*JSX\.CSSProperties\s*=\s*\{([^}]*)\}/g)].map((match) => match[1] ?? "");

  return bodies
    .flatMap((body) => [...body.matchAll(/"?([a-zA-Z][\w-]*)"?\s*:\s*"([^"]*)"/g)])
    .map(([, property, value]) => `${property}: ${value};`)
    .join("\n");
}
