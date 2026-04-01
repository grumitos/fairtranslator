#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";

const DEFAULT_MAX_DEPTH = 6;
const IGNORED_DIRS = new Set([
  ".git",
  ".idea",
  ".vscode",
  ".fairtl",
  "build",
  "dist",
  "node_modules",
  "target",
  "__pycache__",
]);

const SUPPORTED_ENGINES = [
  "RPG Maker MZ",
  "RPG Maker MV",
  "RPG Maker VX Ace",
  "RPG Maker VX",
  "RPG Maker XP",
];

function parseArgs(argv) {
  const options = {
    root: process.cwd(),
    maxDepth: DEFAULT_MAX_DEPTH,
    json: false,
    includeMarkers: true,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];

    if (arg === "--json") {
      options.json = true;
      continue;
    }

    if (arg === "--no-markers") {
      options.includeMarkers = false;
      continue;
    }

    if (arg === "--max-depth") {
      const value = argv[index + 1];
      if (!value) {
        throw new Error("Missing value for --max-depth");
      }
      const parsed = Number.parseInt(value, 10);
      if (!Number.isInteger(parsed) || parsed < 0) {
        throw new Error(`Invalid --max-depth value: ${value}`);
      }
      options.maxDepth = parsed;
      index += 1;
      continue;
    }

    if (arg === "--help" || arg === "-h") {
      printHelp();
      process.exit(0);
    }

    if (arg.startsWith("--")) {
      throw new Error(`Unknown option: ${arg}`);
    }

    options.root = path.resolve(arg);
  }

  return options;
}

function printHelp() {
  console.log(`Uso:
  node scripts/scan-rpg-games.mjs [ruta]

Opciones:
  --json               Imprime el resultado en JSON
  --max-depth N        Profundidad maxima de escaneo (default: ${DEFAULT_MAX_DEPTH})
  --no-markers         Oculta las firmas detectadas en la salida de texto
  --help, -h           Muestra esta ayuda

Motores soportados por el proyecto:
  - ${SUPPORTED_ENGINES.join("\n  - ")}

Motores no soportados detectados por heuristica:
  - RPG Maker 2000/2003
  - Wolf RPG Editor
  - Kirikiri / KAG
  - TyranoScript / TyranoBuilder
  - Ren'Py
  - Unity
  - Godot
  - GameMaker
`);
}

function safeReadDir(dir) {
  try {
    return fs.readdirSync(dir, { withFileTypes: true });
  } catch {
    return [];
  }
}

function buildIndex(dir) {
  const directories = new Map();
  const files = new Map();

  for (const entry of safeReadDir(dir)) {
    const key = entry.name.toLowerCase();
    if (entry.isDirectory()) {
      directories.set(key, entry.name);
    } else if (entry.isFile()) {
      files.set(key, entry.name);
    }
  }

  return { directories, files };
}

function resolveCaseInsensitive(dir, ...segments) {
  let currentDir = dir;

  for (let index = 0; index < segments.length; index += 1) {
    const segment = segments[index];
    const lookup = segment.toLowerCase();
    const snapshot = buildIndex(currentDir);
    const isLast = index === segments.length - 1;

    if (isLast) {
      const fileName = snapshot.files.get(lookup);
      if (fileName) {
        return path.join(currentDir, fileName);
      }

      const dirName = snapshot.directories.get(lookup);
      if (dirName) {
        return path.join(currentDir, dirName);
      }

      return null;
    }

    const dirName = snapshot.directories.get(lookup);
    if (!dirName) {
      return null;
    }
    currentDir = path.join(currentDir, dirName);
  }

  return currentDir;
}

function hasFile(dir, ...segments) {
  const resolved = resolveCaseInsensitive(dir, ...segments);
  return resolved ? safeStat(resolved)?.isFile() ?? false : false;
}

function hasDirectory(dir, ...segments) {
  const resolved = resolveCaseInsensitive(dir, ...segments);
  return resolved ? safeStat(resolved)?.isDirectory() ?? false : false;
}

function safeStat(target) {
  try {
    return fs.statSync(target);
  } catch {
    return null;
  }
}

function listFiles(dir, ...segments) {
  const target = resolveCaseInsensitive(dir, ...segments);
  if (!target) {
    return [];
  }

  const stats = safeStat(target);
  if (!stats?.isDirectory()) {
    return [];
  }

  return safeReadDir(target)
    .filter((entry) => entry.isFile())
    .map((entry) => entry.name);
}

function listDirs(dir) {
  return safeReadDir(dir)
    .filter((entry) => entry.isDirectory())
    .map((entry) => entry.name);
}

function hasTopLevelFileByName(dir, fileName) {
  return buildIndex(dir).files.has(fileName.toLowerCase());
}

function findTopLevelFile(dir, predicate) {
  const snapshot = buildIndex(dir);
  for (const fileName of snapshot.files.values()) {
    if (predicate(fileName)) {
      return fileName;
    }
  }
  return null;
}

function findTopLevelDir(dir, predicate) {
  const snapshot = buildIndex(dir);
  for (const dirName of snapshot.directories.values()) {
    if (predicate(dirName)) {
      return dirName;
    }
  }
  return null;
}

function hasAnyFileWithExtension(dir, extension, ...segments) {
  return listFiles(dir, ...segments).some((fileName) =>
    fileName.toLowerCase().endsWith(extension.toLowerCase()),
  );
}

function detectGame(dir) {
  const supported = detectSupportedGame(dir);
  if (supported) {
    return supported;
  }

  return detectUnsupportedGame(dir);
}

function detectSupportedGame(dir) {
  if (hasFile(dir, "js", "rmmz_core.js") && hasDirectory(dir, "data")) {
    return makeResult("RPG Maker MZ", true, "high", ["js/rmmz_core.js", "data/"]);
  }

  if (hasFile(dir, "js", "rpg_core.js") && hasDirectory(dir, "data")) {
    return makeResult("RPG Maker MV", true, "high", ["js/rpg_core.js", "data/"]);
  }

  if (hasFile(dir, "Data", "System.rvdata2") || hasAnyFileWithExtension(dir, ".rvdata2", "Data")) {
    return makeResult("RPG Maker VX Ace", true, "high", collectLegacyMarkers(dir, "Data", ".rvdata2"));
  }

  if (hasFile(dir, "Data", "System.rvdata") || hasAnyFileWithExtension(dir, ".rvdata", "Data")) {
    return makeResult("RPG Maker VX", true, "high", collectLegacyMarkers(dir, "Data", ".rvdata"));
  }

  if (hasFile(dir, "Data", "System.rxdata") || hasAnyFileWithExtension(dir, ".rxdata", "Data")) {
    return makeResult("RPG Maker XP", true, "high", collectLegacyMarkers(dir, "Data", ".rxdata"));
  }

  return null;
}

function collectLegacyMarkers(dir, dataDirName, extension) {
  const markers = [];
  const systemName = `System${extension}`;

  if (hasFile(dir, dataDirName, systemName)) {
    markers.push(`${dataDirName}/${systemName}`);
  }
  if (hasAnyFileWithExtension(dir, extension, dataDirName)) {
    markers.push(`${dataDirName}/*${extension}`);
  }

  return unique(markers);
}

function detectUnsupportedGame(dir) {
  const rm2kOr2k3 = detectRpgMaker200x(dir);
  if (rm2kOr2k3) {
    return rm2kOr2k3;
  }

  const wolf = detectWolf(dir);
  if (wolf) {
    return wolf;
  }

  const kirikiri = detectKirikiri(dir);
  if (kirikiri) {
    return kirikiri;
  }

  const tyrano = detectTyrano(dir);
  if (tyrano) {
    return tyrano;
  }

  const renpy = detectRenPy(dir);
  if (renpy) {
    return renpy;
  }

  const unity = detectUnity(dir);
  if (unity) {
    return unity;
  }

  const godot = detectGodot(dir);
  if (godot) {
    return godot;
  }

  const gamemaker = detectGameMaker(dir);
  if (gamemaker) {
    return gamemaker;
  }

  return null;
}

function detectRpgMaker200x(dir) {
  const markers = [];
  for (const fileName of ["RPG_RT.ldb", "RPG_RT.lmt", "RPG_RT.ini", "RPG_RT.exe"]) {
    if (hasTopLevelFileByName(dir, fileName)) {
      markers.push(fileName);
    }
  }
  if (findTopLevelFile(dir, (name) => /^map\d+\.lmu$/i.test(name))) {
    markers.push("Map*.lmu");
  }
  if (markers.length === 0) {
    return null;
  }
  return makeResult("RPG Maker 2000/2003", false, "medium", markers);
}

function detectWolf(dir) {
  const markers = [];
  if (hasTopLevelFileByName(dir, "Data.wolf")) {
    markers.push("Data.wolf");
  }
  if (hasFile(dir, "Data", "BasicData", "Game.dat")) {
    markers.push("Data/BasicData/Game.dat");
  }
  if (hasFile(dir, "Data", "BasicData", "BasicData.dat")) {
    markers.push("Data/BasicData/BasicData.dat");
  }
  if (markers.length === 0) {
    return null;
  }
  return makeResult("Wolf RPG Editor", false, "medium", markers);
}

function detectKirikiri(dir) {
  const markers = [];
  if (findTopLevelFile(dir, (name) => name.toLowerCase().endsWith(".xp3"))) {
    markers.push("*.xp3");
  }
  for (const fileName of ["config.tjs", "startup.tjs"]) {
    if (hasTopLevelFileByName(dir, fileName)) {
      markers.push(fileName);
    }
  }
  if (findTopLevelFile(dir, (name) => /^krkr.*\.exe$/i.test(name))) {
    markers.push("krkr*.exe");
  }
  if (markers.length < 2) {
    return null;
  }
  return makeResult("Kirikiri / KAG", false, "medium", markers);
}

function detectTyrano(dir) {
  const markers = [];
  if (hasDirectory(dir, "tyrano")) {
    markers.push("tyrano/");
  }
  if (hasDirectory(dir, "data", "scenario")) {
    markers.push("data/scenario/");
  }
  if (hasFile(dir, "tyrano", "plugins", "kag", "kag.js")) {
    markers.push("tyrano/plugins/kag/kag.js");
  }
  if (!markers.includes("data/scenario/")) {
    const hasKs = hasAnyFileWithExtension(dir, ".ks", "data", "scenario");
    if (hasKs) {
      markers.push("data/scenario/*.ks");
    }
  }
  if (markers.length < 2) {
    return null;
  }
  return makeResult("TyranoScript / TyranoBuilder", false, "medium", markers);
}

function detectRenPy(dir) {
  const markers = [];
  if (hasDirectory(dir, "renpy")) {
    markers.push("renpy/");
  }
  if (hasDirectory(dir, "game")) {
    markers.push("game/");
  }
  if (hasAnyFileWithExtension(dir, ".rpa", "game")) {
    markers.push("game/*.rpa");
  }
  if (hasAnyFileWithExtension(dir, ".rpy", "game")) {
    markers.push("game/*.rpy");
  }
  if (markers.length < 2) {
    return null;
  }
  return makeResult("Ren'Py", false, "medium", markers);
}

function detectUnity(dir) {
  const dataDir = findTopLevelDir(dir, (name) => name.toLowerCase().endsWith("_data"));
  const markers = [];

  if (dataDir) {
    markers.push(`${dataDir}/`);
  }
  if (hasTopLevelFileByName(dir, "UnityPlayer.dll")) {
    markers.push("UnityPlayer.dll");
  }
  if (markers.length < 2) {
    return null;
  }
  return makeResult("Unity", false, "medium", markers);
}

function detectGodot(dir) {
  const pckFile = findTopLevelFile(dir, (name) => name.toLowerCase().endsWith(".pck"));
  const exeFile = findTopLevelFile(dir, (name) => name.toLowerCase().endsWith(".exe"));
  const markers = [];

  if (pckFile) {
    markers.push(pckFile);
  }
  if (exeFile) {
    markers.push(exeFile);
  }
  if (markers.length < 2) {
    return null;
  }
  return makeResult("Godot", false, "low", markers);
}

function detectGameMaker(dir) {
  const markers = [];
  for (const fileName of ["data.win", "game.unx", "game.ios", "options.ini"]) {
    if (hasTopLevelFileByName(dir, fileName)) {
      markers.push(fileName);
    }
  }
  if (markers.length === 0) {
    return null;
  }
  return makeResult("GameMaker", false, "low", markers);
}

function makeResult(engine, supported, confidence, markers) {
  return {
    engine,
    supported,
    confidence,
    markers: unique(markers),
  };
}

function unique(values) {
  return [...new Set(values)];
}

function shouldSkipDir(name) {
  return IGNORED_DIRS.has(name.toLowerCase());
}

function scanGames(rootDir, maxDepth) {
  const queue = [{ dir: rootDir, depth: 0 }];
  const seen = new Set();
  const results = [];

  while (queue.length > 0) {
    const current = queue.shift();
    if (!current) {
      continue;
    }

    const normalized = path.normalize(current.dir).toLowerCase();
    if (seen.has(normalized)) {
      continue;
    }
    seen.add(normalized);

    const detected = detectGame(current.dir);
    if (detected) {
      results.push({
        path: current.dir,
        relativePath: normalizeRelative(rootDir, current.dir),
        ...detected,
      });
      continue;
    }

    if (current.depth >= maxDepth) {
      continue;
    }

    for (const childName of listDirs(current.dir)) {
      if (shouldSkipDir(childName)) {
        continue;
      }
      queue.push({
        dir: path.join(current.dir, childName),
        depth: current.depth + 1,
      });
    }
  }

  results.sort((left, right) => left.relativePath.localeCompare(right.relativePath));
  return results;
}

function normalizeRelative(rootDir, targetDir) {
  const relative = path.relative(rootDir, targetDir);
  return relative === "" ? "." : relative.replaceAll("\\", "/");
}

function printTextReport(rootDir, results, includeMarkers) {
  const supported = results.filter((result) => result.supported);
  const unsupported = results.filter((result) => !result.supported);

  console.log(`Raiz: ${rootDir}`);
  console.log(`Soportados por FairTranslator: ${SUPPORTED_ENGINES.join(", ")}`);
  console.log(`Juegos detectados: ${results.length}`);
  console.log("");

  if (supported.length > 0) {
    console.log("Soportados:");
    for (const result of supported) {
      console.log(`- ${result.engine} :: ${result.relativePath}`);
      if (includeMarkers) {
        console.log(`  firmas: ${result.markers.join(", ")}`);
      }
    }
    console.log("");
  }

  if (unsupported.length > 0) {
    console.log("No soportados o heuristica:");
    for (const result of unsupported) {
      console.log(`- ${result.engine} :: ${result.relativePath} [${result.confidence}]`);
      if (includeMarkers) {
        console.log(`  firmas: ${result.markers.join(", ")}`);
      }
    }
    console.log("");
  }

  if (results.length === 0) {
    console.log("No se detectaron juegos compatibles con las firmas configuradas.");
  }
}

function main() {
  try {
    const options = parseArgs(process.argv.slice(2));
    const rootDir = path.resolve(options.root);

    const stats = safeStat(rootDir);
    if (!stats?.isDirectory()) {
      throw new Error(`La ruta no existe o no es una carpeta: ${rootDir}`);
    }

    const results = scanGames(rootDir, options.maxDepth);

    if (options.json) {
      console.log(
        JSON.stringify(
          {
            scanRoot: rootDir,
            supportedByProject: SUPPORTED_ENGINES,
            results,
          },
          null,
          2,
        ),
      );
      return;
    }

    printTextReport(rootDir, results, options.includeMarkers);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    console.error(`Error: ${message}`);
    process.exitCode = 1;
  }
}

main();
