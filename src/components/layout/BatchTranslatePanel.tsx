import { type CSSProperties, useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Bot, CheckCircle, ClipboardCopy, Play, Upload } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { Progress } from "@/components/ui/progress";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Sheet, SheetContent, SheetHeader, SheetTitle } from "@/components/ui/sheet";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Textarea } from "@/components/ui/textarea";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useTranslation as useTranslationHook } from "@/hooks/useTranslation";
import * as ipc from "@/lib/ipc";
import { isPendingTranslationEntry } from "@/lib/translationMetrics";
import { useProjectStore } from "@/stores/projectStore";
import type { AiBatchExport, ProviderType } from "@/types/domain";

interface BatchTranslatePanelProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

type BatchMode = "providers" | "externalAi";

const PROVIDERS: { id: ProviderType; labelKey: string }[] = [
  { id: "openai", labelKey: "editor:batch.providers.openai" },
  { id: "deepl", labelKey: "editor:batch.providers.deepl" },
  { id: "google", labelKey: "editor:batch.providers.google" },
  { id: "codex", labelKey: "editor:batch.providers.codex" },
];

export function BatchTranslatePanel({
  open,
  onOpenChange,
}: BatchTranslatePanelProps) {
  const { t } = useTranslation(["editor", "common"]);
  const { isTranslating, progress, error, startBatch } = useTranslationHook();
  const project = useProjectStore((s) => s.project);
  const replaceProject = useProjectStore((s) => s.replaceProject);
  const [provider, setProvider] = useState<ProviderType | "">("");
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
  const wasTranslatingRef = useRef(false);

  const files = project?.files ?? [];
  const fileSelection = allFiles ? [] : selectedFiles;
  const untranslatedCount = files
    .filter((f) => allFiles || selectedFiles.includes(f.relativePath))
    .reduce((sum, f) => sum + f.entries.filter(isPendingTranslationEntry).length, 0);
  const instructionCandidates =
    aiBatch?.entries.filter((entry) => entry.instructionCandidate).length ?? 0;
  const providerCapabilityKey = provider
    ? (`editor:batch.providers.capabilities.${provider}` as const)
    : null;

  const handleToggleFile = useCallback((path: string) => {
    setSelectedFiles((prev) =>
      prev.includes(path) ? prev.filter((p) => p !== path) : [...prev, path],
    );
  }, []);

  const handleStart = useCallback(async () => {
    if (!provider) {
      return;
    }
    try {
      await startBatch(provider, fileSelection);
    } catch (e) {
      toast.error(String(e));
    }
  }, [provider, fileSelection, startBatch]);

  const handleExportAiBatch = useCallback(async () => {
    const parsedMaxEntries = maxEntries.trim() ? Number(maxEntries) : undefined;
    if (
      parsedMaxEntries !== undefined &&
      (!Number.isFinite(parsedMaxEntries) || parsedMaxEntries < 1)
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
      const refreshed = await ipc.projectGet();
      if (refreshed) {
        replaceProject(refreshed, { markDirty: true });
      }
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
  }, [aiBatch, aiResponse, replaceProject, t]);

  useEffect(() => {
    setAiBatch(null);
    setAiPrompt("");
    setAiResponse("");
    setAiError(null);
    setAiWarnings([]);
  }, [project?.id]);

  useEffect(() => {
    if (wasTranslatingRef.current && !isTranslating && progress) {
      if (progress.failedEntries > 0) {
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

  const progressPercent =
    progress && progress.totalEntries > 0
      ? Math.round(
          ((progress.entriesTranslated + progress.failedEntries) / progress.totalEntries) * 100,
        )
      : 0;

  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent style={{ "--sheet-width": "420px" } as CSSProperties}>
        <SheetHeader>
          <SheetTitle>{t("editor:batch.title")}</SheetTitle>
        </SheetHeader>
        <div className="mt-4 space-y-4">
          <div className="space-y-2">
            <Label>{t("editor:batch.selectFiles")}</Label>
            <div className="flex items-center gap-2">
              <Checkbox
                id="all-files"
                checked={allFiles}
                onCheckedChange={(v) => {
                  setAllFiles(v === true);
                  if (v === true) {
                    setSelectedFiles([]);
                  }
                }}
              />
              <Label htmlFor="all-files" className="text-xs">
                {t("editor:batch.allFiles")}
              </Label>
            </div>

            {!allFiles && (
              <ScrollArea className="h-40 rounded-md border p-2">
                {files.map((file) => {
                  const untranslated = file.entries.filter(
                    (e) => e.status === "untranslated",
                  ).length;
                  return (
                    <div key={file.relativePath} className="flex items-center gap-2 py-1">
                      <Checkbox
                        checked={selectedFiles.includes(file.relativePath)}
                        onCheckedChange={() => handleToggleFile(file.relativePath)}
                      />
                      <span className="flex-1 truncate text-xs">{file.relativePath}</span>
                      <span className="text-[10px] text-muted-foreground">{untranslated}</span>
                    </div>
                  );
                })}
              </ScrollArea>
            )}

            <p className="text-xs text-muted-foreground">
              {t("editor:batch.untranslatedOnly")}: {untranslatedCount}
            </p>
          </div>

          <Tabs value={mode} onValueChange={(value) => setMode(value as BatchMode)}>
            <TabsList className="grid w-full grid-cols-2">
              <TabsTrigger value="providers">{t("editor:batch.tabs.providers")}</TabsTrigger>
              <TabsTrigger value="externalAi">{t("editor:batch.tabs.externalAi")}</TabsTrigger>
            </TabsList>

            <TabsContent value="providers" className="space-y-4 pt-2">
              <div className="rounded-md border border-blue-500/20 bg-blue-500/10 p-3 text-[11px] text-muted-foreground">
                <p className="font-medium text-foreground">{t("editor:batch.providers.modeTitle")}</p>
                <p className="mt-1">{t("editor:batch.providers.modeDescription")}</p>
                <p className="mt-1">{t("editor:batch.providers.modeFeatures")}</p>
              </div>

              <div className="space-y-2">
                <Label>{t("editor:batch.provider")}</Label>
                <Select value={provider} onValueChange={(v) => setProvider(v as ProviderType)}>
                  <SelectTrigger className="w-full">
                    <SelectValue placeholder={t("editor:batch.selectProvider")} />
                  </SelectTrigger>
                  <SelectContent>
                    {PROVIDERS.map((providerOption) => (
                      <SelectItem key={providerOption.id} value={providerOption.id}>
                        {t(providerOption.labelKey)}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>

              {providerCapabilityKey && (
                <div className="rounded-md border border-border/70 bg-muted/40 p-3 text-[11px] text-muted-foreground">
                  <p className="font-medium text-foreground">
                    {t("editor:batch.providers.capabilityTitle")}
                  </p>
                  <p className="mt-1">{t(providerCapabilityKey)}</p>
                </div>
              )}

              <Button
                onClick={handleStart}
                disabled={!provider || isTranslating || untranslatedCount === 0}
                className="w-full"
              >
                {isTranslating ? (
                  <>
                    <span className="mr-2 animate-spin">&#8987;</span>
                    {t("editor:batch.translating")}
                  </>
                ) : (
                  <>
                    <Play className="mr-2 size-4" />
                    {t("editor:batch.start")}
                  </>
                )}
              </Button>
            </TabsContent>

            <TabsContent value="externalAi" className="space-y-4 pt-2">
              <div className="space-y-2">
                <Label htmlFor="ai-max-entries">{t("editor:batch.ai.maxEntries")}</Label>
                <Input
                  id="ai-max-entries"
                  type="number"
                  min={1}
                  step={1}
                  value={maxEntries}
                  onChange={(event) => setMaxEntries(event.target.value)}
                />
                <p className="text-xs text-muted-foreground">
                  {t("editor:batch.ai.description")}
                </p>
              </div>

              <Button
                onClick={handleExportAiBatch}
                disabled={isAiBusy || untranslatedCount === 0}
                className="w-full"
                variant="secondary"
              >
                <Bot className="mr-2 size-4" />
                {t("editor:batch.ai.export")}
              </Button>

              {aiBatch && (
                <div className="space-y-2 rounded-md border p-3 text-xs">
                  <p>
                    {t("editor:batch.ai.exportedDetail", {
                      exported: aiBatch.exportedEntries,
                      total: aiBatch.totalSelectedEntries,
                    })}
                  </p>
                  <p className="text-muted-foreground">
                    {t("editor:batch.ai.instructionCandidates", {
                      count: instructionCandidates,
                    })}
                  </p>
                </div>
              )}

              <div className="space-y-2">
                <div className="flex items-center justify-between gap-2">
                  <Label htmlFor="ai-prompt">{t("editor:batch.ai.prompt")}</Label>
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    onClick={handleCopyPrompt}
                    disabled={!aiPrompt.trim()}
                  >
                    <ClipboardCopy className="mr-2 size-4" />
                    {t("editor:batch.ai.copy")}
                  </Button>
                </div>
                <Textarea
                  id="ai-prompt"
                  value={aiPrompt}
                  readOnly
                  className="min-h-40 font-mono text-xs"
                  placeholder={t("editor:batch.ai.promptPlaceholder")}
                />
              </div>

              <div className="space-y-2">
                <Label htmlFor="ai-response">{t("editor:batch.ai.response")}</Label>
                <Textarea
                  id="ai-response"
                  value={aiResponse}
                  onChange={(event) => setAiResponse(event.target.value)}
                  className="min-h-40 font-mono text-xs"
                  placeholder={t("editor:batch.ai.responsePlaceholder")}
                />
              </div>

              <Button
                onClick={handleApplyAiBatch}
                disabled={isAiBusy || !aiBatch || !aiResponse.trim()}
                className="w-full"
              >
                <Upload className="mr-2 size-4" />
                {t("editor:batch.ai.apply")}
              </Button>

              {aiError && <p className="text-xs text-destructive">{aiError}</p>}
              {aiWarnings.length > 0 && (
                <div className="rounded-md border border-amber-500/30 bg-amber-500/10 p-2 text-xs text-amber-900 dark:text-amber-100">
                  {aiWarnings.map((warning) => (
                    <p key={warning}>{warning}</p>
                  ))}
                </div>
              )}
            </TabsContent>
          </Tabs>

          {progress && (
            <div className="space-y-2">
              <Progress value={progressPercent} className="h-2" />
              <p className="text-xs text-muted-foreground">
                {t("editor:batch.progress", {
                  completed: progress.entriesTranslated,
                  total: progress.totalEntries,
                })}
              </p>
              <p className="text-xs text-muted-foreground">
                {t("editor:batch.progressFiles", {
                  completed: progress.filesProcessed,
                  total: progress.totalFiles,
                })}
              </p>
              <p className="text-xs text-muted-foreground">
                {t("editor:batch.progressChunks", {
                  completed: progress.chunksProcessed,
                  total: progress.totalChunks,
                })}
              </p>
              {progress.failedEntries > 0 && (
                <p className="text-xs text-destructive">
                  {t("editor:batch.errors", { count: progress.failedEntries })}
                </p>
              )}
              {progress.currentFile && (
                <div className="space-y-1">
                  <p className="truncate text-xs text-muted-foreground">
                    {t("editor:batch.currentFile", { file: progress.currentFile })}
                  </p>
                  <p className="text-xs text-muted-foreground">
                    {t("editor:batch.currentChunk", {
                      current: progress.currentFileChunksProcessed,
                      total: progress.currentFileTotalChunks,
                    })}
                  </p>
                </div>
              )}
              {!isTranslating && progress.chunksProcessed === progress.totalChunks && (
                <div className="space-y-2">
                  <div className="flex items-center gap-2 text-xs">
                    <CheckCircle
                      className={
                        progress.failedEntries > 0 ? "size-4 text-amber-500" : "size-4 text-green-500"
                      }
                    />
                    <span
                      className={
                        progress.failedEntries > 0
                          ? "text-amber-600 dark:text-amber-400"
                          : "text-green-600 dark:text-green-400"
                      }
                    >
                      {progress.failedEntries > 0
                        ? t("editor:batch.completeWithErrorsDetail", {
                            completed: progress.entriesTranslated,
                            failed: progress.failedEntries,
                            total: progress.totalEntries,
                          })
                        : t("editor:batch.completeDetail", {
                            completed: progress.entriesTranslated,
                            total: progress.totalEntries,
                          })}
                    </span>
                  </div>
                  {progress.warnings.length > 0 && (
                    <div className="rounded-md border border-amber-500/30 bg-amber-500/10 p-2 text-xs text-amber-900 dark:text-amber-100">
                      {progress.warnings.map((warning) => (
                        <p key={warning}>{warning}</p>
                      ))}
                    </div>
                  )}
                </div>
              )}
            </div>
          )}
          {error && <p className="text-xs text-destructive">{error}</p>}
        </div>
      </SheetContent>
    </Sheet>
  );
}
