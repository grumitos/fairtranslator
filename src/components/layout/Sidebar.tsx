import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { ScrollArea } from "@/components/ui/scroll-area";
import { isTranslatableEntry, isTranslatedEntry } from "@/lib/translationMetrics";
import { useProjectStore } from "@/stores/projectStore";
import { cn } from "@/lib/utils";
import { FileText, FolderOpen } from "lucide-react";
import type { TranslationFile } from "@/types/domain";

/** Returns a Tailwind color class based on translation progress percentage */
function progressColor(percent: number): string {
  if (percent >= 100) return "bg-green-500";
  if (percent >= 60) return "bg-yellow-500";
  if (percent >= 30) return "bg-orange-500";
  return "bg-red-500";
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

  if (!project) return null;

  const files = project.files;

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
        const entryCount = file.entries.filter(isTranslatableEntry).length;
        const translatedCount = file.entries.filter(isTranslatedEntry).length;
        total += entryCount;
        translated += translatedCount;
        return {
          file,
          entryCount,
          translatedCount,
          percent: entryCount > 0 ? Math.round((translatedCount / entryCount) * 100) : 0,
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

  return (
    <aside className="flex h-full w-full flex-col bg-sidebar text-sidebar-foreground">
      <div className="flex items-center gap-2 border-b px-3 py-2">
        <FolderOpen className="size-4 text-muted-foreground" />
        <span className="text-sm font-medium truncate">{project.name}</span>
      </div>

      {/* Global progress bar */}
      <div className="px-3 py-1.5 border-b">
        <div className="flex items-center justify-between text-xs text-muted-foreground mb-1">
          <span>{t("statusBar.percentage", { percent: globalStats.percent })}</span>
          <span>{globalStats.translated}/{globalStats.total}</span>
        </div>
        <div className="h-1.5 w-full rounded-full bg-muted overflow-hidden">
          <div
            className={cn("h-full rounded-full transition-all", progressColor(globalStats.percent))}
            style={{ width: `${globalStats.percent}%` }}
          />
        </div>
      </div>

      <ScrollArea className="flex-1">
        <div className="p-1">
          {/* All entries */}
          <button
            onClick={() => setActiveFile(null)}
            className={cn(
              "flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm transition-colors hover:bg-sidebar-accent",
              !activeFile && "bg-sidebar-accent text-sidebar-accent-foreground font-medium",
            )}
          >
            <FileText className="size-3.5 text-muted-foreground" />
            <span>{t("sidebar.allEntries")}</span>
            <span className="ml-auto text-xs text-muted-foreground">
              {globalStats.total}
            </span>
          </button>

          {/* File groups */}
          {grouped.map((group) => (
            <div key={group.label} className="mt-2">
              <div className="px-2 py-1 text-xs font-semibold uppercase text-muted-foreground tracking-wider">
                {group.label}
              </div>
              {group.files.map(({ file, entryCount, translatedCount, percent }) => {
                const isActive = activeFile?.relativePath === file.relativePath;

                return (
                  <button
                    key={file.relativePath}
                    onClick={() => handleFileClick(file)}
                    className={cn(
                      "flex w-full flex-col rounded-md px-2 py-1 text-left text-sm transition-colors hover:bg-sidebar-accent",
                      isActive &&
                        "bg-sidebar-accent text-sidebar-accent-foreground font-medium",
                    )}
                  >
                    <div className="flex w-full items-center gap-2">
                      <FileText className="size-3.5 shrink-0 text-muted-foreground" />
                      <span className="truncate">{fileStem(file.relativePath)}</span>
                      <span className="ml-auto shrink-0 text-xs text-muted-foreground">
                        {translatedCount}/{entryCount}
                      </span>
                    </div>
                    {/* Per-file mini progress bar */}
                    <div className="mt-0.5 ml-5.5 h-1 w-[calc(100%-1.375rem)] rounded-full bg-muted overflow-hidden">
                      <div
                        className={cn("h-full rounded-full transition-all", progressColor(percent))}
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
