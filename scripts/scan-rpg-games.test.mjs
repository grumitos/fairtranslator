import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import test from "node:test";

const scanner = path.resolve("scripts/scan-rpg-games.mjs");

function tempRoot(label) {
  return fs.mkdtempSync(path.join(os.tmpdir(), `fairtl-scan-${label}-`));
}

function writeFile(root, relativePath, content = "") {
  const filePath = path.join(root, relativePath);
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(filePath, content);
}

function scan(root, maxDepth = 4) {
  const result = spawnSync(
    process.execPath,
    [scanner, root, "--json", "--max-depth", String(maxDepth)],
    { encoding: "utf8" },
  );
  assert.equal(result.status, 0, result.stderr);
  return JSON.parse(result.stdout).results;
}

test("Unity detection requires globalgamemanagers, not an empty _Data folder", () => {
  const root = tempRoot("unity-negative");
  try {
    fs.mkdirSync(path.join(root, "Empty_Data"));
    writeFile(root, "UnityPlayer.dll");

    assert.equal(scan(root, 0).some((result) => result.engine === "Unity"), false);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test("Unity inventory finds globalgamemanagers even without UnityPlayer.dll", () => {
  const root = tempRoot("unity-positive");
  try {
    writeFile(root, "Sample_Data/globalgamemanagers");

    const unity = scan(root, 0).find((result) => result.engine === "Unity");
    assert.ok(unity);
    assert.ok(unity.markers.includes("Sample_Data/globalgamemanagers"));
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test("low-confidence parent detection does not hide a nested supported game", () => {
  const root = tempRoot("nested-mz");
  try {
    writeFile(root, "options.ini");
    writeFile(root, "NestedGame/js/rmmz_core.js");
    writeFile(root, "NestedGame/data/System.json", "{}");

    const results = scan(root);
    assert.ok(results.some((result) => result.engine === "GameMaker"));
    assert.ok(results.some((result) => result.engine === "RPG Maker MZ"));
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});
