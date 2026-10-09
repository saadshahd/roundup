import * as v from "valibot";
import { storedValue } from "../state/storage";
import "../tokens.css";

const tokens = getComputedStyle(document.documentElement);

export const DEFAULT_FONT_SIZE = Number.parseFloat(tokens.getPropertyValue("--text-body"));

export const MIN_FONT_SIZE = Number.parseFloat(tokens.getPropertyValue("--text-terminal-min"));

export const MAX_FONT_SIZE = Number.parseFloat(tokens.getPropertyValue("--text-terminal-max"));

const fontSizeSchema = v.pipe(v.number(), v.minValue(MIN_FONT_SIZE), v.maxValue(MAX_FONT_SIZE));

export const fontSizeStorage = (path: string) => storedValue(`roundup:terminal-font:${path}`, fontSizeSchema);
