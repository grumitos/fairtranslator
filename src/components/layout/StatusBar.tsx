import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { isTranslatableEntry, isTranslatedEntry } from "@/lib/translationMetrics";
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

  if (!project) {
    return (
      <footer className="flex h-6 items-center border-t bg-muted/50 px-3 text-xs text-muted-foreground">
        {t("statusBar.noProject")}
      </footer>
    );
  }

  const { totalEntries, translatedEntries, percent, activeTotal, activeTranslated } = useMemo(() => {
    let total = 0;
    let translated = 0;
    for (const file of project.files) {
      total += file.entries.filter(isTranslatableEntry).length;
      translated += file.entries.filter(isTranslatedEntry).length;
    }

    const activeEntries = activeFile?.entries ?? [];
    const activeTotalCount = activeEntries.filter(isTranslatableEntry).length;
    const activeTranslatedCount = activeEntries.filter(isTranslatedEntry).length;

    return {
      totalEntries: total,
      translatedEntries: translated,
      percent: total > 0 ? Math.round((translated / total) * 100) : 0,
      activeTotal: activeTotalCount,
      activeTranslated: activeTranslatedCount,
    };
  }, [project.files, activeFile]);

  return (
    <footer className="flex h-6 items-center justify-between border-t bg-muted/50 px-3 text-xs text-muted-foreground">
      <div className="flex items-center gap-4">
        <span>{t("statusBar.engine", { engine: formatEngineLabel(project.engine) })}</span>
        <span>
          {project.sourceLang.toUpperCase()} &rarr; {project.targetLang.toUpperCase()}
        </span>
      </div>
      <div className="flex items-center gap-3">
        {activeFile && (
          <span className="text-foreground/70">
            {fileStem(activeFile.relativePath)}: {activeTranslated}/{activeTotal}
          </span>
        )}
        <span>{t("statusBar.total", { count: totalEntries })}</span>
        <span>{t("statusBar.translated", { count: translatedEntries })}</span>

        {/* Mini inline progress bar */}
        <div className="flex items-center gap-1.5">
          <div className="h-2 w-20 rounded-full bg-muted overflow-hidden">
            <div
              className={cn(
                "h-full rounded-full transition-all",
                percent >= 100
                  ? "bg-green-500"
                  : percent >= 60
                    ? "bg-yellow-500"
                    : percent >= 30
                      ? "bg-orange-500"
                      : "bg-red-500",
              )}
              style={{ width: `${percent}%` }}
            />
          </div>
          <span className="font-medium text-foreground/80">
            {t("statusBar.percentage", { percent })}
          </span>
        </div>
      </div>
    </footer>
  );
}
