import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { ScrollArea } from "@/components/ui/scroll-area";
import { countTranslationProgress } from "@/lib/translationMetrics";
import { useProjectStore } from "@/stores/projectStore";
import { cn } from "@/lib/utils";
import { FileText, FolderOpen } from "lucide-react";
import type { TranslationFile } from "@/types/domain";

/** Returns a Tailwind color class based on translation progress percentage */
function progressColor(percent: number): string {
  if (percent >= 100) return "ft-progress-complete";
  if (percent >= 60) return "ft-progress-good";
  if (percent >= 30) return "ft-progress-partial";
  return "ft-progress-blocked";
}

function fileStem(relativePath: string): string {
  const fileName = relativePath.split("/").pop() ?? relativePath;
  return fileName.replace(/\.[^.]+$/, "");
}

export function Sidebar() {
  const { t } = useTranslation("editor");
  const project = useProjectStore((s) => s.project);
  const activeFile = useProjectStore((s) => s.activeFile);
  const setActiveFile = useProjectStore((s) => s.setActiveFile);

  const files = project?.files ?? [];

  const handleFileClick = (file: TranslationFile) => {
    setActiveFile(file);
  };

  const { grouped, globalStats } = useMemo(() => {
    const labels = {
      system: t("sidebar.groups.system"),
      database: t("sidebar.groups.database"),
      events: t("sidebar.groups.events"),
      maps: t("sidebar.groups.maps"),
    };
    const groups = groupFiles(files, labels);
    let total = 0;
    let translated = 0;

    const groupedWithStats = groups.map((group) => ({
      ...group,
      files: group.files.map((file) => {
        const stats = countTranslationProgress(file.entries);
        total += stats.total;
        translated += stats.translated;
        return {
          file,
          entryCount: stats.total,
          translatedCount: stats.translated,
          percent: stats.percent,
        };
      }),
    }));

    return {
      grouped: groupedWithStats,
      globalStats: {
        total,
        translated,
        percent: total > 0 ? Math.round((translated / total) * 100) : 0,
      },
    };
  }, [files, t]);

  if (!project) return null;

  return (
    <aside className="flex h-full w-full flex-col bg-sidebar text-sidebar-foreground">
      <div className="flex h-10 items-center gap-2 border-b border-sidebar-border px-3">
        <FolderOpen className="size-4 shrink-0 text-muted-foreground" />
        <span className="truncate text-sm font-semibold tracking-normal">{project.name}</span>
      </div>

      {/* Global progress bar */}
      <div className="border-b border-sidebar-border px-3 py-2">
        <div className="mb-1 flex items-center justify-between text-[11px] text-muted-foreground">
          <span>{t("statusBar.percentage", { percent: globalStats.percent })}</span>
          <span className="font-medium tabular-nums text-sidebar-foreground">{globalStats.translated}/{globalStats.total}</span>
        </div>
        <div
          className="h-1.5 w-full overflow-hidden rounded-full border border-sidebar-border bg-background/55"
          role="progressbar"
          aria-label={t("statusBar.percentage", { percent: globalStats.percent })}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={globalStats.percent}
        >
          <div
            className={cn("h-full rounded-full transition-[width] duration-200", progressColor(globalStats.percent))}
            style={{ width: `${globalStats.percent}%` }}
          />
        </div>
      </div>

      <ScrollArea className="flex-1">
        <div className="p-1.5">
          {/* All entries */}
          <button
            onClick={() => setActiveFile(null)}
            className={cn(
              "ft-focus flex h-8 w-full items-center gap-2 rounded-md border border-transparent px-2 text-left text-sm transition-[background-color,border-color,color] duration-150 hover:bg-sidebar-accent",
              !activeFile && "border-sidebar-border bg-sidebar-accent text-sidebar-accent-foreground shadow-xs",
            )}
            aria-current={!activeFile ? "page" : undefined}
          >
            <FileText className="size-3.5 shrink-0 text-muted-foreground" />
            <span className="truncate font-medium">{t("sidebar.allEntries")}</span>
            <span className="ml-auto rounded bg-background/70 px-1.5 py-0.5 text-[10px] text-muted-foreground">
              {globalStats.total}
            </span>
          </button>

          {/* File groups */}
          {grouped.length === 0 ? (
            <div className="ft-empty-state mt-2 px-3 py-5 text-xs text-muted-foreground">
              {t("sidebar.noFiles")}
            </div>
          ) : grouped.map((group) => (
            <div key={group.label} className="mt-2">
              <div className="px-2 py-1 text-[10px] font-semibold uppercase tracking-normal text-muted-foreground">
                {group.label}
              </div>
              {group.files.map(({ file, entryCount, translatedCount, percent }) => {
                const isActive = activeFile?.relativePath === file.relativePath;

                return (
                  <button
                    key={file.relativePath}
                    onClick={() => handleFileClick(file)}
                    className={cn(
                      "ft-focus flex w-full flex-col rounded-md border border-transparent px-2 py-1 text-left text-sm transition-[background-color,border-color,color] duration-150 hover:bg-sidebar-accent",
                      isActive &&
                        "border-sidebar-border bg-sidebar-accent text-sidebar-accent-foreground shadow-xs",
                    )}
                    aria-current={isActive ? "page" : undefined}
                  >
                    <div className="flex w-full items-center gap-2">
                      <FileText className="size-3.5 shrink-0 text-muted-foreground" />
                      <span className="truncate font-medium">{fileStem(file.relativePath)}</span>
                      <span className="ml-auto shrink-0 tabular-nums text-[10px] text-muted-foreground">
                        {translatedCount}/{entryCount}
                      </span>
                    </div>
                    {/* Per-file mini progress bar */}
                    <div
                      className="mt-1 ml-5 h-1 w-[calc(100%-1.25rem)] overflow-hidden rounded-full bg-background/60"
                      role="progressbar"
                      aria-label={t("statusBar.percentage", { percent })}
                      aria-valuemin={0}
                      aria-valuemax={100}
                      aria-valuenow={percent}
                    >
                      <div
                        className={cn("h-full rounded-full transition-[width] duration-200", progressColor(percent))}
                        style={{ width: `${percent}%` }}
                      />
                    </div>
                  </button>
                );
              })}
            </div>
          ))}
        </div>
      </ScrollArea>
    </aside>
  );
}

// ---------------------------------------------------------------------------
// File grouping helpers
// ---------------------------------------------------------------------------

interface FileGroup {
  label: string;
  files: TranslationFile[];
}

interface GroupLabels {
  system: string;
  database: string;
  events: string;
  maps: string;
}

function groupFiles(files: TranslationFile[], labels: GroupLabels): FileGroup[] {
  const system: TranslationFile[] = [];
  const database: TranslationFile[] = [];
  const events: TranslationFile[] = [];
  const maps: TranslationFile[] = [];

  for (const file of files) {
    const name = fileStem(file.relativePath);
    if (name === "System") {
      system.push(file);
    } else if (name === "CommonEvents" || name === "Troops") {
      events.push(file);
    } else if (/^Map\d+$/.test(name)) {
      maps.push(file);
    } else {
      database.push(file);
    }
  }

  const groups: FileGroup[] = [];
  if (system.length > 0) groups.push({ label: labels.system, files: system });
  if (database.length > 0) groups.push({ label: labels.database, files: database });
  if (events.length > 0) groups.push({ label: labels.events, files: events });
  if (maps.length > 0) groups.push({ label: labels.maps, files: maps });

  return groups;
}
