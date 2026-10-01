export type {
  AiBatchApplyResult,
  AiBatchEntry,
  AiBatchExport,
  AiBatchResponseRef,
  AiBatchTranslation,
  BatchProgress,
  Engine,
  EntryStatus,
  GlossaryTerm,
  Project,
  ProviderType,
  TranslationEntry,
  TranslationFile,
} from "./generated/domain";

import type { Engine, EntryStatus, ProviderType } from "./generated/domain";

/** Supported provider authentication modes */
export type ProviderAuthMode = "apiKey" | "codexManaged";

export interface ProjectEntryUpdate {
  filePath: string;
  entryId: string;
  translation: string;
  status: EntryStatus;
  notes?: string;
}

/** Result returned after exporting a project */
export interface ExportResult {
  filesExported: number;
  entriesApplied: number;
  totalEntries: number;
  warnings: string[];
  textFit: TextFitReport;
}

/** Kind of on-screen slot an entry is drawn in (mirrors Rust `TextSlotKind`). */
export type TextSlotKind = "message" | "scroll" | "choice" | "description" | "name" | "term";

/** Entry whose translation still does not fit after the exporter fitted it. */
export interface TextOverflow {
  filePath: string;
  entryId: string;
  kind: TextSlotKind;
  /** Estimated width in pixels of the widest line. */
  width: number;
  /** Width budget in pixels for one line of this slot. */
  maxWidth: number;
  lines: number;
  maxLines: number | null;
}

/** What the exporter did to make text fit the game's windows (mirrors Rust `TextFitReport`). */
export interface TextFitReport {
  /** Active message plugin that wraps `<WordWrap>` messages by itself. */
  wrapPlugin: string | null;
  /** Window sizes widened to match the original text (diagnostic notes, English). */
  calibration: string[];
  wrappedEntries: number;
  paginatedEntries: number;
  pagesAdded: number;
  overflowCount: number;
  overflow: TextOverflow[];
}

/**
 * Provider settings returned by apikey_load. The Rust side serializes Option::None as null and
 * never omits a field, so every optional setting is `string | null`, not `string | undefined`.
 */
export interface ApiKeyInfo {
  provider: ProviderType;
  model: string | null;
  endpoint: string | null;
  organization: string | null;
  project: string | null;
  commandPath: string | null;
  profile: string | null;
  authMode: ProviderAuthMode | null;
  isConfigured: boolean;
}

export interface ProviderModel {
  id: string;
  ownedBy: string | null;
  created: number | null;
}

export interface CodexRuntimeStatus {
  state: "ready" | "loginRequired" | "notInstalled" | "error";
  effectiveCommandPath: string;
  resolvedCommandPath: string | null;
  commandSource: "sidecar" | "configured" | "path";
  version: string | null;
  isInstalled: boolean;
  isAuthenticated: boolean;
  message: string;
}

/** Background update state of an external CLI (mirrors Rust `CliUpdateState`, snake_case). */
export type CliUpdateState = "checking" | "up_to_date" | "updating" | "updated" | "failed" | "skipped" | "offline";

/** Why an update was skipped (mirrors Rust `CliSkipReason`). */
export type CliSkipReason = "disabled" | "bundled" | "custom" | "not_installed";

export interface CliUpdateStatus {
  cli: string;
  state: CliUpdateState;
  fromVersion: string | null;
  toVersion: string | null;
  /** Backend diagnostic in English; the UI shows localized labels instead. */
  message: string;
  skipReason?: CliSkipReason | null;
}

/** Why no project can be created from a folder (mirrors Rust `GameBlockReason`, snake_case). */
export type GameBlockReason = "packed_archive" | "no_game" | "multiple_games";

/** What `project_create` would find in a folder; `game_inspect` only reads. */
export interface GameInspection {
  /** Folder a project would use (for a packed game, the archive's folder). */
  gameRoot: string | null;
  engine: Engine | null;
  /** Encrypted RGSS archive in the game folder (file name), if any. */
  packedArchive: string | null;
  /** null when a project can be created. */
  blockReason: GameBlockReason | null;
  dataFiles: number;
  /** Files with translatable text and their entries; null when blocked. */
  textFiles: number | null;
  entries: number | null;
}

/** Where the open project lives on disk; `saved: false` until the first save of a new project. */
export interface ProjectLocation {
  path: string;
  saved: boolean;
}
