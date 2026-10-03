// U130: the one scanner for look values (docs/design-system.md, "Tokens"; D1) that every stylesheet and inline-style test shares.

const COLOUR_PROPERTY = /(?:^|-)color$|^background(?:-color)?$|^(?:fill|stroke)$/;

const RADIUS_PROPERTY = /(?:^|-)radius$/;

const DURATION_PROPERTY = /(?:^|-)(?:duration|delay)$|^(?:transition|animation)$/;

const SHADOW_PROPERTY = /^box-shadow$/;

const FONT_SIZE_PROPERTY = /^font(?:-size)?$/;

const COLOUR_FUNCTION = /#[0-9a-f]{3,8}\b|\b(?:rgba?|hsla?|oklch|oklab|lab|lch|hwb|color-mix|color)\(/i;

// Shorthands whose colour is one word among others (`2px solid red`).
const COLOUR_SHORTHAND = /^(?:border(?:-(?:top|right|bottom|left|block|inline)(?:-(?:start|end))?)?|outline|text-shadow|text-decoration|column-rule|background-image)$/;

const NOT_A_COLOUR = new Set(["solid", "dashed", "dotted", "double", "none", "hidden", "inset", "outset", "groove", "ridge", "underline", "overline", "line-through", "wavy", "auto", "to", "top", "right", "bottom", "left", "center", "circle", "ellipse", "at", "inherit", "initial", "unset", "transparent", "currentcolor"]);

// Custom properties U130 slice 3 deletes; until then they hold the two colours the table does not have.
const UNTIL_SLICE_3 = new Set(["--light", "--lightest"]);

const COLOUR_KEYWORDS_ALLOWED = new Set(["transparent", "inherit", "initial", "unset", "none", "currentcolor"]);

// A time written as digits, or as a template placeholder with a unit (`${ms}ms`, which `inlineStyleCss` writes as `⟨expr⟩ms`), which no scan can read.
const DURATION = /\b\d*\.?\d+m?s\b|⟩m?s\b/;

const FONT_LENGTH = /\b\d*\.?\d+(?:px|em|rem|pt|%)/;

// A value is a Token read when nothing is left once every `var(--…)` is taken out.
const withoutVars = (value: string): string => value.replace(/var\(--[\w-]+\)/g, " ").trim();

const isLiteral = (property: string, value: string): boolean => {
  const rest = withoutVars(value);

  if (property.startsWith("--")) return !UNTIL_SLICE_3.has(property) && (COLOUR_FUNCTION.test(rest) || DURATION.test(rest));

  if (COLOUR_FUNCTION.test(rest)) return true;

  if (COLOUR_SHORTHAND.test(property)) return rest.split(/[\s,()]+/).some((word) => /^[a-z]+$/i.test(word) && !NOT_A_COLOUR.has(word.toLowerCase()));

  if (COLOUR_PROPERTY.test(property)) return /[a-z]/i.test(rest) && !rest.split(/\s+/).every((word) => COLOUR_KEYWORDS_ALLOWED.has(word.toLowerCase()));

  if (RADIUS_PROPERTY.test(property)) return rest !== "" && rest !== "0";

  if (SHADOW_PROPERTY.test(property)) return rest !== "" && rest !== "none";

  if (DURATION_PROPERTY.test(property)) return DURATION.test(rest) && !/^0m?s$/.test(rest);

  if (FONT_SIZE_PROPERTY.test(property)) return FONT_LENGTH.test(rest) || (property === "font-size" && rest !== "" && rest !== "inherit");

  return false;
};

// `[;}]` or the end ends a declaration, so the last one in a rule needs no `;`. A custom property is a definition, so it is flagged only when it holds a colour function or a time.
const DECLARATION = /(?:^|[\s;{])(-{0,2}[a-z][\w-]*)\s*:\s*([^;{}]+)(?=[;}]|$)/gi;

/** Every look-property declaration in `css` that is not a `var(--…)` read, as `"property: value"`. */
export function lookLiterals(css: string): string[] {
  const uncommented = css.replace(/\/\*[\s\S]*?\*\//g, "");

  return [...uncommented.matchAll(DECLARATION)].flatMap(([, property = "", value = ""]) =>
    isLiteral(property.toLowerCase(), value.trim()) ? [`${property}: ${value.trim()}`] : [],
  );
}

// A value ends at a comma or newline outside a string, except before a line starting with `?` or `:`, so a ternary or a template literal stays whole.
const INLINE_PAIR = /"?([a-zA-Z][\w-]*)"?\s*:\s*((?:"(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'|`(?:[^`\\]|\\.)*`|\n\s*(?=[?:])|[^,\n"'`])+)/g;

/**
 * The look pairs of every `style={{ … }}` object and exported `JSX.CSSProperties` object in `source`, written as CSS
 * (`"font-size": "13px"` becomes `font-size: 13px;`) so `lookLiterals` reads both with one rule.
 */
export function inlineStyleCss(source: string): string {
  const bodies = [...source.matchAll(/style=\{\{([\s\S]*?)\}\}/g), ...source.matchAll(/:\s*JSX\.CSSProperties\s*=\s*\{([^}]*)\}/g)].map((match) => match[1] ?? "");

  return bodies
    .flatMap((body) => [...body.matchAll(INLINE_PAIR)])
    .map(([, property, value = ""]) => `${property}: ${value.trim().replace(/^(["'`])([\s\S]*)\1$/, "$2").replace(/\$\{[^}]*\}/g, "⟨expr⟩")};`)
    .join("\n");
}
