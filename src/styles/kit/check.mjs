#!/usr/bin/env node
// Design kit check: the brand's OKLCH band, its perceptual distance (dE_OK) from the state colors,
// and WCAG contrast of text colors. Node built-ins only; runs with node and bun.
//
// Usage, in the kit:        node scripts/check.mjs [--brand-from <app.css>...] [--tokens <tokens.css>]
// Usage, in a vendored copy: node <dest>/check.mjs [--brand-from <app.css>...]
//
// Reads custom properties from the kit's tokens.css, then from each --brand-from file in order; a
// later declaration wins, as in one :root. @import is not followed. tokens.css is looked up next to
// this file (a vendored copy) and then one folder up (the kit's own layout). Exit code: 0 when no
// check fails (warnings included), 1 when a check fails, 2 when a file or token cannot be read.

import { existsSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const HERE = dirname(fileURLToPath(import.meta.url));
const KIT_DIR = existsSync(join(HERE, "tokens.css")) ? HERE : resolve(HERE, "..");

const RULES = {
  lightness: [0.7, 0.76],
  chroma: [0.07, 0.1],
  // Euclidean distance in OKLab between --brand and each state color.
  separation: { fail: 0.075, warn: 0.1 },
  contrast: 4.5,
};
const STATES = ["success", "danger", "warning", "info"];
const SURFACES = ["bg-0", "bg-1", "bg-2", "bg-3"];
const INKS = ["ink", "ink-2", "ink-3"];
const BRAND_SLOT = ["brand", "brand-ink", "brand-text", "brand-soft", "brand-line"];

class CheckError extends Error {}

// Arguments ---------------------------------------------------------------------------------------

function parseArgs(argv) {
  const options = { tokens: resolve(KIT_DIR, "tokens.css"), brandFrom: [], help: false };
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    if (arg === "--brand-from") {
      const start = options.brandFrom.length;
      while (i + 1 < argv.length && !argv[i + 1].startsWith("--")) options.brandFrom.push(argv[++i]);
      if (options.brandFrom.length === start) throw new CheckError("--brand-from needs at least one CSS file");
    } else if (arg === "--tokens") {
      if (i + 1 >= argv.length) throw new CheckError("--tokens needs a file");
      options.tokens = argv[++i];
    } else if (arg === "-h" || arg === "--help") {
      options.help = true;
    } else {
      throw new CheckError(`unknown argument: ${arg}`);
    }
  }
  return options;
}

// CSS custom properties ---------------------------------------------------------------------------

/** Custom property declarations of a stylesheet, in source order. Selectors such as
 *  `.button--brand:hover` are not declarations: a declaration starts after `{`, `;` or `}`. */
function customProperties(css) {
  const text = css.replace(/\/\*[\s\S]*?\*\//g, " ");
  const found = [];
  const start = /([{;}])\s*(--[A-Za-z0-9_-]+)\s*:/g;
  let match;
  while ((match = start.exec(text))) {
    let depth = 0;
    let quote = null;
    let end = start.lastIndex;
    for (; end < text.length; end++) {
      const char = text[end];
      if (quote) {
        if (char === "\\") end++;
        else if (char === quote) quote = null;
      } else if (char === '"' || char === "'") quote = char;
      else if (char === "(") depth++;
      else if (char === ")") depth--;
      else if (depth <= 0 && (char === ";" || char === "}" || char === "{")) break;
    }
    const value = text
      .slice(start.lastIndex, end)
      .replace(/!important\s*$/i, "")
      .trim();
    found.push([match[2], value]);
    start.lastIndex = end;
  }
  return found;
}

function readTokens(files) {
  const tokens = new Map();
  files.forEach((file, index) => {
    let css;
    try {
      css = readFileSync(file, "utf8");
    } catch (error) {
      throw new CheckError(`cannot read ${file}: ${error.message}`);
    }
    for (const [name, value] of customProperties(css)) tokens.set(name, { value, source: index });
  });
  return tokens;
}

function topLevelComma(text) {
  let depth = 0;
  for (let i = 0; i < text.length; i++) {
    if (text[i] === "(") depth++;
    else if (text[i] === ")") depth--;
    else if (text[i] === "," && depth === 0) return i;
  }
  return -1;
}

/** Replaces every var() in a value, fallbacks and nesting included. */
function substitute(value, tokens, stack) {
  let out = "";
  let index = 0;
  const pattern = /\bvar\(/g;
  let match;
  while ((match = pattern.exec(value))) {
    out += value.slice(index, match.index);
    let depth = 1;
    let end = pattern.lastIndex;
    for (; end < value.length && depth > 0; end++) {
      if (value[end] === "(") depth++;
      else if (value[end] === ")") depth--;
    }
    if (depth !== 0) throw new CheckError(`${stack.at(-1)}: unbalanced var() in "${value}"`);
    const inner = value.slice(pattern.lastIndex, end - 1);
    const comma = topLevelComma(inner);
    const reference = (comma < 0 ? inner : inner.slice(0, comma)).trim();
    const fallback = comma < 0 ? null : inner.slice(comma + 1).trim();
    if (tokens.has(reference)) out += resolveToken(reference, tokens, stack).value;
    else if (fallback !== null) out += substitute(fallback, tokens, stack);
    else throw new CheckError(`${stack.at(-1)} uses ${reference}, which no file defines`);
    index = end;
    pattern.lastIndex = end;
  }
  return out + value.slice(index);
}

function resolveToken(name, tokens, stack = []) {
  if (stack.includes(name)) throw new CheckError(`circular reference: ${[...stack, name].join(" -> ")}`);
  const entry = tokens.get(name);
  if (!entry) throw new CheckError(`${name} is not defined in any file`);
  return { value: substitute(entry.value, tokens, [...stack, name]).trim(), source: entry.source };
}

// Colors ------------------------------------------------------------------------------------------

const NAMED = {
  transparent: [0, 0, 0, 0],
  black: [0, 0, 0, 1],
  white: [1, 1, 1, 1],
};

function number(text, percentScale = 1) {
  const value = text.trim().toLowerCase();
  if (value === "none") return 0;
  if (value.endsWith("%")) return (parseFloat(value) / 100) * percentScale;
  const parsed = parseFloat(value);
  if (!Number.isFinite(parsed)) throw new CheckError(`not a number: ${text}`);
  return parsed;
}

function angle(text) {
  const value = text.trim().toLowerCase();
  if (value === "none") return 0;
  const parsed = parseFloat(value);
  if (value.endsWith("turn")) return parsed * 360;
  if (value.endsWith("grad")) return parsed * 0.9;
  if (value.endsWith("rad")) return (parsed * 180) / Math.PI;
  return parsed;
}

function channels(body) {
  let parts;
  let alpha = null;
  if (body.includes(",")) {
    parts = body.split(",").map((part) => part.trim());
    if (parts.length === 4) alpha = parts.pop();
  } else {
    const [main, rest] = body.split("/");
    parts = main.trim().split(/\s+/);
    if (rest !== undefined) alpha = rest.trim();
  }
  if (parts.length !== 3) throw new CheckError(`expected three channels in "${body}"`);
  return { parts, alpha: alpha === null ? 1 : clamp(number(alpha), 0, 1) };
}

const clamp = (value, min, max) => Math.min(max, Math.max(min, value));
const toLinear = (c) => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);
const toGamma = (c) => (c <= 0.0031308 ? 12.92 * c : 1.055 * c ** (1 / 2.4) - 0.055);

/** A CSS color as sRGB channels in 0..1 plus alpha. OKLCH and OKLab outside sRGB are clipped. */
function parseColor(text) {
  const value = text.trim().toLowerCase();
  if (NAMED[value]) {
    const [r, g, b, a] = NAMED[value];
    return { r, g, b, a };
  }
  const hex = /^#([0-9a-f]{3,8})$/.exec(value);
  if (hex) {
    let digits = hex[1];
    if (digits.length === 3 || digits.length === 4) digits = [...digits].map((d) => d + d).join("");
    if (digits.length !== 6 && digits.length !== 8) throw new CheckError(`bad hex color: ${text}`);
    const byte = (i) => parseInt(digits.slice(i, i + 2), 16) / 255;
    return { r: byte(0), g: byte(2), b: byte(4), a: digits.length === 8 ? byte(6) : 1 };
  }
  const fn = /^(rgba?|hsla?|oklch|oklab)\((.*)\)$/.exec(value);
  if (!fn) throw new CheckError(`unsupported color syntax: ${text}`);
  const { parts, alpha } = channels(fn[2]);
  switch (fn[1]) {
    case "rgb":
    case "rgba": {
      const [r, g, b] = parts.map((part) => clamp(number(part, 255) / 255, 0, 1));
      return { r, g, b, a: alpha };
    }
    case "hsl":
    case "hsla": {
      const h = ((angle(parts[0]) % 360) + 360) % 360;
      const s = clamp(number(parts[1], 100) / 100, 0, 1);
      const l = clamp(number(parts[2], 100) / 100, 0, 1);
      const k = (n) => (n + h / 30) % 12;
      const f = (n) => l - s * Math.min(l, 1 - l) * Math.max(-1, Math.min(k(n) - 3, 9 - k(n), 1));
      return { r: f(0), g: f(8), b: f(4), a: alpha };
    }
    case "oklch": {
      const L = number(parts[0], 1);
      const C = number(parts[1], 0.4);
      const H = (angle(parts[2]) * Math.PI) / 180;
      return { ...fromOklab(L, C * Math.cos(H), C * Math.sin(H)), a: alpha };
    }
    case "oklab":
      return { ...fromOklab(number(parts[0], 1), number(parts[1], 0.4), number(parts[2], 0.4)), a: alpha };
  }
  throw new CheckError(`unsupported color syntax: ${text}`);
}

/** sRGB (0..1, gamma encoded) to OKLab, with Bjorn Ottosson's matrices. */
function toOklab({ r, g, b }) {
  const [lr, lg, lb] = [r, g, b].map(toLinear);
  const l = Math.cbrt(0.4122214708 * lr + 0.5363325363 * lg + 0.0514459929 * lb);
  const m = Math.cbrt(0.2119034982 * lr + 0.6806995451 * lg + 0.1073969566 * lb);
  const s = Math.cbrt(0.0883024619 * lr + 0.2817188376 * lg + 0.6299787005 * lb);
  return {
    L: 0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s,
    a: 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s,
    b: 0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s,
  };
}

function fromOklab(L, a, b) {
  const l = (L + 0.3963377774 * a + 0.2158037573 * b) ** 3;
  const m = (L - 0.1055613458 * a - 0.0638541728 * b) ** 3;
  const s = (L - 0.0894841775 * a - 1.291485548 * b) ** 3;
  const linear = [
    4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
    -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
    -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s,
  ];
  const [r, g, bl] = linear.map((c) => clamp(toGamma(clamp(c, 0, 1)), 0, 1));
  return { r, g, b: bl };
}

function toOklch(color) {
  const { L, a, b } = toOklab(color);
  const C = Math.hypot(a, b);
  const H = ((Math.atan2(b, a) * 180) / Math.PI + 360) % 360;
  return { L, C, H };
}

/** Paints a translucent color over an opaque one, in gamma-encoded sRGB like the browser. */
function over(top, bottom) {
  const mix = (t, u) => t * top.a + u * (1 - top.a);
  return { r: mix(top.r, bottom.r), g: mix(top.g, bottom.g), b: mix(top.b, bottom.b), a: 1 };
}

const luminance = ({ r, g, b }) => 0.2126 * toLinear(r) + 0.7152 * toLinear(g) + 0.0722 * toLinear(b);

function contrast(first, second) {
  const [high, low] = [luminance(first), luminance(second)].sort((x, y) => y - x);
  return (high + 0.05) / (low + 0.05);
}

const hueDistance = (x, y) => {
  const d = Math.abs(x - y) % 360;
  return d > 180 ? 360 - d : d;
};

function hexOf({ r, g, b, a }) {
  const byte = (c) => Math.round(c * 255).toString(16).padStart(2, "0");
  return `#${byte(r)}${byte(g)}${byte(b)}${a < 1 ? byte(a) : ""}`.toUpperCase();
}

// Report ------------------------------------------------------------------------------------------

function table(rows, headers) {
  const widths = headers.map((header, i) => Math.max(header.length, ...rows.map((row) => String(row[i]).length)));
  const line = (cells) => cells.map((cell, i) => String(cell).padEnd(widths[i])).join("   ").trimEnd();
  return [line(headers), line(widths.map((w) => "-".repeat(w))), ...rows.map(line)].join("\n");
}

/** A vendored copy has KIT_VERSION (semver + hash); the kit itself has package.json. */
function kitVersion() {
  try {
    return readFileSync(join(KIT_DIR, "KIT_VERSION"), "utf8").trim();
  } catch {
    try {
      return JSON.parse(readFileSync(join(KIT_DIR, "package.json"), "utf8")).version;
    } catch {
      return "unknown";
    }
  }
}

/** Euclidean distance in OKLab. */
const deltaE = (x, y) => Math.hypot(x.L - y.L, x.a - y.a, x.b - y.b);

function run(options) {
  const files = [resolve(options.tokens), ...options.brandFrom.map((file) => resolve(file))];
  const tokens = readTokens(files);
  const color = (name) => {
    const token = resolveToken(`--${name}`, tokens);
    return { name, ...token, color: parseColor(token.value) };
  };
  const page = color("bg-0");
  if (page.color.a < 1) throw new CheckError("--bg-0 must be opaque");
  const solid = (entry) => (entry.color.a < 1 ? over(entry.color, page.color) : entry.color);

  const surfaces = SURFACES.map(color);
  const states = STATES.map(color);
  const brand = color("brand");
  const brandInk = color("brand-ink");
  const brandText = color("brand-text");
  const inks = INKS.map(color);
  const brandSetByApp = tokens.get("--brand").source > 0;

  const out = [];
  out.push(`Design kit check - kit ${kitVersion()}`, "");
  out.push("Sources (a later file wins):");
  files.forEach((file, i) => out.push(`  [${i + 1}] ${file}${i === 0 ? "  (kit tokens)" : "  (app)"}`));
  out.push("");

  const resolved = [...surfaces, ...states, brand, brandInk, brandText, ...inks].map((entry) => {
    const lch = toOklch(solid(entry));
    return [
      `--${entry.name}`,
      hexOf(entry.color),
      `oklch(${lch.L.toFixed(3)} ${lch.C.toFixed(3)} ${lch.C < 1e-4 ? "none" : lch.H.toFixed(1)})`,
      `[${entry.source + 1}]`,
    ];
  });
  out.push("Resolved tokens:");
  out.push(table(resolved, ["Token", "Value", "OKLCH", "From"]).replace(/^/gm, "  "), "");

  const rows = [];
  const count = { ok: 0, WARN: 0, FAIL: 0 };
  const record = (rule, value, target, status) => {
    count[status]++;
    rows.push([rule, value, target, status]);
  };
  const check = (rule, value, target, pass) => record(rule, value, target, pass ? "ok" : "FAIL");

  const brandLab = toOklab(solid(brand));
  const brandLch = toOklch(solid(brand));
  const [minL, maxL] = RULES.lightness;
  const [minC, maxC] = RULES.chroma;
  const { fail, warn } = RULES.separation;
  if (brandSetByApp) {
    check("brand lightness (OKLCH L)", brandLch.L.toFixed(3), `${minL.toFixed(2)}-${maxL.toFixed(2)}`, brandLch.L >= minL && brandLch.L <= maxL);
    check("brand chroma (OKLCH C)", brandLch.C.toFixed(3), `${minC.toFixed(2)}-${maxC.toFixed(2)}`, brandLch.C >= minC && brandLch.C <= maxC);
    // Perceptual distance, not hue alone: lightness and chroma separate colors of the same hue.
    for (const state of states) {
      const stateLab = toOklab(solid(state));
      const stateLch = toOklch(solid(state));
      const distance = deltaE(brandLab, stateLab);
      const context = [
        `dL ${Math.abs(brandLch.L - stateLch.L).toFixed(3)}`,
        `dC ${Math.abs(brandLch.C - stateLch.C).toFixed(3)}`,
        `dh ${hueDistance(brandLch.H, stateLch.H).toFixed(1)} deg`,
      ].join(", ");
      record(
        `brand vs ${state.name} (dE_OK)`,
        `${distance.toFixed(3)} (${context})`,
        `ok >= ${warn.toFixed(2)}, fail < ${fail}`,
        distance < fail ? "FAIL" : distance < warn ? "WARN" : "ok",
      );
    }
  } else {
    rows.push(["brand band and separation", "-", "-", "skipped: no app file sets --brand (neutral fallback)"]);
  }

  const target = `>= ${RULES.contrast}:1`;
  const contrastRow = (fg, bg) => {
    const ratio = contrast(over(fg.color, solid(bg)), solid(bg));
    check(`${fg.name} on ${bg.name}`, `${ratio.toFixed(2)}:1`, target, ratio >= RULES.contrast);
  };
  for (const surface of surfaces) contrastRow(brandText, surface);
  for (const state of states) for (const surface of surfaces) contrastRow(state, surface);
  contrastRow(brandInk, brand);

  out.push("Checks:");
  out.push(table(rows, ["Rule", "Value", "Target", "Result"]).replace(/^/gm, "  "));
  if (count.WARN) {
    out.push(
      `  WARN: a dE_OK from ${fail} up to ${warn.toFixed(2)} means related colors. It does not fail;`,
      "  use them in roles that never cross (see DESIGN.md, Separation from the states).",
    );
  }
  out.push("");

  const notes = inks.map((ink) => {
    const worst = surfaces
      .map((surface) => ({ surface, ratio: contrast(over(ink.color, solid(surface)), solid(surface)) }))
      .sort((x, y) => x.ratio - y.ratio)[0];
    return [`${ink.name} on bg-0 to bg-3`, `min ${worst.ratio.toFixed(2)}:1 (${worst.surface.name})`];
  });
  out.push("Informational (never fails):");
  out.push(table(notes, ["Text", "Contrast"]).replace(/^/gm, "  "));
  if (brandSetByApp) {
    const fallback = BRAND_SLOT.filter((name) => tokens.get(`--${name}`).source === 0);
    if (fallback.length) out.push(`  Brand tokens left on the kit's neutral fallback: ${fallback.map((n) => `--${n}`).join(", ")}`);
  }
  out.push("");

  const total = count.ok + count.WARN + count.FAIL;
  const warnings = `${count.WARN} warning${count.WARN === 1 ? "" : "s"}`;
  out.push(`Result: ${total} checks: ${count.ok} ok, ${warnings}, ${count.FAIL} failed.`);
  return { text: out.join("\n"), failures: count.FAIL };
}

try {
  const options = parseArgs(process.argv.slice(2));
  if (options.help) {
    console.log("Usage: node check.mjs [--brand-from <app.css>...] [--tokens <tokens.css>]");
  } else {
    const { text, failures } = run(options);
    console.log(text);
    process.exitCode = failures ? 1 : 0;
  }
} catch (error) {
  if (!(error instanceof CheckError)) throw error;
  console.error(`check: ${error.message}`);
  process.exitCode = 2;
}
