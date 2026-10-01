import { useEffect } from "react";
import { useProject } from "@/hooks/useProject";
import { useEditorStore } from "@/stores/editorStore";
import { useProjectStore } from "@/stores/projectStore";
import { NewProjectDialog } from "@/components/layout/NewProjectDialog";
import { SettingsPanel } from "@/components/layout/SettingsPanel";
import { ContextPanel } from "@/components/layout/ContextPanel";
import { BatchTranslatePanel } from "@/components/layout/BatchTranslatePanel";
import { ExportPanel } from "@/components/layout/ExportPanel";
import { TitleBar } from "@/components/layout/TitleBar";
import { usePanelStore, type Panel } from "@/components/layout/panelStore";
import { useProjectCommands } from "@/components/layout/useProjectCommands";
import { useProjectLocationStore } from "@/lib/projectLocation";

function isTypingTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  if (target.closest("[role='dialog']") || target.isContentEditable) return true;
  return Boolean(target.closest("input, textarea, select, [role='textbox']"));
}

/*
 * Controlador de la ventana: pinta la barra de título y abre los paneles del proyecto
 * (nuevo, abrir, ajustes, contexto, lotes, exportar). Solo hay un panel abierto a la vez.
 */
export function Toolbar() {
  const { project, isDirty, isLoading } = useProject();
  const { saveProject, saveProjectAs, openProject } = useProjectCommands();
  const location = useProjectLocationStore((s) => s.location?.path ?? null);
  const setStatusFilter = useEditorStore((s) => s.setStatusFilter);
  const panel = usePanelStore((s) => s.panel);
  const settingsProvider = usePanelStore((s) => s.settingsProvider);
  const contextTab = usePanelStore((s) => s.contextTab);
  const openPanel = usePanelStore((s) => s.open);
  const openSettings = usePanelStore((s) => s.openSettings);
  const openContext = usePanelStore((s) => s.openContext);
  const setContextTab = usePanelStore((s) => s.setContextTab);
  const closePanel = usePanelStore((s) => s.close);

  // Ctrl+N y Ctrl+O funcionan también sin proyecto abierto; Ctrl+S y la búsqueda viven en useKeyboardShortcuts.
  useEffect(() => {
    const handler = (event: KeyboardEvent) => {
      if (!(event.ctrlKey || event.metaKey) || event.altKey || isTypingTarget(event.target)) return;
      const key = event.key.toLowerCase();
      if (key === "n") {
        event.preventDefault();
        openPanel("newProject");
      } else if (key === "o") {
        event.preventDefault();
        void openProject();
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [openPanel, openProject]);

  const busy = !project || isLoading;
  const setPanelOpen = (target: Panel) => (open: boolean) => {
    if (open) return;
    if (usePanelStore.getState().panel === target) closePanel();
  };

  return (
    <>
      <TitleBar
        project={
          project
            ? { name: project.name, engine: project.engine, sourceLang: project.sourceLang, targetLang: project.targetLang }
            : null
        }
        isDirty={isDirty}
        busy={busy}
        onNewProject={() => openPanel("newProject")}
        onOpenProject={() => void openProject()}
        onSave={() => void saveProject()}
        onSaveAs={() => void saveProjectAs()}
        location={location}
        onExport={() => openPanel("export")}
        onTranslate={() => openPanel("batch")}
        onGlossary={() => openContext("glossary")}
        onSettings={() => openSettings(undefined)}
      />

      <NewProjectDialog open={panel === "newProject"} onOpenChange={setPanelOpen("newProject")} />
      <SettingsPanel
        open={panel === "settings"}
        onOpenChange={setPanelOpen("settings")}
        initialProvider={settingsProvider}
      />
      <ContextPanel
        open={panel === "context"}
        onOpenChange={setPanelOpen("context")}
        tab={contextTab}
        onTabChange={setContextTab}
      />
      <BatchTranslatePanel
        open={panel === "batch"}
        onOpenChange={setPanelOpen("batch")}
        onConfigureProvider={(provider) => openSettings(provider)}
      />
      <ExportPanel
        open={panel === "export"}
        onOpenChange={setPanelOpen("export")}
        onReviewDrafts={() => {
          setStatusFilter("draft");
          closePanel();
        }}
        onOpenEntry={(filePath, entryId) => {
          // Sin filtros, para que la entrada sea visible en la tabla del archivo.
          const file = useProjectStore.getState().project?.files.find((f) => f.relativePath === filePath) ?? null;
          const editor = useEditorStore.getState();
          editor.setStatusFilter(null);
          editor.setSearchQuery("");
          useProjectStore.getState().setActiveFile(file);
          editor.setScrollToEntryId(entryId);
          closePanel();
        }}
      />
    </>
  );
}
