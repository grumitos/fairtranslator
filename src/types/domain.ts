/** Supported RPG Maker engine types */
export type Engine = "mv" | "mz" | "vxAce" | "vx" | "xp";

/** Translation status for individual entries */
export type EntryStatus = "untranslated" | "draft" | "reviewed" | "final";

/** Supported translation API providers */
export type ProviderType = "openai" | "deepl" | "google" | "codex";

/** Supported provider authentication modes */
export type ProviderAuthMode = "apiKey" | "codexManaged";

/** Root project model */
export interface Project {
  id: string;
  name: string;
  sourceLang: string;
  targetLang: string;
  engine: Engine;
  gameDir: string;
  files: TranslationFile[];
  glossary: GlossaryTerm[];
  styleGuide: string[];
  createdAt: string;
  updatedAt: string;
}

/** A single game file and its translatable entries */
export interface TranslationFile {
  relativePath: string;
  entries: TranslationEntry[];
}

/** A single translatable text entry */
export interface TranslationEntry {
  id: string;
  fileContext: string;
  original: string;
  translation: string;
  status: EntryStatus;
  notes: string;
}

export interface ProjectEntryUpdate {
  filePath: string;
  entryId: string;
  translation: string;
  status: EntryStatus;
  notes?: string;
}

/** A glossary term for translation consistency */
export interface GlossaryTerm {
  source: string;
  target: string;
  caseSensitive: boolean;
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

/** Progress event payload during batch translation */
export interface BatchProgress {
  filesProcessed: number;
  totalFiles: number;
  chunksProcessed: number;
  totalChunks: number;
  entriesTranslated: number;
  totalEntries: number;
  currentFile: string;
  currentFileChunksProcessed: number;
  currentFileTotalChunks: number;
  failedEntries: number;
  failedBatches: number;
  warnings: string[];
}

/** A single entry exported for manual or AI-assisted translation */
export interface AiBatchEntry {
  filePath: string;
  entryId: string;
  fileContext: string;
  original: string;
  currentTranslation: string;
  instructionCandidate: boolean;
  instructionReason?: string;
}

/** Structured batch payload ready to be sent to an AI model */
export interface AiBatchExport {
  projectId: string;
  projectName: string;
  batchId: string;
  engine: Engine;
  sourceLang: string;
  targetLang: string;
  totalSelectedEntries: number;
  exportedEntries: number;
  entries: AiBatchEntry[];
  responseMap: AiBatchResponseRef[];
  prompt: string;
}

export interface AiBatchResponseRef {
  localId: number;
  filePath: string;
  entryId: string;
}

/** A single translated entry produced by an external AI workflow */
export interface AiBatchTranslation {
  filePath: string;
  entryId: string;
  translation: string;
  notes?: string;
  skip: boolean;
}

/** Result of applying an external AI batch back into the current project */
export interface AiBatchApplyResult {
  appliedEntries: number;
  skippedEntries: number;
  missingEntries: string[];
  duplicateEntries: string[];
  invalidEntries: string[];
  warnings: string[];
}
