import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { countTranslationProgress } from "@/lib/translationMetrics";
import { useProjectStore } from "@/stores/projectStore";
import { cn } from "@/lib/utils";
import type { Engine } from "@/types/domain";

function formatEngineLabel(engine: Engine): string {
  switch (engine) {
    case "mv":
      return "MV";
    case "mz":
      return "MZ";
    case "vxAce":
      return "VX Ace";
    case "vx":
      return "VX";
    case "xp":
      return "XP";
  }
}

function fileStem(relativePath: string): string {
  const fileName = relativePath.split("/").pop() ?? relativePath;
  return fileName.replace(/\.[^.]+$/, "");
}

export function StatusBar() {
  const { t } = useTranslation("editor");
  const project = useProjectStore((s) => s.project);
  const activeFile = useProjectStore((s) => s.activeFile);

  const { totalEntries, translatedEntries, percent, activeTotal, activeTranslated } = useMemo(() => {
    if (!project) {
      return {
        totalEntries: 0,
        translatedEntries: 0,
        percent: 0,
        activeTotal: 0,
        activeTranslated: 0,
      };
    }

    const totalStats = project.files.reduce(
      (acc, file) => {
        const stats = countTranslationProgress(file.entries);
        acc.total += stats.total;
        acc.translated += stats.translated;
        return acc;
      },
      { total: 0, translated: 0 },
    );
    const activeStats = activeFile
      ? countTranslationProgress(activeFile.entries)
      : { total: 0, translated: 0 };

    return {
      totalEntries: totalStats.total,
      translatedEntries: totalStats.translated,
      percent: totalStats.total > 0 ? Math.round((totalStats.translated / totalStats.total) * 100) : 0,
      activeTotal: activeStats.total,
      activeTranslated: activeStats.translated,
    };
  }, [project, activeFile]);

  if (!project) {
    return (
      <footer className="flex h-7 items-center border-t bg-card/95 px-3 text-xs text-muted-foreground">
        {t("statusBar.noProject")}
      </footer>
    );
  }

  return (
    <footer className="flex h-7 items-center justify-between gap-3 border-t bg-card/95 px-3 text-xs text-muted-foreground">
      <div className="flex min-w-0 items-center gap-3">
        <span>{t("statusBar.engine", { engine: formatEngineLabel(project.engine) })}</span>
        <span>
          {project.sourceLang.toUpperCase()} &rarr; {project.targetLang.toUpperCase()}
        </span>
      </div>
      <div className="flex min-w-0 items-center gap-3">
        {activeFile && (
          <span className="hidden max-w-48 truncate text-foreground/70 sm:inline">
            {fileStem(activeFile.relativePath)}: {activeTranslated}/{activeTotal}
          </span>
        )}
        <span className="hidden sm:inline">{t("statusBar.total", { count: totalEntries })}</span>
        <span className="hidden sm:inline">{t("statusBar.translated", { count: translatedEntries })}</span>

        {/* Mini inline progress bar */}
        <div className="flex items-center gap-1.5">
          <div
            className="h-2 w-20 overflow-hidden rounded-full border bg-muted"
            role="progressbar"
            aria-label={t("statusBar.percentage", { percent })}
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={percent}
          >
            <div
              className={cn(
                "h-full rounded-full transition-[width] duration-200",
                percent >= 100
                  ? "ft-progress-complete"
                  : percent >= 60
                    ? "ft-progress-good"
                    : percent >= 30
                      ? "ft-progress-partial"
                      : "ft-progress-blocked",
              )}
              style={{ width: `${percent}%` }}
            />
          </div>
          <span className="font-semibold tabular-nums text-foreground/80">
            {t("statusBar.percentage", { percent })}
          </span>
        </div>
      </div>
    </footer>
  );
}
