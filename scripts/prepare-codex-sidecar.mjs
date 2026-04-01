import { chmodSync, copyFileSync, existsSync, mkdirSync } from "node:fs";
import { dirname, extname, join, resolve } from "node:path";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const binariesDir = join(repoRoot, "src-tauri", "binaries");

function run(command, args) {
  return execFileSync(command, args, {
    cwd: repoRoot,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  }).trim();
}

function resolveTargetTriple() {
  const explicit =
    process.env.FAIRTRANSLATOR_TAURI_TARGET ||
    process.env.TAURI_BUILD_TARGET ||
    process.env.CARGO_BUILD_TARGET;
  if (explicit) return explicit.trim();

  const rustcVersion = run("rustc", ["-vV"]);
  const hostLine = rustcVersion
    .split(/\r?\n/)
    .find((line) => line.startsWith("host: "));
  if (!hostLine) {
    throw new Error("Could not determine the Rust host triple from `rustc -vV`.");
  }
  return hostLine.replace("host: ", "").trim();
}

function resolveCodexBinary() {
  const explicit = process.env.FAIRTRANSLATOR_CODEX_BINARY;
  if (explicit) {
    const candidate = resolve(explicit);
    if (!existsSync(candidate)) {
      throw new Error(`FAIRTRANSLATOR_CODEX_BINARY does not exist: ${candidate}`);
    }
    return candidate;
  }

  const locator = process.platform === "win32" ? "where" : "which";
  const output = run(locator, ["codex"]);
  const firstMatch = output.split(/\r?\n/).find(Boolean);
  if (!firstMatch) {
    throw new Error(
      "Could not find `codex` on PATH. Install Codex first or set FAIRTRANSLATOR_CODEX_BINARY.",
    );
  }

  return resolve(firstMatch.trim());
}

function buildSidecarFilename(targetTriple, sourceBinaryPath) {
  const extension = extname(sourceBinaryPath);
  return `codex-${targetTriple}${extension}`;
}

function main() {
  const targetTriple = resolveTargetTriple();
  const sourceBinary = resolveCodexBinary();
  const outputFilename = buildSidecarFilename(targetTriple, sourceBinary);
  const outputPath = join(binariesDir, outputFilename);

  mkdirSync(binariesDir, { recursive: true });
  copyFileSync(sourceBinary, outputPath);
  if (process.platform !== "win32") {
    chmodSync(outputPath, 0o755);
  }

  console.log(`Copied Codex sidecar to ${outputPath}`);
  console.log(`Target triple: ${targetTriple}`);
}

main();
