/*
 * Tokens especiales que el traductor debe conservar: códigos de control de RPG Maker (\V[1], \C[2],
 * \{, \.), nombres compuestos (\N[1]-san), %1, {name}/<name> y el DSL de condiciones de opciones.
 *
 * Es un port fiel de `collect_special_token_spans` en
 * src-tauri/src/services/translation_integrity.rs: la UI resalta exactamente lo que el backend
 * valida. Si cambia allí, hay que cambiarlo aquí.
 */

export interface TextSegment {
  text: string;
  kind: "text" | "code";
}

const SINGLE_MARKERS = new Set(["$", ".", "|", "!", ">", "<", "^", "{", "}", "\\"]);
const CHOICE_DSL_NAMES = new Set(["if", "en", "enable", "disable", "show", "hide", "dis"]);
const CHOICE_DSL_SYMBOLS = new Set(["[", "]", "_", "-", "+", "!", "=", "<", ">", "&", "|", " "]);
const PLACEHOLDER_SYMBOLS = new Set(["_", "-", "."]);

const isAsciiAlpha = (ch: string | undefined) => ch !== undefined && /^[A-Za-z]$/.test(ch);
const isAsciiDigit = (ch: string | undefined) => ch !== undefined && /^[0-9]$/.test(ch);
const isAsciiAlnum = (ch: string | undefined) => isAsciiAlpha(ch) || isAsciiDigit(ch);

function matchControlCode(chars: string[], start: number): number | null {
  if (chars.length - start < 2 || chars[start] !== "\\") return null;

  const marker = chars[start + 1];
  if (SINGLE_MARKERS.has(marker)) return 2;
  if (!isAsciiAlpha(marker)) return null;
  if ((marker === "n" || marker === "r" || marker === "t") && chars[start + 2] !== "[") return 2;

  let cursor = 2;
  while (start + cursor < chars.length && isAsciiAlpha(chars[start + cursor])) cursor += 1;

  if (start + cursor < chars.length && chars[start + cursor] === "[") {
    cursor += 1;
    while (start + cursor < chars.length && !["]", "\n", "\r"].includes(chars[start + cursor])) {
      cursor += 1;
    }
    if (start + cursor < chars.length && chars[start + cursor] === "]") cursor += 1;
  }

  return cursor;
}

function matchNameCompound(chars: string[], start: number): number | null {
  const baseLength = matchControlCode(chars, start);
  if (baseLength === null) return null;

  const base = chars.slice(start, start + baseLength).join("");
  if (!(base.startsWith("\\N[") || base.startsWith("\\P["))) return null;
  if (chars[start + baseLength] !== "-") return null;

  let cursor = baseLength + 1;
  let sawAsciiAlpha = false;
  while (start + cursor < chars.length) {
    const ch = chars[start + cursor];
    if (isAsciiAlpha(ch)) {
      sawAsciiAlpha = true;
      cursor += 1;
      continue;
    }
    if (ch === "-") {
      cursor += 1;
      continue;
    }
    break;
  }

  return sawAsciiAlpha ? cursor : null;
}

function matchPercentPlaceholder(chars: string[], start: number): number | null {
  if (chars[start] !== "%") return null;
  let cursor = start + 1;
  while (cursor < chars.length && isAsciiDigit(chars[cursor])) cursor += 1;
  return cursor > start + 1 ? cursor - start : null;
}

function matchNamedPlaceholder(chars: string[], start: number): number | null {
  const open = chars[start];
  const close = open === "{" ? "}" : open === "<" ? ">" : null;
  if (!close) return null;

  let cursor = 1;
  while (start + cursor < chars.length) {
    const ch = chars[start + cursor];
    if (ch === close) return cursor > 1 ? cursor + 1 : null;
    if (isAsciiAlnum(ch) || PLACEHOLDER_SYMBOLS.has(ch)) {
      cursor += 1;
      continue;
    }
    return null;
  }
  return null;
}

function matchChoiceControlDsl(chars: string[], start: number): number | null {
  let nameLength = 0;
  while (nameLength < 13 && isAsciiAlpha(chars[start + nameLength])) nameLength += 1;
  if (nameLength < 2 || nameLength > 12 || chars[start + nameLength] !== "(") return null;

  const name = chars.slice(start, start + nameLength).join("").toLowerCase();
  if (!CHOICE_DSL_NAMES.has(name)) return null;

  let cursor = nameLength + 1;
  let sawBracketedOperand = false;
  while (start + cursor < chars.length) {
    const ch = chars[start + cursor];
    if (ch === "\n" || ch === "\r") return null;
    if (ch === "[") sawBracketedOperand = true;
    if (ch === ")") return sawBracketedOperand ? cursor + 1 : null;
    if (isAsciiAlnum(ch) || CHOICE_DSL_SYMBOLS.has(ch)) {
      cursor += 1;
      continue;
    }
    return null;
  }
  return null;
}

export function splitSpecialTokens(text: string): TextSegment[] {
  const chars = Array.from(text);
  const segments: TextSegment[] = [];
  let plainStart = 0;
  let index = 0;

  const pushPlain = (end: number) => {
    if (end > plainStart) segments.push({ text: chars.slice(plainStart, end).join(""), kind: "text" });
  };

  while (index < chars.length) {
    const length =
      matchChoiceControlDsl(chars, index) ??
      matchNameCompound(chars, index) ??
      (chars[index] === "\\" ? matchControlCode(chars, index) : null) ??
      matchPercentPlaceholder(chars, index) ??
      matchNamedPlaceholder(chars, index);

    if (length) {
      pushPlain(index);
      segments.push({ text: chars.slice(index, index + length).join(""), kind: "code" });
      index += length;
      plainStart = index;
      continue;
    }
    index += 1;
  }

  pushPlain(chars.length);
  return segments;
}
