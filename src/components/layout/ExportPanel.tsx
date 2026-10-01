import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { open as dialogOpen } from "@tauri-apps/plugin-dialog";
import { Lock, Upload } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import {
  Sheet,
  SheetBody,
  SheetContent,
  SheetDescription,
  SheetFooter,
  SheetHeader,
  SheetTitle,
} from "@/components/ui/sheet";
import { StatusGlyph } from "@/components/common/StatusGlyph";
import { engineLabel } from "@/components/common/engineLabel";
import { useProject } from "@/hooks/useProject";
import * as ipc from "@/lib/ipc";
import { projectStats } from "@/lib/translationMetrics";
import type { EntryStatus, ExportResult, TextFitReport } from "@/types/domain";
import { Fact } from "./export/Fact";
import { TextFitSummary } from "./export/TextFitSummary";

interface ExportPanelProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onReviewDrafts: () => void;
  onOpenEntry: (filePath: string, entryId: string) => void;
}

const STATUS_ORDER: EntryStatus[] = ["final", "reviewed", "draft", "untranslated"];
// Tramo de cada estado en el medidor apilado: marca para lo final, tinta para lo revisado y grises después.
const SEGMENT_COLOR: Record<EntryStatus, string> = {
  final: "var(--brand)",
  reviewed: "var(--ink)",
  draft: "var(--ink-3)",
  untranslated: "var(--bg-4)",
};

/*
 * Exportar en el panel lateral: cómo está el proyecto por estado, a dónde va la copia y el resultado.
 * La única aclaración permitida por la regla de textos: la carpeta del juego no se modifica.
 */
export function ExportPanel({ open, onOpenChange, onReviewDrafts, onOpenEntry }: ExportPanelProps) {
  const { t } = useTranslation(["common", "editor"]);
  const { project, exportProject } = useProject();
  const [destination, setDestination] = useState<string | null>(null);
  const [isExporting, setIsExporting] = useState(false);
  const [result, setResult] = useState<ExportResult | null>(null);
  const [fitReport, setFitReport] = useState<TextFitReport | null>(null);
  const [isAnalyzing, setIsAnalyzing] = useState(false);

  useEffect(() => {
    if (open) {
      setResult(null);
      setFitReport(null);
    }
  }, [open]);

  const counts = useMemo(() => projectStats(project?.files ?? []).byStatus, [project]);
  const total = counts.untranslated + counts.draft + counts.reviewed + counts.final;
  const translated = total - counts.untranslated;
  const percent = total > 0 ? Math.round((translated / total) * 100) : 0;

  const chooseDestination = useCallback(async () => {
    const selected = await dialogOpen({ directory: true, title: t("common:export.selectFolder") });
    if (selected && typeof selected === "string") {
      setDestination(selected);
      return selected;
    }
    return null;
  }, [t]);

  const runExport = useCallback(async () => {
    const target = destination ?? (await chooseDestination());
    if (!target) return;
    setIsExporting(true);
    try {
      setResult(await exportProject(target));
    } catch (error) {
      toast.error(t("common:error"), {
        description: error instanceof Error ? error.message : String(error),
      });
    } finally {
      setIsExporting(false);
    }
  }, [chooseDestination, destination, exportProject, t]);

  const analyzeFit = useCallback(async () => {
    setIsAnalyzing(true);
    try {
      setFitReport(await ipc.textFitAnalyze());
    } catch (error) {
      toast.error(t("common:error"), {
        description: error instanceof Error ? error.message : String(error),
      });
    } finally {
      setIsAnalyzing(false);
    }
  }, [t]);

  if (!project) return null;

  const hasWarnings = (result?.warnings.length ?? 0) > 0;

  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent>
        <SheetHeader>
          <SheetTitle>
            {result
              ? t(hasWarnings ? "common:export.doneWithWarnings" : "common:export.done")
              : t("common:export.button")}
          </SheetTitle>
          <SheetDescription>
            {`${project.name} · ${engineLabel(project.engine)} · ${project.sourceLang.toUpperCase()} → ${project.targetLang.toUpperCase()}`}
          </SheetDescription>
        </SheetHeader>

        <SheetBody className="gap-[22px] pt-5">
          {!result ? (
            <>
              <section className="flex flex-col gap-3" aria-label={t("editor:grid.status")}>
                <div className="flex items-baseline justify-between">
                  <span className="text-[32px] leading-none font-semibold tracking-[-0.02em] tabular-nums">
                    {percent}&nbsp;%
                  </span>
                  <span className="font-mono text-xs text-ink-2">
                    {t("editor:sidebar.progressOf", { translated, total })}
                  </span>
                </div>
                <div aria-hidden="true" className="flex h-1.5 gap-0.5 overflow-hidden rounded-full">
                  {STATUS_ORDER.map((status) =>
                    counts[status] > 0 ? (
                      <span
                        key={status}
                        style={{ flexGrow: counts[status], background: SEGMENT_COLOR[status] }}
                      />
                    ) : null,
                  )}
                </div>
                <dl className="flex flex-col border-t border-line">
                  {STATUS_ORDER.map((status) => (
                    <div key={status} className="flex items-center gap-2.5 border-b border-line-soft py-2">
                      <StatusGlyph status={status} />
                      <dt className="text-sm text-ink-2">{t(`editor:status.${status}`)}</dt>
                      <dd className="m-0 ml-auto font-mono text-[12.5px] tabular-nums">{counts[status]}</dd>
                    </div>
                  ))}
                </dl>
              </section>

              <dl className="flex flex-col border-t border-line">
                <div className="flex flex-col gap-2 border-b border-line py-3">
                  <dt className="text-xs text-ink-3">{t("common:export.destination")}</dt>
                  <dd className="m-0 flex items-center gap-2">
                    <span
                      className={
                        destination
                          ? "min-w-0 flex-1 truncate font-mono text-[12.5px]"
                          : "min-w-0 flex-1 text-[12.5px] text-ink-4"
                      }
                      title={destination ?? undefined}
                    >
                      {destination ?? t("common:export.noDestination")}
                    </span>
                    <Button size="sm" variant="outline" onClick={() => void chooseDestination()} disabled={isExporting}>
                      {destination ? t("common:export.change") : t("common:export.choose")}
                    </Button>
                  </dd>
                </div>
                <div className="flex flex-col gap-2 border-b border-line py-3">
                  <dt className="text-xs text-ink-3">{t("common:export.gameFolder")}</dt>
                  <dd className="m-0 truncate font-mono text-[12.5px] text-ink-3" title={project.gameDir}>
                    {project.gameDir}
                  </dd>
                </div>
                {!fitReport && (
                  <div className="flex items-center justify-between gap-2 border-b border-line py-3">
                    <dt className="text-xs text-ink-3">{t("common:export.fit.title")}</dt>
                    <dd className="m-0">
                      <Button size="sm" variant="outline" onClick={() => void analyzeFit()} disabled={isAnalyzing || isExporting}>
                        {isAnalyzing ? t("common:export.fit.analyzing") : t("common:export.fit.analyze")}
                      </Button>
                    </dd>
                  </div>
                )}
              </dl>

              {fitReport && <TextFitSummary report={fitReport} project={project} onOpenEntry={onOpenEntry} />}

              <p className="m-0 inline-flex items-center gap-2 text-xs text-ink-3">
                <Lock aria-hidden="true" className="size-3.5 shrink-0" strokeWidth={1.6} />
                {t("common:export.untouched")}
              </p>
            </>
          ) : (
            <section className="flex flex-col gap-[22px]" role="status">
              <dl className="flex flex-col border-t border-line">
                <Fact label={t("common:export.filesWritten")} value={String(result.filesExported)} />
                <Fact
                  label={t("common:export.applied")}
                  value={t("editor:sidebar.progressOf", { translated: result.entriesApplied, total: result.totalEntries })}
                />
                <Fact label={t("common:export.warningsLabel")} value={String(result.warnings.length)} tone={hasWarnings ? "warning" : undefined} />
                <div className="flex flex-col gap-1.5 border-b border-line py-3">
                  <dt className="text-xs text-ink-3">{t("common:export.result.destination")}</dt>
                  <dd className="m-0 font-mono text-[12.5px] break-all select-text">{destination}</dd>
                </div>
              </dl>
              <TextFitSummary report={result.textFit} project={project} onOpenEntry={onOpenEntry} />
              {hasWarnings && (
                <ul
                  className="flex max-h-56 flex-col gap-1 overflow-y-auto text-xs text-ink-2"
                  aria-label={t("common:export.result.warnings", { count: result.warnings.length })}
                >
                  {result.warnings.map((warning, index) => (
                    <li key={`${index}-${warning}`} className="border-b border-line-soft pb-1">
                      {warning}
                    </li>
                  ))}
                </ul>
              )}
            </section>
          )}
        </SheetBody>

        <SheetFooter>
          {result ? (
            <Button variant="outline" size="lg" onClick={() => onOpenChange(false)}>
              {t("common:close")}
            </Button>
          ) : (
            <>
              <Button
                variant="outline"
                size="lg"
                disabled={counts.draft === 0 || isExporting}
                onClick={onReviewDrafts}
              >
                {t("common:export.readiness.reviewDrafts")}
              </Button>
              <Button variant="brand" size="lg" onClick={() => void runExport()} disabled={isExporting}>
                <Upload strokeWidth={1.8} />
                {isExporting ? t("common:export.exporting") : t("common:export.run")}
              </Button>
            </>
          )}
        </SheetFooter>
      </SheetContent>
    </Sheet>
  );
}
