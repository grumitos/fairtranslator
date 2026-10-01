import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { Replace, Search } from "lucide-react";
import { StatusGlyph } from "@/components/common/StatusGlyph";
import { fileGroupKey, fileStem } from "@/components/common/fileGroups";
import { Button } from "@/components/ui/button";
import { fileStats, projectStats } from "@/lib/translationMetrics";
import { cn } from "@/lib/utils";
import { useEditorStore } from "@/stores/editorStore";
import { useProjectStore } from "@/stores/projectStore";
import type { EntryStatus } from "@/types/domain";

const FILTERS: (EntryStatus | null)[] = [null, "untranslated", "draft", "reviewed", "final"];

/*
 * Cabecera del área de trabajo: qué se está viendo (archivo o proyecto), sus cifras y los filtros.
 * Sin cajas: título, fila de hechos y una franja de filtros entre líneas.
 */
export function EditorHeader() {
  const { t } = useTranslation("editor");
  const project = useProjectStore((s) => s.project);
  const activeFile = useProjectStore((s) => s.activeFile);
  const searchQuery = useEditorStore((s) => s.searchQuery);
  const setSearchQuery = useEditorStore((s) => s.setSearchQuery);
  const statusFilter = useEditorStore((s) => s.statusFilter);
  const setStatusFilter = useEditorStore((s) => s.setStatusFilter);
  const showFindReplace = useEditorStore((s) => s.showFindReplace);
  const toggleFindReplace = useEditorStore((s) => s.toggleFindReplace);

  const stats = useMemo(() => {
    const counts = activeFile ? fileStats(activeFile.entries) : projectStats(project?.files ?? []);
    return { ...counts, percent: counts.total > 0 ? Math.round((counts.translated / counts.total) * 100) : 0 };
  }, [activeFile, project]);

  if (!project) return null;

  const group = activeFile ? t(`sidebar.groups.${fileGroupKey(activeFile.relativePath)}`) : t("common:project");
  const title = activeFile ? fileStem(activeFile.relativePath) : t("sidebar.allEntries");
  const subtitle = activeFile ? activeFile.relativePath : t("sidebar.fileCount", { count: project.files.length });

  return (
    <div className="shrink-0">
      <div className="flex items-end justify-between gap-6 px-6 pt-[18px] pb-4">
        <div className="flex min-w-0 flex-col gap-1.5">
          <span className="text-[11px] font-medium tracking-[0.06em] text-ink-4 uppercase">{group}</span>
          <div className="flex min-w-0 items-baseline gap-2.5">
            <h1 className="truncate text-[20px] leading-tight font-semibold tracking-[-0.015em]">{title}</h1>
            <span className="truncate font-mono text-xs text-ink-4">{subtitle}</span>
          </div>
        </div>
        <dl className="flex shrink-0 items-end gap-7">
          <Fact label={t("header.entries")} value={stats.total} />
          <Fact label={t("header.translated")} value={stats.translated} />
          <div className="flex w-24 flex-col gap-1">
            <dt className="text-xs text-ink-3">{t("header.progress")}</dt>
            <dd className="flex flex-col gap-1.5">
              <span className="font-mono text-[15px] font-medium tabular-nums">{stats.percent}&nbsp;%</span>
              <span
                className="block h-1 overflow-hidden rounded-full bg-line"
                role="progressbar"
                aria-label={t("statusBar.percentage", { percent: stats.percent })}
                aria-valuemin={0}
                aria-valuemax={100}
                aria-valuenow={stats.percent}
              >
                <span className="block h-full bg-brand transition-[width] duration-(--motion-slow)" style={{ width: `${stats.percent}%` }} />
              </span>
            </dd>
          </div>
        </dl>
      </div>

      <div
        role="toolbar"
        aria-label={t("toolbar.filter")}
        className="flex h-12 items-center gap-3 border-y border-line px-6"
      >
        <div
          role="radiogroup"
          aria-label={t("sidebar.filterByStatus")}
          className="flex shrink-0 gap-0.5 rounded-[8px] border border-line-field bg-surface-1 p-0.5"
        >
          {FILTERS.map((status) => {
            const checked = statusFilter === status;
            const count = status ? stats.byStatus[status] : stats.total;
            return (
              <button
                key={status ?? "all"}
                type="button"
                role="radio"
                aria-checked={checked}
                onClick={() => setStatusFilter(status)}
                className={cn(
                  "inline-flex h-[26px] items-center gap-[7px] rounded-md px-2.5 text-[12.5px] whitespace-nowrap outline-none transition-colors duration-(--motion-fast) focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ink",
                  checked ? "bg-surface-3 text-ink" : "text-ink-3 hover:text-ink",
                )}
              >
                {status && <StatusGlyph status={status} className="size-3" />}
                {t(status ? `status.${status}` : "status.all")}
                <span className={cn("font-mono text-[11.5px] tabular-nums", checked ? "text-ink-2" : "text-ink-4")}>
                  {count}
                </span>
              </button>
            );
          })}
        </div>

        <div className="ml-auto flex min-w-0 flex-1 items-center justify-end gap-2">
          <label className="flex h-[30px] w-full max-w-[280px] min-w-[140px] items-center gap-2 rounded-md border border-line-field bg-surface-2 pr-1.5 pl-2.5 focus-within:border-line-chip hover:border-line-strong">
            <Search aria-hidden="true" className="size-3.5 shrink-0 text-ink-3" strokeWidth={1.8} />
            <input
              type="search"
              value={searchQuery}
              onChange={(event) => setSearchQuery(event.target.value)}
              placeholder={t("common:search")}
              aria-label={t("common:search")}
              className="h-7 min-w-0 flex-1 bg-transparent text-sm text-ink outline-none placeholder:text-ink-4"
            />
          </label>
          <Button
            variant="ghost"
            size="icon"
            aria-label={t("toolbar.findReplace")}
            title={`${t("toolbar.findReplace")} · ${t("toolbar.findReplaceShortcut")}`}
            aria-pressed={showFindReplace}
            onClick={toggleFindReplace}
            className={cn(showFindReplace && "border-line-chip bg-surface-3 text-ink")}
          >
            <Replace strokeWidth={1.6} />
          </Button>
        </div>
      </div>
    </div>
  );
}

function Fact({ label, value }: { label: string; value: number }) {
  return (
    <div className="flex flex-col gap-1">
      <dt className="text-xs text-ink-3">{label}</dt>
      <dd className="font-mono text-[15px] font-medium tabular-nums">{value}</dd>
    </div>
  );
}
