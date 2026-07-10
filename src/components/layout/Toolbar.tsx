import { useCallback, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { open as dialogOpen } from "@tauri-apps/plugin-dialog";
import { toast } from "sonner";
import {
  FilePlus,
  FolderOpen,
  Save,
  Search,
  Filter,
  Download,
  Settings,
  BookOpen,
  FileText,
  Languages,
  Replace,
  Sun,
  Moon,
} from "lucide-react";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Separator } from "@/components/ui/separator";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { useProject } from "@/hooks/useProject";
import { useEditorStore } from "@/stores/editorStore";
import { useSettingsStore } from "@/stores/settingsStore";
import { NewProjectDialog } from "@/components/layout/NewProjectDialog";
import { SettingsPanel } from "@/components/layout/SettingsPanel";
import { GlossaryPanel } from "@/components/layout/GlossaryPanel";
import { StyleGuidePanel } from "@/components/layout/StyleGuidePanel";
import { BatchTranslatePanel } from "@/components/layout/BatchTranslatePanel";
import { IconAction } from "@/components/layout/toolbar/IconAction";
import { isTranslatableEntry } from "@/lib/translationMetrics";
import type { EntryStatus, ExportResult, ProviderType } from "@/types/domain";

interface ExportSummary {
  outputDir: string;
  result: ExportResult;
}

export function Toolbar() {
  const { t } = useTranslation(["common", "editor"]);
  const { project, save, isDirty, isLoading, open, exportProject } = useProject();
  const searchQuery = useEditorStore((s) => s.searchQuery);
  const setSearchQuery = useEditorStore((s) => s.setSearchQuery);
  const statusFilter = useEditorStore((s) => s.statusFilter);
  const setStatusFilter = useEditorStore((s) => s.setStatusFilter);
  const toggleFindReplace = useEditorStore((s) => s.toggleFindReplace);
  const showFindReplace = useEditorStore((s) => s.showFindReplace);
  const theme = useSettingsStore((s) => s.theme);
  const setTheme = useSettingsStore((s) => s.setTheme);
  const [showNewDialog, setShowNewDialog] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [settingsProvider, setSettingsProvider] = useState<ProviderType>();
  const [showGlossary, setShowGlossary] = useState(false);
  const [showStyleGuide, setShowStyleGuide] = useState(false);
  const [showBatchTranslate, setShowBatchTranslate] = useState(false);
  const [showExportReadiness, setShowExportReadiness] = useState(false);
  const [exportSummary, setExportSummary] = useState<ExportSummary | null>(null);

  const exportStatusCounts = useMemo<Record<EntryStatus, number>>(() => {
    const counts: Record<EntryStatus, number> = {
      untranslated: 0,
      draft: 0,
      reviewed: 0,
      final: 0,
    };
    for (const file of project?.files ?? []) {
      for (const entry of file.entries) {
        if (isTranslatableEntry(entry)) {
          counts[entry.status] += 1;
        }
      }
    }
    return counts;
  }, [project]);

  const handleSave = useCallback(async () => {
    try {
      await save();
    } catch (error) {
      toast.error(t("common:error"), {
        description: error instanceof Error ? error.message : String(error),
      });
    }
  }, [save, t]);

  const handleOpen = useCallback(async () => {
    if (isDirty && !window.confirm(t("common:unsavedChanges.openProject"))) {
      return;
    }

    const selected = await dialogOpen({ directory: true, title: t("common:openProjectFolder"), filters: [] });
    if (selected && typeof selected === "string") {
      try {
        await open(selected);
      } catch (error) {
        toast.error(t("common:error"), {
          description: error instanceof Error ? error.message : String(error),
        });
      }
    }
  }, [isDirty, open, t]);

  const runExport = useCallback(async () => {
    const selected = await dialogOpen({ directory: true, title: t("common:export.selectFolder") });
    if (selected && typeof selected === "string") {
      try {
        const result = await exportProject(selected);
        setExportSummary({ outputDir: selected, result });
      } catch (error) {
        toast.error(t("common:error"), {
          description: error instanceof Error ? error.message : String(error),
        });
      }
    }
  }, [exportProject, t]);

  const handleExport = useCallback(() => {
    if (exportStatusCounts.untranslated > 0 || exportStatusCounts.draft > 0) {
      setShowExportReadiness(true);
      return;
    }
    void runExport();
  }, [exportStatusCounts, runExport]);

  const handleThemeToggle = useCallback(() => {
    setTheme(theme === "dark" ? "light" : "dark");
  }, [theme, setTheme]);

  const projectUnavailable = !project || isLoading;

  return (
    <>
      <header className="grid min-h-11 shrink-0 grid-cols-[minmax(0,1fr)_auto] items-center gap-1 border-b bg-card/95 px-1.5 py-1 shadow-xs md:flex md:h-11 md:flex-nowrap md:py-0">
        <div className="col-start-1 row-start-1 flex min-w-0 items-center gap-1 overflow-x-auto overflow-y-hidden md:col-auto md:row-auto">
          <div className="flex shrink-0 items-center gap-1 rounded-md border bg-background/65 p-0.5">
            <IconAction label={t("common:newProject")} onClick={() => setShowNewDialog(true)}>
              <FilePlus className="size-4" />
            </IconAction>
            <IconAction label={t("common:openProject")} onClick={handleOpen}>
              <FolderOpen className="size-4" />
            </IconAction>
            <IconAction label={t("common:save")} onClick={handleSave} disabled={projectUnavailable}>
              <Save className="size-4" />
            </IconAction>
            <IconAction
              label={t("common:export.button")}
              onClick={handleExport}
              disabled={projectUnavailable}
            >
              <Download className="size-4" />
            </IconAction>
          </div>

          <Separator orientation="vertical" className="mx-1 h-6 shrink-0" />

          <div className="flex shrink-0 items-center gap-1 rounded-md border bg-background/65 p-0.5">
            <IconAction
              label={t("editor:toolbar.batchTranslate")}
              onClick={() => setShowBatchTranslate(true)}
              disabled={!project}
            >
              <Languages className="size-4" />
            </IconAction>
            <IconAction
              label={t("editor:toolbar.glossary")}
              onClick={() => setShowGlossary(true)}
              disabled={!project}
            >
              <BookOpen className="size-4" />
            </IconAction>
            <IconAction
              label={t("editor:toolbar.styleGuide")}
              onClick={() => setShowStyleGuide(true)}
              disabled={!project}
            >
              <FileText className="size-4" />
            </IconAction>
            <IconAction
              label={t("editor:toolbar.findReplace")}
              onClick={toggleFindReplace}
              disabled={!project}
              active={showFindReplace}
            >
              <Replace className="size-4" />
            </IconAction>
          </div>

        </div>

        <div className="col-span-2 row-start-2 flex min-w-0 items-center gap-1 md:col-auto md:row-auto md:min-w-80 md:flex-1">
          <div className="relative flex min-w-0 flex-1 items-center md:flex-none">
            <Search className="pointer-events-none absolute left-2.5 size-3.5 text-muted-foreground" />
            <Input
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder={t("common:search")}
              className="h-8 w-full pl-7 text-xs md:w-52"
              disabled={!project}
              aria-label={t("common:search")}
            />
          </div>

          <Select
            value={statusFilter ?? "all"}
            onValueChange={(value) => setStatusFilter(value === "all" ? null : (value as EntryStatus))}
            disabled={!project}
          >
            <SelectTrigger className="h-8 w-32 shrink-0 gap-1.5 px-2.5 text-xs md:w-36" aria-label={t("editor:toolbar.filter")}>
              <Filter className="size-3.5 text-muted-foreground" />
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="all">{t("editor:status.all")}</SelectItem>
              <SelectItem value="untranslated">{t("editor:status.untranslated")}</SelectItem>
              <SelectItem value="draft">{t("editor:status.draft")}</SelectItem>
              <SelectItem value="reviewed">{t("editor:status.reviewed")}</SelectItem>
              <SelectItem value="final">{t("editor:status.final")}</SelectItem>
            </SelectContent>
          </Select>
        </div>

        <div className="col-start-2 row-start-1 ml-1 flex shrink-0 items-center gap-1 md:col-auto md:row-auto">
          <IconAction label={t("editor:toolbar.toggleTheme")} onClick={handleThemeToggle}>
            {theme === "dark" ? <Sun className="size-4" /> : <Moon className="size-4" />}
          </IconAction>
          <IconAction
            label={t("editor:toolbar.settings")}
            onClick={() => {
              setSettingsProvider(undefined);
              setShowSettings(true);
            }}
          >
            <Settings className="size-4" />
          </IconAction>
          {project && (
            <div className="hidden min-w-0 items-center gap-2 rounded-md border bg-background/65 px-2 py-1 md:flex">
              {isDirty && (
                <span className="ft-status-draft rounded-full px-1.5 py-0.5 text-[10px] font-semibold">
                  {t("common:unsavedChanges.badge")}
                </span>
              )}
              <span className="max-w-48 truncate text-xs text-muted-foreground">{project.name}</span>
            </div>
          )}
        </div>
      </header>

      <NewProjectDialog open={showNewDialog} onOpenChange={setShowNewDialog} />
      <SettingsPanel
        open={showSettings}
        onOpenChange={setShowSettings}
        initialProvider={settingsProvider}
      />
      <GlossaryPanel open={showGlossary} onOpenChange={setShowGlossary} />
      <StyleGuidePanel open={showStyleGuide} onOpenChange={setShowStyleGuide} />
      <BatchTranslatePanel
        open={showBatchTranslate}
        onOpenChange={setShowBatchTranslate}
        onConfigureProvider={(provider) => {
          setSettingsProvider(provider);
          setShowBatchTranslate(false);
          setShowSettings(true);
        }}
      />

      <Dialog open={showExportReadiness} onOpenChange={setShowExportReadiness}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>{t("common:export.readiness.title")}</DialogTitle>
            <DialogDescription>{t("common:export.readiness.description")}</DialogDescription>
          </DialogHeader>
          <div className="grid grid-cols-2 gap-2 text-sm">
            <p className="rounded-md border bg-muted/35 px-3 py-2">
              {t("common:export.readiness.untranslated", { count: exportStatusCounts.untranslated })}
            </p>
            <p className="rounded-md border bg-muted/35 px-3 py-2">
              {t("common:export.readiness.draft", { count: exportStatusCounts.draft })}
            </p>
            <p className="rounded-md border bg-muted/35 px-3 py-2">
              {t("common:export.readiness.reviewed", { count: exportStatusCounts.reviewed })}
            </p>
            <p className="rounded-md border bg-muted/35 px-3 py-2">
              {t("common:export.readiness.final", { count: exportStatusCounts.final })}
            </p>
          </div>
          <DialogFooter className="sm:flex-wrap">
            <Button variant="outline" onClick={() => setShowExportReadiness(false)}>
              {t("common:cancel")}
            </Button>
            <Button
              variant="secondary"
              disabled={exportStatusCounts.draft === 0}
              onClick={() => {
                setStatusFilter("draft");
                setShowExportReadiness(false);
              }}
            >
              {t("common:export.readiness.reviewDrafts")}
            </Button>
            <Button
              onClick={() => {
                setShowExportReadiness(false);
                void runExport();
              }}
            >
              {t("common:export.readiness.exportAnyway")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog
        open={exportSummary !== null}
        onOpenChange={(nextOpen) => {
          if (!nextOpen) setExportSummary(null);
        }}
      >
        {exportSummary && (
          <DialogContent className="sm:max-w-lg">
            <DialogHeader>
              <DialogTitle>
                {t(exportSummary.result.warnings.length > 0
                  ? "common:export.warning"
                  : "common:export.success")}
              </DialogTitle>
              <DialogDescription>
                {t("common:export.successDetail", {
                  files: exportSummary.result.filesExported,
                  applied: exportSummary.result.entriesApplied,
                  total: exportSummary.result.totalEntries,
                })}
              </DialogDescription>
            </DialogHeader>
            <div className="space-y-3 text-sm">
              <div>
                <p className="text-xs font-medium text-muted-foreground">
                  {t("common:export.result.destination")}
                </p>
                <p className="mt-1 break-all rounded-md border bg-muted/35 px-3 py-2 select-text">
                  {exportSummary.outputDir}
                </p>
              </div>
              {exportSummary.result.warnings.length > 0 ? (
                <div
                  className="max-h-48 overflow-y-auto rounded-md border p-3"
                  aria-label={t("common:export.result.warnings", {
                    count: exportSummary.result.warnings.length,
                  })}
                >
                  <p className="mb-2 text-xs font-medium text-warning">
                    {t("common:export.result.warnings", {
                      count: exportSummary.result.warnings.length,
                    })}
                  </p>
                  <ul className="list-disc space-y-1 pl-4 text-xs text-muted-foreground">
                    {exportSummary.result.warnings.map((warning, index) => (
                      <li key={`${index}-${warning}`}>{warning}</li>
                    ))}
                  </ul>
                </div>
              ) : (
                <p className="rounded-md border bg-muted/35 px-3 py-2 text-xs text-muted-foreground">
                  {t("common:export.result.noWarnings")}
                </p>
              )}
            </div>
            <DialogFooter>
              <Button onClick={() => setExportSummary(null)}>{t("common:close")}</Button>
            </DialogFooter>
          </DialogContent>
        )}
      </Dialog>
    </>
  );
}
