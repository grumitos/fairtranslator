import { invoke } from "@tauri-apps/api/core";
import type {
  AiBatchApplyResult,
  AiBatchExport,
  ApiKeyInfo,
  BatchProgress,
  CodexRuntimeStatus,
  EntryStatus,
  ExportResult,
  GlossaryTerm,
  ProjectEntryUpdate,
  Project,
  ProviderType,
} from "@/types/domain";

function readNestedMessage(value: unknown): string | null {
  if (!value || typeof value !== "object") return null;
  const record = value as Record<string, unknown>;

  if (typeof record.message === "string" && record.message.trim()) {
    return record.message;
  }
  if (typeof record.error === "string" && record.error.trim()) {
    return record.error;
  }
  if (record.error && typeof record.error === "object") {
    const nested = readNestedMessage(record.error);
    if (nested) return nested;
  }

  return null;
}

export function toErrorMessage(error: unknown): string {
  if (error instanceof Error && error.message.trim()) {
    return error.message;
  }
  if (typeof error === "string" && error.trim()) {
    return error;
  }

  const nested = readNestedMessage(error);
  if (nested) return nested;

  if (error && typeof error === "object") {
    try {
      return JSON.stringify(error);
    } catch {
      return "Unknown IPC error";
    }
  }

  return "Unknown IPC error";
}

async function safeInvoke<T>(command: string, payload?: Record<string, unknown>) {
  try {
    return await invoke<T>(command, payload);
  } catch (error) {
    throw new Error(toErrorMessage(error));
  }
}

// ── Project commands ────────────────────────────────────────────────

export const projectCreate = (
  gameDir: string,
  name: string,
  sourceLang: string,
  targetLang: string,
) =>
  safeInvoke<Project>("project_create", {
    gameDir,
    name,
    sourceLang,
    targetLang,
  });

export const projectOpen = (path: string) =>
  safeInvoke<Project>("project_open", { path });

export const projectSave = (path?: string) =>
  safeInvoke<void>("project_save", { path: path ?? null });

export const projectGet = () =>
  safeInvoke<Project | null>("project_get");

export const projectUpdateEntry = (
  filePath: string,
  entryId: string,
  translation: string,
  status: EntryStatus,
  notes?: string,
) =>
  safeInvoke<void>("project_update_entry", {
    filePath,
    entryId,
    translation,
    status,
    notes: notes ?? null,
  });

export const projectUpdateEntries = (
  updates: ProjectEntryUpdate[],
) =>
  safeInvoke<void>("project_update_entries", { updates });

// ── Export commands ─────────────────────────────────────────────────

export const exportProject = (outputDir: string) =>
  safeInvoke<ExportResult>("export_project", { outputDir });

// ── API key commands ────────────────────────────────────────────────

export const apikeyLoad = (provider: ProviderType) =>
  safeInvoke<ApiKeyInfo>("apikey_load", { provider });

export const apikeySave = (
  provider: ProviderType,
  apiKey: string,
  model?: string,
  endpoint?: string,
  organization?: string,
  project?: string,
  commandPath?: string,
  profile?: string,
  authMode?: string,
) =>
  safeInvoke<void>("apikey_save", {
    provider,
    apiKey,
    model: model ?? null,
    endpoint: endpoint ?? null,
    organization: organization ?? null,
    project: project ?? null,
    commandPath: commandPath ?? null,
    profile: profile ?? null,
    authMode: authMode ?? null,
  });

export const apikeyDelete = (provider: ProviderType) =>
  safeInvoke<void>("apikey_delete", { provider });

export const apikeyValidate = (provider: ProviderType) =>
  safeInvoke<void>("apikey_validate", { provider });

export const codexStatus = (
  commandPath?: string,
  model?: string,
  profile?: string,
) =>
  safeInvoke<CodexRuntimeStatus>("codex_status", {
    commandPath: commandPath ?? null,
    model: model ?? null,
    profile: profile ?? null,
  });

export const codexLogin = (
  commandPath?: string,
  model?: string,
  profile?: string,
  deviceAuth?: boolean,
) =>
  safeInvoke<CodexRuntimeStatus>("codex_login", {
    commandPath: commandPath ?? null,
    model: model ?? null,
    profile: profile ?? null,
    deviceAuth: deviceAuth ?? null,
  });

// ── AI-assisted translation commands ────────────────────────────────

export const translateAiBatchExport = (
  filePaths: string[],
  maxEntries?: number,
) =>
  safeInvoke<AiBatchExport>("translate_ai_batch_export", {
    filePaths,
    maxEntries: maxEntries ?? null,
  });

export const translateAiBatchApply = (
  rawResponse: string,
  markStatus?: EntryStatus,
  expectedProjectId?: string,
  expectedBatchId?: string,
  responseMap?: { localId: number; filePath: string; entryId: string }[],
) =>
  safeInvoke<AiBatchApplyResult>("translate_ai_batch_apply", {
    rawResponse,
    markStatus: markStatus ?? null,
    expectedProjectId: expectedProjectId ?? null,
    expectedBatchId: expectedBatchId ?? null,
    responseMap: responseMap ?? null,
  });

// ── Glossary commands ───────────────────────────────────────────────

export const glossaryAdd = (
  source: string,
  target: string,
  caseSensitive: boolean,
) =>
  safeInvoke<GlossaryTerm[]>("glossary_add", { source, target, caseSensitive });

export const glossaryUpdate = (
  source: string,
  target: string,
  caseSensitive: boolean,
) =>
  safeInvoke<GlossaryTerm[]>("glossary_update", { source, target, caseSensitive });

export const glossaryRemove = (source: string) =>
  safeInvoke<GlossaryTerm[]>("glossary_remove", { source });

export const glossaryList = () =>
  safeInvoke<GlossaryTerm[]>("glossary_list");

// ── Style guide commands ───────────────────────────────────────────

export const styleGuideList = () =>
  safeInvoke<string[]>("style_guide_list");

export const styleGuideReplace = (items: string[]) =>
  safeInvoke<string[]>("style_guide_replace", { items });

// ── Batch translation command ───────────────────────────────────────

export const translateBatchStart = (
  provider: ProviderType,
  filePaths: string[],
) =>
  safeInvoke<BatchProgress>("translate_batch_start", { provider, filePaths });
