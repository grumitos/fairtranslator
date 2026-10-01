import { useCallback } from "react";
import { useTranslation } from "react-i18next";
import { open as dialogOpen, save as dialogSave } from "@tauri-apps/plugin-dialog";
import { toast } from "sonner";
import { useProject } from "@/hooks/useProject";
import { confirmAction } from "@/components/common/ConfirmDialog";
import * as ipc from "@/lib/ipc";
import { refreshProjectLocation, useProjectLocationStore } from "@/lib/projectLocation";
import { projectStats } from "@/lib/translationMetrics";
import { useProjectStore } from "@/stores/projectStore";
import { useSettingsStore } from "@/stores/settingsStore";
import type { Project } from "@/types/domain";

function errorDescription(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/** Anota el proyecto en "Recientes" con su progreso actual. */
export function rememberRecentProject(project: Project, path: string) {
  const { translated, total } = projectStats(project.files);
  useSettingsStore.getState().rememberProject({
    path,
    name: project.name,
    engine: project.engine,
    percent: total > 0 ? Math.round((translated / total) * 100) : 0,
    openedAt: new Date().toISOString(),
  });
}

// Tras guardar: la ruta final la decide el backend (puede añadir ".fairtl").
async function rememberSavedProject() {
  const location = await refreshProjectLocation();
  const project = useProjectStore.getState().project;
  if (project && location?.saved) rememberRecentProject(project, location.path);
}

// Órdenes de proyecto compartidas por la barra de título, la pantalla de inicio y los atajos.
export function useProjectCommands() {
  const { t } = useTranslation("common");
  const { save, isDirty, open } = useProject();

  const saveProjectAs = useCallback(async () => {
    const project = useProjectStore.getState().project;
    if (!project) return;
    const current = useProjectLocationStore.getState().location;
    const target = await dialogSave({
      title: t("saveAs"),
      defaultPath: current?.path ?? `${project.name}.fairtl`,
      filters: [{ name: "FairTranslator", extensions: ["fairtl"] }],
    });
    if (!target) return;
    try {
      await save(target);
      await rememberSavedProject();
    } catch (error) {
      toast.error(t("error"), { description: errorDescription(error) });
    }
  }, [save, t]);

  const saveProject = useCallback(async () => {
    // Sin ruta de guardado (no hay carpeta Documentos) el backend pide "Guardar como".
    const location = await ipc.projectLocation().catch(() => undefined);
    if (location === null) {
      await saveProjectAs();
      return;
    }
    try {
      await save();
      await rememberSavedProject();
    } catch (error) {
      toast.error(t("error"), { description: errorDescription(error) });
    }
  }, [save, saveProjectAs, t]);

  const confirmDiscard = useCallback(
    async () =>
      !isDirty ||
      confirmAction({ message: t("unsavedChanges.openProject"), confirmLabel: t("discard"), destructive: true }),
    [isDirty, t],
  );

  const openProjectAt = useCallback(
    async (path: string) => {
      try {
        const project = await open(path);
        const location = await refreshProjectLocation();
        rememberRecentProject(project, location?.path ?? path);
        return true;
      } catch (error) {
        toast.error(t("error"), { description: errorDescription(error) });
        return false;
      }
    },
    [open, t],
  );

  const openProject = useCallback(async () => {
    if (!(await confirmDiscard())) return;
    const selected = await dialogOpen({ directory: true, title: t("openProjectFolder"), filters: [] });
    if (selected && typeof selected === "string") await openProjectAt(selected);
  }, [confirmDiscard, openProjectAt, t]);

  const openRecentProject = useCallback(
    async (path: string) => {
      if (!(await confirmDiscard())) return false;
      return openProjectAt(path);
    },
    [confirmDiscard, openProjectAt],
  );

  return { saveProject, saveProjectAs, openProject, openRecentProject };
}
