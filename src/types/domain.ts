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

import type { EntryStatus, ProviderType } from "./generated/domain";

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
}

/** API key info returned by apikey_load */
export interface ApiKeyInfo {
  provider: ProviderType;
  apiKey: string;
  model?: string;
  endpoint?: string;
  organization?: string;
  project?: string;
  commandPath?: string;
  profile?: string;
  authMode?: ProviderAuthMode;
  isConfigured?: boolean;
}

export interface ProviderModel {
  id: string;
  ownedBy?: string;
  created?: number;
}

export interface CodexRuntimeStatus {
  state: "ready" | "loginRequired" | "notInstalled" | "error" | string;
  effectiveCommandPath: string;
  resolvedCommandPath?: string;
  commandSource: "sidecar" | "configured" | "path" | string;
  version?: string;
  isInstalled: boolean;
  isAuthenticated: boolean;
  message: string;
}
