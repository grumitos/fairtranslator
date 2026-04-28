import { useCallback, useState } from "react";
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
import type { EntryStatus } from "@/types/domain";

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
  const [showGlossary, setShowGlossary] = useState(false);
  const [showStyleGuide, setShowStyleGuide] = useState(false);
  const [showBatchTranslate, setShowBatchTranslate] = useState(false);

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

    const selected = await dialogOpen({ directory: true, title: t("common:openProject"), filters: [] });
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

  const handleExport = useCallback(async () => {
    const selected = await dialogOpen({ directory: true, title: t("common:export.selectFolder") });
    if (selected && typeof selected === "string") {
      try {
        const result = await exportProject(selected);
        toast.success(t("common:export.success"), {
          description: t("common:export.successDetail", {
            files: result.filesExported,
            applied: result.entriesApplied,
            total: result.totalEntries,
          }),
        });
        if (result.warnings.length > 0) {
          toast.warning(t("common:export.warning"), {
            description: `${t("common:export.warningDetail", {
              count: result.warnings.length,
            })} ${result.warnings.slice(0, 2).join(" ")}`.trim(),
          });
        }
      } catch (error) {
        toast.error(t("common:error"), {
          description: error instanceof Error ? error.message : String(error),
        });
      }
    }
  }, [exportProject, t]);

  const handleThemeToggle = useCallback(() => {
    setTheme(theme === "dark" ? "light" : "dark");
  }, [theme, setTheme]);

  const projectUnavailable = !project || isLoading;

  return (
    <>
      <header className="flex h-11 items-center gap-1 border-b bg-card/95 px-1.5 shadow-xs">
        <div className="flex min-w-0 flex-1 items-center gap-1 overflow-x-auto overflow-y-hidden">
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
              label={`${t("editor:toolbar.findReplace")} - ${t("editor:toolbar.findReplaceShortcut")}`}
              onClick={toggleFindReplace}
              disabled={!project}
              active={showFindReplace}
            >
              <Replace className="size-4" />
            </IconAction>
          </div>

          <Separator orientation="vertical" className="mx-1 h-6 shrink-0" />

          <div className="relative flex shrink-0 items-center">
            <Search className="pointer-events-none absolute left-2.5 size-3.5 text-muted-foreground" />
            <Input
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder={t("common:search")}
              className="h-8 w-52 pl-7 text-xs"
              disabled={!project}
              aria-label={t("common:search")}
            />
          </div>

          <Select
            value={statusFilter ?? "all"}
            onValueChange={(value) => setStatusFilter(value === "all" ? null : (value as EntryStatus))}
            disabled={!project}
          >
            <SelectTrigger className="h-8 w-36 shrink-0 gap-1.5 px-2.5 text-xs" aria-label={t("editor:toolbar.filter")}>
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

        <div className="ml-1 flex shrink-0 items-center gap-1">
          <IconAction label={t("editor:toolbar.toggleTheme")} onClick={handleThemeToggle}>
            {theme === "dark" ? <Sun className="size-4" /> : <Moon className="size-4" />}
          </IconAction>
          <IconAction label={t("editor:toolbar.settings")} onClick={() => setShowSettings(true)}>
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
      <SettingsPanel open={showSettings} onOpenChange={setShowSettings} />
      <GlossaryPanel open={showGlossary} onOpenChange={setShowGlossary} />
      <StyleGuidePanel open={showStyleGuide} onOpenChange={setShowStyleGuide} />
      <BatchTranslatePanel open={showBatchTranslate} onOpenChange={setShowBatchTranslate} />
    </>
  );
}
