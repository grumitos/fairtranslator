import { invoke } from "@tauri-apps/api/core";
import type {
  AiBatchApplyResult,
  AiBatchExport,
  AiBatchResponseRef,
  ApiKeyInfo,
  BatchProgress,
  CodexRuntimeStatus,
  EntryStatus,
  ExportResult,
  GlossaryTerm,
  ProjectEntryUpdate,
  Project,
  ProviderModel,
  ProviderType,
} from "@/types/domain";

type Nullable<T> = T | null;

type IpcCommand<Payload, Response> = {
  payload: Payload;
  response: Response;
};

type IpcCommands = {
  project_create: IpcCommand<
    { gameDir: string; name: string; sourceLang: string; targetLang: string },
    Project
  >;
  project_open: IpcCommand<{ path: string }, Project>;
  project_save: IpcCommand<{ path: Nullable<string> }, void>;
  project_get: IpcCommand<undefined, Project | null>;
  project_update_entry: IpcCommand<
    {
      filePath: string;
      entryId: string;
      translation: string;
      status: EntryStatus;
      notes: Nullable<string>;
    },
    void
  >;
  project_update_entries: IpcCommand<{ updates: ProjectEntryUpdate[] }, void>;
  export_project: IpcCommand<{ outputDir: string }, ExportResult>;
  apikey_load: IpcCommand<{ provider: ProviderType }, ApiKeyInfo>;
  apikey_save: IpcCommand<
    {
      provider: ProviderType;
      apiKey: string;
      model: Nullable<string>;
      endpoint: Nullable<string>;
      organization: Nullable<string>;
      project: Nullable<string>;
      commandPath: Nullable<string>;
      profile: Nullable<string>;
      authMode: Nullable<string>;
    },
    void
  >;
  apikey_delete: IpcCommand<{ provider: ProviderType }, void>;
  apikey_validate: IpcCommand<{ provider: ProviderType }, void>;
  apikey_models: IpcCommand<
    {
      provider: ProviderType;
      apiKey: Nullable<string>;
      model: Nullable<string>;
      endpoint: Nullable<string>;
      organization: Nullable<string>;
      project: Nullable<string>;
      commandPath: Nullable<string>;
      profile: Nullable<string>;
    },
    ProviderModel[]
  >;
  codex_status: IpcCommand<
    {
      commandPath: Nullable<string>;
      model: Nullable<string>;
      profile: Nullable<string>;
    },
    CodexRuntimeStatus
  >;
  codex_login: IpcCommand<
    {
      commandPath: Nullable<string>;
      model: Nullable<string>;
      profile: Nullable<string>;
      deviceAuth: Nullable<boolean>;
    },
    CodexRuntimeStatus
  >;
  translate_ai_batch_export: IpcCommand<
    { filePaths: string[]; maxEntries: Nullable<number> },
    AiBatchExport
  >;
  translate_ai_batch_apply: IpcCommand<
    {
      rawResponse: string;
      markStatus: Nullable<EntryStatus>;
      expectedProjectId: Nullable<string>;
      expectedBatchId: Nullable<string>;
      responseMap: Nullable<AiBatchResponseRef[]>;
    },
    AiBatchApplyResult
  >;
  glossary_add: IpcCommand<
    { source: string; target: string; caseSensitive: boolean },
    GlossaryTerm[]
  >;
  glossary_update: IpcCommand<
    { source: string; target: string; caseSensitive: boolean },
    GlossaryTerm[]
  >;
  glossary_remove: IpcCommand<{ source: string }, GlossaryTerm[]>;
  glossary_list: IpcCommand<undefined, GlossaryTerm[]>;
  style_guide_list: IpcCommand<undefined, string[]>;
  style_guide_replace: IpcCommand<{ items: string[] }, string[]>;
  translate_batch_start: IpcCommand<
    { provider: ProviderType; filePaths: string[]; model: Nullable<string> },
    BatchProgress
  >;
  translate_batch_cancel: IpcCommand<{ runId: string }, boolean>;
};

type IpcCommandName = keyof IpcCommands;
type IpcPayload<C extends IpcCommandName> = IpcCommands[C]["payload"];
type IpcResponse<C extends IpcCommandName> = IpcCommands[C]["response"];

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

async function safeInvoke<C extends IpcCommandName>(
  command: C,
  ...args: IpcPayload<C> extends undefined ? [] : [payload: IpcPayload<C>]
): Promise<IpcResponse<C>> {
  const payload = args[0] as Record<string, unknown> | undefined;
  try {
    return await invoke<IpcResponse<C>>(command, payload);
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
  safeInvoke("project_create", {
    gameDir,
    name,
    sourceLang,
    targetLang,
  });

export const projectOpen = (path: string) =>
  safeInvoke("project_open", { path });

export const projectSave = (path?: string) =>
  safeInvoke("project_save", { path: path ?? null });

export const projectGet = () =>
  safeInvoke("project_get");

export const projectUpdateEntry = (
  filePath: string,
  entryId: string,
  translation: string,
  status: EntryStatus,
  notes?: string,
) =>
  safeInvoke("project_update_entry", {
    filePath,
    entryId,
    translation,
    status,
    notes: notes ?? null,
  });

export const projectUpdateEntries = (
  updates: ProjectEntryUpdate[],
) =>
  safeInvoke("project_update_entries", { updates });

// ── Export commands ─────────────────────────────────────────────────

export const exportProject = (outputDir: string) =>
  safeInvoke("export_project", { outputDir });

// ── API key commands ────────────────────────────────────────────────

export const apikeyLoad = (provider: ProviderType) =>
  safeInvoke("apikey_load", { provider });

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
  safeInvoke("apikey_save", {
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
  safeInvoke("apikey_delete", { provider });

export const apikeyValidate = (provider: ProviderType) =>
  safeInvoke("apikey_validate", { provider });

export const apikeyModels = (
  provider: ProviderType,
  apiKey?: string,
  model?: string,
  endpoint?: string,
  organization?: string,
  project?: string,
  commandPath?: string,
  profile?: string,
) =>
  safeInvoke("apikey_models", {
    provider,
    apiKey: apiKey ?? null,
    model: model ?? null,
    endpoint: endpoint ?? null,
    organization: organization ?? null,
    project: project ?? null,
    commandPath: commandPath ?? null,
    profile: profile ?? null,
  });

export const codexStatus = (
  commandPath?: string,
  model?: string,
  profile?: string,
) =>
  safeInvoke("codex_status", {
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
  safeInvoke("codex_login", {
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
  safeInvoke("translate_ai_batch_export", {
    filePaths,
    maxEntries: maxEntries ?? null,
  });

export const translateAiBatchApply = (
  rawResponse: string,
  markStatus?: EntryStatus,
  expectedProjectId?: string,
  expectedBatchId?: string,
  responseMap?: AiBatchResponseRef[],
) =>
  safeInvoke("translate_ai_batch_apply", {
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
  safeInvoke("glossary_add", { source, target, caseSensitive });

export const glossaryUpdate = (
  source: string,
  target: string,
  caseSensitive: boolean,
) =>
  safeInvoke("glossary_update", { source, target, caseSensitive });

export const glossaryRemove = (source: string) =>
  safeInvoke("glossary_remove", { source });

export const glossaryList = () =>
  safeInvoke("glossary_list");

// ── Style guide commands ───────────────────────────────────────────

export const styleGuideList = () =>
  safeInvoke("style_guide_list");

export const styleGuideReplace = (items: string[]) =>
  safeInvoke("style_guide_replace", { items });

// ── Batch translation command ───────────────────────────────────────

export const translateBatchStart = (
  provider: ProviderType,
  filePaths: string[],
  model?: string,
) =>
  safeInvoke("translate_batch_start", { provider, filePaths, model: model ?? null });

export const translateBatchCancel = (runId: string) =>
  safeInvoke("translate_batch_cancel", { runId });
