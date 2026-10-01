import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { List } from "lucide-react";
import { ProgressRing } from "@/components/common/ProgressRing";
import { fileStem, groupFiles } from "@/components/common/fileGroups";
import { ScrollArea } from "@/components/ui/scroll-area";
import { fileStats } from "@/lib/translationMetrics";
import { useProjectStore } from "@/stores/projectStore";
import { cn } from "@/lib/utils";

/*
 * Archivos del proyecto: avance global arriba y la lista agrupada debajo. Filas planas, sin cajas;
 * el archivo activo solo cambia el fondo.
 */
export function Sidebar() {
  const { t } = useTranslation("editor");
  const project = useProjectStore((s) => s.project);
  const activeFile = useProjectStore((s) => s.activeFile);
  const setActiveFile = useProjectStore((s) => s.setActiveFile);

  const { groups, globalStats } = useMemo(() => {
    let total = 0;
    let translated = 0;

    const grouped = groupFiles(project?.files ?? []).map((group) => ({
      key: group.key,
      files: group.files.map((file) => {
        const stats = fileStats(file.entries);
        total += stats.total;
        translated += stats.translated;
        return { file, total: stats.total, translated: stats.translated };
      }),
    }));

    return {
      groups: grouped,
      globalStats: { total, translated, percent: total > 0 ? Math.round((translated / total) * 100) : 0 },
    };
  }, [project]);

  if (!project) return null;

  const rowClass = (active: boolean) =>
    cn(
      "flex w-full items-center gap-2.5 rounded-md px-2.5 text-left text-sm outline-none transition-colors duration-(--motion-fast) focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-ink",
      active ? "bg-surface-3 text-ink" : "text-ink hover:bg-surface-2",
    );

  return (
    <aside aria-label={t("sidebar.files")} className="flex h-full w-full flex-col border-r border-line bg-surface-1">
      <div className="flex flex-col gap-2.5 border-b border-line px-4 pt-4 pb-3.5">
        <div className="flex items-baseline justify-between gap-3">
          <span className="text-[22px] leading-none font-semibold tracking-[-0.02em] tabular-nums">
            {globalStats.percent}&nbsp;%
          </span>
          <span className="font-mono text-xs text-ink-3">
            {t("sidebar.progressOf", { translated: globalStats.translated, total: globalStats.total })}
          </span>
        </div>
        <div
          className="h-1 overflow-hidden rounded-full bg-line"
          role="progressbar"
          aria-label={t("statusBar.percentage", { percent: globalStats.percent })}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={globalStats.percent}
        >
          <div
            className="h-full rounded-full bg-brand transition-[width] duration-(--motion-slow)"
            style={{ width: `${globalStats.percent}%` }}
          />
        </div>
      </div>

      <ScrollArea className="min-h-0 flex-1">
        <nav aria-label={t("sidebar.files")} className="flex flex-col p-2">
          <button
            type="button"
            onClick={() => setActiveFile(null)}
            className={cn(rowClass(!activeFile), "h-8")}
            aria-current={!activeFile ? "page" : undefined}
          >
            <List aria-hidden="true" className="size-[15px] shrink-0 text-ink-2" strokeWidth={1.6} />
            <span className="truncate font-medium">{t("sidebar.allEntries")}</span>
            <span className="ml-auto font-mono text-xs text-ink-3 tabular-nums">{globalStats.total}</span>
          </button>

          {groups.length === 0 ? (
            <p className="ft-empty-state mt-2 text-xs">{t("sidebar.noFiles")}</p>
          ) : (
            groups.map((group) => (
              <div key={group.key} role="group" aria-label={t(`sidebar.groups.${group.key}`)} className="flex flex-col">
                <span className="px-2.5 pt-4 pb-1.5 text-[11px] font-medium tracking-[0.06em] text-ink-4 uppercase">
                  {t(`sidebar.groups.${group.key}`)}
                </span>
                {group.files.map(({ file, total, translated }) => {
                  const isActive = activeFile?.relativePath === file.relativePath;
                  return (
                    <button
                      key={file.relativePath}
                      type="button"
                      onClick={() => setActiveFile(file)}
                      className={cn(rowClass(isActive), "h-[30px]")}
                      aria-current={isActive ? "page" : undefined}
                      title={file.relativePath}
                    >
                      <ProgressRing done={translated} total={total} />
                      <span className={cn("truncate", isActive && "font-semibold")}>{fileStem(file.relativePath)}</span>
                      <span className={cn("ml-auto font-mono text-xs tabular-nums", isActive ? "text-ink-2" : "text-ink-3")}>
                        {translated}/{total}
                      </span>
                    </button>
                  );
                })}
              </div>
            ))
          )}
        </nav>
      </ScrollArea>
    </aside>
  );
}
