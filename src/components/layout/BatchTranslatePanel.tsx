import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { ClipboardCopy, Languages, RefreshCw, Square, Upload } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  Sheet,
  SheetBody,
  SheetContent,
  SheetDescription,
  SheetFooter,
  SheetHeader,
  SheetTitle,
} from "@/components/ui/sheet";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Textarea } from "@/components/ui/textarea";
import { useTranslation as useTranslationHook } from "@/hooks/useTranslation";
import * as ipc from "@/lib/ipc";
import { refreshProjectFromBackend } from "@/lib/projectSync";
import { fileStats } from "@/lib/translationMetrics";
import { cn } from "@/lib/utils";
import { useProjectStore } from "@/stores/projectStore";
import { useSettingsStore } from "@/stores/settingsStore";
import type { AiBatchExport, ApiKeyInfo, ProviderModel, ProviderType } from "@/types/domain";

interface BatchTranslatePanelProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onConfigureProvider: (provider: ProviderType) => void;
}

type BatchMode = "providers" | "externalAi";
type ProviderState = "ready" | "missing" | "unknown";

const PROVIDERS: { id: ProviderType; kind: "CLI" | "API" }[] = [
  { id: "codex", kind: "CLI" },
  { id: "openai", kind: "API" },
  { id: "opencode", kind: "API" },
  { id: "deepl", kind: "API" },
  { id: "google", kind: "API" },
];

export function BatchTranslatePanel({
  open,
  onOpenChange,
  onConfigureProvider,
}: BatchTranslatePanelProps) {
  const { t } = useTranslation(["editor", "common"]);
  const { isTranslating, progress, error, startBatch, cancelBatch } = useTranslationHook();
  const project = useProjectStore((s) => s.project);
  const lastProvider = useSettingsStore((s) => s.lastProvider);
  const setLastProvider = useSettingsStore((s) => s.setLastProvider);
  const [provider, setProvider] = useState<ProviderType | "">(() => lastProvider ?? "");
  const [providerConfig, setProviderConfig] = useState<ApiKeyInfo | null>(null);
  const [providerConfigLoadFailed, setProviderConfigLoadFailed] = useState(false);
  const [providerStates, setProviderStates] = useState<Partial<Record<ProviderType, ProviderState>>>({});
  const [runModel, setRunModel] = useState("");
  const [providerModels, setProviderModels] = useState<ProviderModel[]>([]);
  const [isRefreshingModels, setIsRefreshingModels] = useState(false);
  const [mode, setMode] = useState<BatchMode>("providers");
  const [selectedFiles, setSelectedFiles] = useState<string[]>([]);
  const [allFiles, setAllFiles] = useState(true);
  const [isAiBusy, setIsAiBusy] = useState(false);
  const [maxEntries, setMaxEntries] = useState("200");
  const [aiBatch, setAiBatch] = useState<AiBatchExport | null>(null);
  const [aiPrompt, setAiPrompt] = useState("");
  const [aiResponse, setAiResponse] = useState("");
  const [aiError, setAiError] = useState<string | null>(null);
  const [aiWarnings, setAiWarnings] = useState<string[]>([]);
  const [isCancelling, setIsCancelling] = useState(false);
  const wasTranslatingRef = useRef(false);

  const files = project?.files ?? [];
  const fileSelection = useMemo(() => (allFiles ? [] : selectedFiles), [allFiles, selectedFiles]);
  const untranslatedCount = useMemo(
    () => files
      .filter((file) => allFiles || selectedFiles.includes(file.relativePath))
      .reduce((sum, file) => sum + fileStats(file.entries).pending, 0),
    [allFiles, files, selectedFiles],
  );
  const instructionCandidates =
    aiBatch?.entries.filter((entry) => entry.instructionCandidate).length ?? 0;
  const supportsRunModel =
    provider === "openai" || provider === "opencode" || provider === "codex";
  const requiresRunModel = provider === "openai" || provider === "opencode";
  const requiresProviderConfig = Boolean(provider && provider !== "codex");
  const canStartProviderRun =
    Boolean(provider) &&
    (!requiresProviderConfig || Boolean(providerConfig?.isConfigured)) &&
    (!requiresRunModel || Boolean(runModel.trim())) &&
    !isTranslating &&
    untranslatedCount > 0;
  const handleToggleFile = useCallback((path: string) => {
    setSelectedFiles((prev) =>
      prev.includes(path) ? prev.filter((p) => p !== path) : [...prev, path],
    );
  }, []);

  const handleStart = useCallback(async () => {
    if (!provider) {
      return;
    }
    if (requiresProviderConfig && !providerConfig?.isConfigured) {
      toast.error(t("editor:batch.noProvider"));
      return;
    }
    if (requiresRunModel && !runModel.trim()) {
      toast.error(t("editor:batch.models.required"));
      return;
    }
    try {
      const selectedRunModel = supportsRunModel
        ? provider === "codex"
          ? runModel
          : runModel.trim() || undefined
        : undefined;
      await startBatch(provider, fileSelection, selectedRunModel);
      setLastProvider(provider);
    } catch {
      // useTranslation ya muestra el error en el panel.
    }
  }, [fileSelection, provider, providerConfig?.isConfigured, requiresProviderConfig, requiresRunModel, runModel, setLastProvider, startBatch, supportsRunModel, t]);

  const handleCancel = useCallback(async () => {
    setIsCancelling(true);
    try {
      const accepted = await cancelBatch();
      if (accepted) {
        toast.info(t("editor:batch.cancelRequested"));
      } else {
        setIsCancelling(false);
      }
    } catch (e) {
      setIsCancelling(false);
      toast.error(t("editor:batch.cancelFailed"), {
        description: e instanceof Error ? e.message : String(e),
      });
    }
  }, [cancelBatch, t]);

  const handleRefreshProviderModels = useCallback(async () => {
    if (!provider || !supportsRunModel) return;
    setIsRefreshingModels(true);
    try {
      const models = await ipc.apikeyModels(
        provider,
        undefined,
        runModel || undefined,
        providerConfig?.endpoint,
        providerConfig?.organization,
        providerConfig?.project,
        providerConfig?.commandPath,
        providerConfig?.profile,
      );
      setProviderModels(models);
      if (!runModel && models.length > 0) {
        setRunModel(models[0].id);
      }
      toast.success(t("editor:batch.models.refreshed"), {
        description: t("editor:batch.models.count", { count: models.length }),
      });
    } catch (e) {
      toast.error(t("editor:batch.models.refreshFailed"), {
        description: e instanceof Error ? e.message : String(e),
      });
    } finally {
      setIsRefreshingModels(false);
    }
  }, [provider, providerConfig, runModel, supportsRunModel, t]);

  const handleExportAiBatch = useCallback(async () => {
    const parsedMaxEntries = maxEntries.trim() ? Number(maxEntries) : undefined;
    if (
      parsedMaxEntries !== undefined &&
      (!Number.isInteger(parsedMaxEntries) || parsedMaxEntries < 1)
    ) {
      setAiError(t("editor:batch.ai.invalidMaxEntries"));
      return;
    }

    setIsAiBusy(true);
    setAiError(null);
    setAiWarnings([]);
    try {
      const batch = await ipc.translateAiBatchExport(fileSelection, parsedMaxEntries);
      setAiBatch(batch);
      setAiPrompt(batch.prompt);
      setAiResponse("");
      toast.success(t("editor:batch.ai.exported"), {
        description: t("editor:batch.ai.exportedDetail", {
          exported: batch.exportedEntries,
          total: batch.totalSelectedEntries,
        }),
      });
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      setAiError(message);
      toast.error(t("editor:batch.ai.exportFailed"), {
        description: message,
      });
    } finally {
      setIsAiBusy(false);
    }
  }, [fileSelection, maxEntries, t]);

  const handleCopyPrompt = useCallback(async () => {
    if (!aiPrompt.trim()) {
      return;
    }
    try {
      await navigator.clipboard.writeText(aiPrompt);
      toast.success(t("editor:batch.ai.copied"));
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      setAiError(message);
      toast.error(t("editor:batch.ai.copyFailed"), {
        description: message,
      });
    }
  }, [aiPrompt, t]);

  const handleApplyAiBatch = useCallback(async () => {
    if (!aiBatch) {
      setAiError(t("editor:batch.ai.exportRequired"));
      return;
    }
    if (!aiResponse.trim()) {
      setAiError(t("editor:batch.ai.responseRequired"));
      return;
    }

    setIsAiBusy(true);
    setAiError(null);
    setAiWarnings([]);
    try {
      const result = await ipc.translateAiBatchApply(
        aiResponse,
        "draft",
        aiBatch.projectId,
        aiBatch.batchId,
        aiBatch.responseMap,
      );
      // Espera las ediciones en vuelo y reintenta si hubo cambios durante la lectura (lib/projectSync).
      await refreshProjectFromBackend(true);
      setAiWarnings(result.warnings);
      if (
        result.missingEntries.length > 0 ||
        result.duplicateEntries.length > 0 ||
        result.invalidEntries.length > 0
      ) {
        const description = t("editor:batch.ai.partialApplyDetail", {
          applied: result.appliedEntries,
          skipped: result.skippedEntries,
          invalid: result.invalidEntries.length,
          missing: result.missingEntries.length,
          duplicates: result.duplicateEntries.length,
        });
        setAiError(description);
        toast.error(t("editor:batch.ai.partialApply"), {
          description,
        });
      } else {
        setAiResponse("");
        toast.success(t("editor:batch.ai.applied"), {
          description: t("editor:batch.ai.appliedDetail", {
            applied: result.appliedEntries,
            skipped: result.skippedEntries,
          }),
        });
        if (result.warnings.length > 0) {
          toast.warning(t("editor:batch.ai.integrityWarnings"), {
            description: t("editor:batch.ai.integrityWarningsDetail", {
              count: result.warnings.length,
            }),
          });
        }
      }
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      setAiError(message);
      toast.error(t("editor:batch.ai.applyFailed"), {
        description: message,
      });
    } finally {
      setIsAiBusy(false);
    }
  }, [aiBatch, aiResponse, t]);

  useEffect(() => {
    setAiBatch(null);
    setAiPrompt("");
    setAiResponse("");
    setAiError(null);
    setAiWarnings([]);
  }, [project?.id]);

  // Estado de cada proveedor para la lista: se consulta al abrir el panel (Codex no necesita clave).
  useEffect(() => {
    if (!open) return;
    let isActive = true;
    void Promise.allSettled(
      PROVIDERS.filter((option) => option.id !== "codex").map((option) =>
        ipc.apikeyLoad(option.id).then((info) => [option.id, info.isConfigured] as const),
      ),
    ).then((results) => {
      if (!isActive) return;
      const next: Partial<Record<ProviderType, ProviderState>> = { codex: "ready" };
      for (const result of results) {
        if (result.status === "fulfilled") {
          next[result.value[0]] = result.value[1] ? "ready" : "missing";
        }
      }
      setProviderStates(next);
    });
    return () => {
      isActive = false;
    };
  }, [open]);

  // Al reabrir el panel se relee la configuración: pudo cambiar en Ajustes mientras estaba cerrado.
  useEffect(() => {
    let isActive = true;
    setProviderModels([]);
    setProviderConfig(null);
    setProviderConfigLoadFailed(false);
    setRunModel("");

    if (!provider) {
      return () => {
        isActive = false;
      };
    }

    ipc.apikeyLoad(provider)
      .then((info) => {
        if (!isActive) return;
        setProviderConfig(info);
        setProviderConfigLoadFailed(false);
        setRunModel(info.model ?? "");
      })
      .catch(() => {
        if (!isActive) return;
        setProviderConfig(null);
        setProviderConfigLoadFailed(true);
        setRunModel("");
      });

    return () => {
      isActive = false;
    };
  }, [provider, open]);

  useEffect(() => {
    if (wasTranslatingRef.current && !isTranslating && progress) {
      if (progress.cancelled) {
        toast.info(t("editor:batch.cancelled"), {
          description: t("editor:batch.cancelledDetail", {
            completed: progress.entriesTranslated,
            total: progress.totalEntries,
          }),
        });
      } else if (progress.failedEntries > 0) {
        toast.error(t("editor:batch.completeWithErrors"), {
          description: t("editor:batch.completeWithErrorsDetail", {
            completed: progress.entriesTranslated,
            failed: progress.failedEntries,
            total: progress.totalEntries,
          }),
        });
      } else {
        toast.success(t("editor:batch.complete"), {
          description: t("editor:batch.completeDetail", {
            completed: progress.entriesTranslated,
            total: progress.totalEntries,
          }),
        });
      }
    }

    wasTranslatingRef.current = isTranslating;
  }, [isTranslating, progress, t]);

  useEffect(() => {
    if (!isTranslating) {
      setIsCancelling(false);
    }
  }, [isTranslating]);

  const progressPercent =
    progress && progress.totalEntries > 0
      ? Math.round(
          ((progress.entriesTranslated + progress.failedEntries) / progress.totalEntries) * 100,
        )
      : 0;
  const runFinished =
    Boolean(progress) &&
    !isTranslating &&
    (progress!.cancelled || progress!.failedEntries > 0 || progress!.chunksProcessed === progress!.totalChunks);
  const providerStatus = (id: ProviderType) => {
    const state = providerStates[id] ?? "unknown";
    if (state === "ready") return t("editor:batch.providers.ready");
    if (state === "missing") return t("editor:batch.providers.configuration.notConfigured");
    return "";
  };

  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent>
        <SheetHeader>
          <SheetTitle>{t("editor:batch.title")}</SheetTitle>
          <SheetDescription>
            {t("editor:batch.summary", { project: project?.name ?? "", count: untranslatedCount })}
          </SheetDescription>
        </SheetHeader>

        <Tabs
          value={mode}
          onValueChange={(value) => setMode(value as BatchMode)}
          className="min-h-0 flex-1 gap-0"
        >
          <TabsList variant="line" className="mx-5 mt-3 w-[calc(100%-2.5rem)] justify-start">
            <TabsTrigger value="providers">{t("editor:batch.tabs.providers")}</TabsTrigger>
            <TabsTrigger value="externalAi">{t("editor:batch.tabs.externalAi")}</TabsTrigger>
          </TabsList>

          <SheetBody className="gap-[22px]">
            <section className="flex flex-col gap-2">
              <span className="text-xs text-ink-3">{t("editor:batch.selectFiles")}</span>
              <div
                role="radiogroup"
                aria-label={t("editor:batch.selectFiles")}
                className="flex gap-0.5 rounded-[8px] border border-line-field bg-surface-0 p-0.5"
              >
                {[true, false].map((value) => (
                  <button
                    key={String(value)}
                    type="button"
                    role="radio"
                    aria-checked={allFiles === value}
                    onClick={() => {
                      setAllFiles(value);
                      if (value) setSelectedFiles([]);
                    }}
                    className={cn(
                      "h-[26px] flex-1 rounded-md text-[12.5px] outline-none transition-colors duration-(--motion-fast) focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ink",
                      allFiles === value ? "bg-surface-3 text-ink" : "text-ink-3 hover:text-ink",
                    )}
                  >
                    {value
                      ? t("editor:batch.allFilesCount", { count: files.length })
                      : t("editor:batch.selection")}
                  </button>
                ))}
              </div>
              {!allFiles && (
                <ScrollArea className="h-44 border-y border-line">
                  {files.length === 0 ? (
                    <p className="ft-empty-state text-xs">{t("editor:sidebar.noFiles")}</p>
                  ) : (
                    files.map((file) => (
                      <label
                        key={file.relativePath}
                        className="flex h-8 cursor-pointer items-center gap-2.5 border-b border-line-soft px-1 last:border-b-0"
                      >
                        <Checkbox
                          checked={selectedFiles.includes(file.relativePath)}
                          onCheckedChange={() => handleToggleFile(file.relativePath)}
                          aria-label={file.relativePath}
                        />
                        <span className="flex-1 truncate font-mono text-xs">{file.relativePath}</span>
                        <span className="font-mono text-xs text-ink-3">
                          {fileStats(file.entries).pending}
                        </span>
                      </label>
                    ))
                  )}
                </ScrollArea>
              )}
              {untranslatedCount === 0 && (
                <p className="text-xs text-ink-3" role="status">
                  {t("editor:batch.noEntries")}
                </p>
              )}
            </section>

            <TabsContent value="providers" className="flex flex-col gap-[22px]">
              <div role="radiogroup" aria-labelledby="batch-provider-label" className="flex flex-col">
                <span id="batch-provider-label" className="mb-1.5 text-xs text-ink-3">
                  {t("editor:batch.provider")}
                </span>
                {PROVIDERS.map((option) => {
                  const checked = provider === option.id;
                  const state = providerStates[option.id] ?? "unknown";
                  return (
                    <button
                      key={option.id}
                      type="button"
                      role="radio"
                      aria-checked={checked}
                      onClick={() => setProvider(option.id)}
                      className="flex h-10 items-center gap-3 border-b border-line-soft px-0.5 text-left text-sm outline-none focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-ink"
                    >
                      <span
                        className={cn(
                          "flex size-4 items-center justify-center rounded-full border-[1.5px]",
                          checked ? "border-ink" : "border-line-chip",
                        )}
                      >
                        {checked && <span className="size-1.5 rounded-full bg-ink" />}
                      </span>
                      <span className={cn("font-medium", state === "missing" && !checked && "text-ink-2")}>
                        {t(`editor:batch.providers.names.${option.id}`)}
                      </span>
                      <span className="font-mono text-[11px] text-ink-4">{option.kind}</span>
                      <span className={cn("ml-auto text-xs", state === "ready" ? "text-ink-2" : "text-ink-4")}>
                        {providerStatus(option.id)}
                      </span>
                    </button>
                  );
                })}
              </div>

              {requiresProviderConfig &&
                (providerConfigLoadFailed || providerConfig?.isConfigured === false) && (
                  <div
                    className="flex items-center justify-between gap-3 border-y border-line py-2.5"
                    role={providerConfigLoadFailed ? "alert" : "status"}
                  >
                    <p className="text-xs text-warning">
                      {t(
                        providerConfigLoadFailed
                          ? "editor:batch.providers.configuration.loadFailed"
                          : "editor:batch.providers.configuration.notConfigured",
                      )}
                    </p>
                    <Button
                      type="button"
                      size="sm"
                      variant="outline"
                      onClick={() => {
                        if (provider) onConfigureProvider(provider);
                      }}
                    >
                      {t("editor:batch.providers.configuration.action")}
                    </Button>
                  </div>
                )}

              {provider && supportsRunModel && (
                <div className="flex flex-col gap-1.5">
                  <label htmlFor="batch-provider-model" className="text-xs text-ink-3">
                    {t("editor:batch.models.title")}
                  </label>
                  <div className="flex gap-2">
                    <Input
                      id="batch-provider-model"
                      name="batch-provider-model"
                      list={providerModels.length > 0 ? "batch-provider-models" : undefined}
                      autoComplete="off"
                      autoCapitalize="none"
                      spellCheck={false}
                      value={runModel}
                      onChange={(event) => setRunModel(event.target.value)}
                      placeholder={
                        provider === "codex"
                          ? t("editor:batch.models.cliDefault")
                          : t("editor:batch.models.manualPlaceholder")
                      }
                      className="font-mono text-[12.5px]"
                    />
                    <Button
                      type="button"
                      size="icon"
                      variant="outline"
                      className="size-8"
                      onClick={() => void handleRefreshProviderModels()}
                      disabled={
                        isRefreshingModels ||
                        (requiresProviderConfig && !providerConfig?.isConfigured)
                      }
                      aria-label={t("editor:batch.models.refresh")}
                      title={t("editor:batch.models.refresh")}
                    >
                      <RefreshCw className={cn(isRefreshingModels && "animate-spin")} strokeWidth={1.7} />
                    </Button>
                  </div>
                  {providerModels.length > 0 && (
                    <datalist id="batch-provider-models">
                      {providerModels.map((option) => (
                        <option key={option.id} value={option.id}>
                          {option.ownedBy ?? ""}
                        </option>
                      ))}
                    </datalist>
                  )}
                </div>
              )}

              <dl className="flex flex-col border-t border-line">
                <Fact label={t("editor:batch.pending")} value={String(untranslatedCount)} mono />
                {provider && (
                  <Fact label={t("editor:batch.context")} value={t(`editor:batch.providers.context.${provider}`)} />
                )}
              </dl>
            </TabsContent>

            <TabsContent value="externalAi" className="flex flex-col gap-[18px]">
              <div className="flex items-end gap-2.5">
                <div className="flex w-40 flex-col gap-1.5">
                  <label htmlFor="ai-max-entries" className="text-xs text-ink-3">
                    {t("editor:batch.ai.maxEntries")}
                  </label>
                  <Input
                    id="ai-max-entries"
                    type="number"
                    min={1}
                    step={1}
                    value={maxEntries}
                    onChange={(event) => setMaxEntries(event.target.value)}
                    className="font-mono"
                  />
                </div>
                <Button
                  onClick={handleExportAiBatch}
                  disabled={isAiBusy || untranslatedCount === 0}
                  variant="outline"
                >
                  <Upload strokeWidth={1.7} />
                  {t("editor:batch.ai.export")}
                </Button>
              </div>

              {aiBatch && (
                <dl className="flex flex-col border-t border-line">
                  <Fact label={t("editor:batch.ai.batch")} value={aiBatch.batchId} mono />
                  <Fact
                    label={t("editor:batch.ai.exportedLabel")}
                    value={`${aiBatch.exportedEntries}/${aiBatch.totalSelectedEntries}`}
                    mono
                  />
                  <Fact label={t("editor:batch.ai.instructionLabel")} value={String(instructionCandidates)} mono />
                </dl>
              )}

              <div className="flex flex-col gap-1.5">
                <div className="flex items-center justify-between gap-2">
                  <label htmlFor="ai-prompt" className="text-xs text-ink-3">
                    {t("editor:batch.ai.prompt")}
                  </label>
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    onClick={handleCopyPrompt}
                    disabled={!aiPrompt.trim()}
                  >
                    <ClipboardCopy strokeWidth={1.6} />
                    {t("editor:batch.ai.copy")}
                  </Button>
                </div>
                <Textarea
                  id="ai-prompt"
                  value={aiPrompt}
                  readOnly
                  className="min-h-40 bg-surface-0 font-mono text-[11.5px] text-ink-2"
                  placeholder="—"
                />
              </div>

              <div className="flex flex-col gap-1.5">
                <label htmlFor="ai-response" className="text-xs text-ink-3">
                  {t("editor:batch.ai.response")}
                </label>
                <Textarea
                  id="ai-response"
                  value={aiResponse}
                  onChange={(event) => setAiResponse(event.target.value)}
                  className="min-h-36 font-mono text-[11.5px]"
                  placeholder={t("editor:batch.ai.responsePlaceholder")}
                />
              </div>

              {aiError && <p className="text-xs text-danger" role="alert">{aiError}</p>}
              {aiWarnings.length > 0 && (
                <ul className="flex flex-col gap-1 text-xs text-warning">
                  {aiWarnings.map((warning) => (
                    <li key={warning}>{warning}</li>
                  ))}
                </ul>
              )}
            </TabsContent>

            {progress && (
              <section className="flex flex-col gap-4 border-t border-line pt-5" aria-live="polite">
                <div className="flex flex-col gap-2.5">
                  <div className="flex items-baseline justify-between">
                    <span className="text-[32px] leading-none font-semibold tracking-[-0.02em] tabular-nums">
                      {progressPercent}&nbsp;%
                    </span>
                    <span className="font-mono text-xs text-ink-2">
                      {t("editor:batch.progressOf", {
                        completed: progress.entriesTranslated,
                        total: progress.totalEntries,
                      })}
                    </span>
                  </div>
                  <div
                    className="h-1.5 overflow-hidden rounded-full bg-line"
                    role="progressbar"
                    aria-label={t("editor:batch.title")}
                    aria-valuemin={0}
                    aria-valuemax={100}
                    aria-valuenow={progressPercent}
                  >
                    <div
                      className="h-full rounded-full bg-brand transition-[width] duration-(--motion-slow)"
                      style={{ width: `${progressPercent}%` }}
                    />
                  </div>
                  <span className="h-4 truncate font-mono text-[11.5px] text-ink-3">
                    {isTranslating && progress.currentFile
                      ? t("editor:batch.currentFileShort", {
                          file: progress.currentFile,
                          current: progress.currentFileChunksProcessed,
                          total: progress.currentFileTotalChunks,
                        })
                      : ""}
                  </span>
                </div>
                <dl className="flex flex-col border-t border-line">
                  <Fact
                    label={t("editor:batch.chunks")}
                    value={`${progress.chunksProcessed}/${progress.totalChunks}`}
                    mono
                  />
                  <Fact
                    label={t("editor:batch.filesLabel")}
                    value={`${progress.filesProcessed}/${progress.totalFiles}`}
                    mono
                  />
                  <Fact
                    label={t("editor:batch.failed")}
                    value={String(progress.failedEntries)}
                    mono
                    tone={progress.failedEntries > 0 ? "danger" : undefined}
                  />
                </dl>
                {runFinished && (
                  <p
                    role="status"
                    className={cn(
                      "text-sm",
                      progress.cancelled || progress.failedEntries > 0 ? "text-warning" : "text-ink",
                    )}
                  >
                    {progress.cancelled
                      ? `${t("editor:batch.cancelled")} · ${t("editor:batch.cancelledDetail", {
                          completed: progress.entriesTranslated,
                          total: progress.totalEntries,
                        })}`
                      : progress.failedEntries > 0
                        ? t("editor:batch.completeWithErrorsDetail", {
                            completed: progress.entriesTranslated,
                            failed: progress.failedEntries,
                            total: progress.totalEntries,
                          })
                        : t("editor:batch.completeDetail", {
                            completed: progress.entriesTranslated,
                            total: progress.totalEntries,
                          })}
                  </p>
                )}
                {runFinished && progress.warnings.length > 0 && (
                  <ul className="flex flex-col gap-1 text-xs text-warning">
                    {progress.warnings.map((warning) => (
                      <li key={warning}>{warning}</li>
                    ))}
                  </ul>
                )}
              </section>
            )}
            {error && <p className="text-xs text-danger" role="alert">{error}</p>}
          </SheetBody>
        </Tabs>

        <SheetFooter>
          {mode === "externalAi" ? (
            <Button
              variant="brand"
              size="lg"
              onClick={handleApplyAiBatch}
              disabled={isAiBusy || !aiBatch || !aiResponse.trim()}
            >
              {t("editor:batch.ai.apply")}
            </Button>
          ) : isTranslating ? (
            <Button
              type="button"
              variant="outline"
              size="lg"
              onClick={() => void handleCancel()}
              disabled={!progress?.runId || isCancelling}
            >
              <Square className="size-3 fill-current" strokeWidth={0} />
              {isCancelling ? t("editor:batch.cancelling") : t("editor:batch.cancel")}
            </Button>
          ) : (
            <>
              <Button variant="outline" size="lg" onClick={() => onOpenChange(false)}>
                {t("common:close")}
              </Button>
              <Button
                variant="brand"
                size="lg"
                onClick={handleStart}
                disabled={!canStartProviderRun}
                aria-label={t("editor:batch.start")}
              >
                <Languages strokeWidth={1.8} />
                {t("editor:toolbar.translate")}
                <span className="font-mono text-xs font-medium">{untranslatedCount}</span>
              </Button>
            </>
          )}
        </SheetFooter>
      </SheetContent>
    </Sheet>
  );
}

function Fact({
  label,
  value,
  mono = false,
  tone,
}: {
  label: string;
  value: string;
  mono?: boolean;
  tone?: "danger";
}) {
  return (
    <div className="flex items-baseline justify-between gap-4 border-b border-line py-2.5">
      <dt className="text-xs text-ink-3">{label}</dt>
      <dd
        className={cn(
          "m-0 text-right text-[12.5px]",
          mono && "font-mono tabular-nums",
          tone === "danger" ? "text-danger" : "text-ink",
        )}
      >
        {value}
      </dd>
    </div>
  );
}
