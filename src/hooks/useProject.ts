import { useCallback } from "react";
import { useProjectStore } from "@/stores/projectStore";
import * as ipc from "@/lib/ipc";
import {
  flushPendingEdits,
  persistProject,
  refreshProjectFromBackend,
  trackEdit,
} from "@/lib/projectSync";
import { refreshProjectLocation } from "@/lib/projectLocation";
import { normalizeProjectEntryUpdate } from "@/lib/translationState";
import type {
  Project,
  EntryStatus,
  ExportResult,
  ProjectEntryUpdate,
} from "@/types/domain";

function applyEntryUpdates(project: Project, updates: ProjectEntryUpdate[]): Project {
  const updatesByFile = new Map<string, Map<string, ProjectEntryUpdate>>();

  for (const update of updates) {
    const normalizedUpdate = normalizeProjectEntryUpdate(update);
    let fileUpdates = updatesByFile.get(normalizedUpdate.filePath);
    if (!fileUpdates) {
      fileUpdates = new Map<string, ProjectEntryUpdate>();
      updatesByFile.set(normalizedUpdate.filePath, fileUpdates);
    }
    fileUpdates.set(normalizedUpdate.entryId, normalizedUpdate);
  }

  return {
    ...project,
    files: project.files.map((file) => {
      const fileUpdates = updatesByFile.get(file.relativePath);
      if (!fileUpdates) {
        return file;
      }

      return {
        ...file,
        entries: file.entries.map((entry) => {
          const update = fileUpdates.get(entry.id);
          if (!update) {
            return entry;
          }
          return {
            ...entry,
            translation: update.translation,
            status: update.status,
            notes: update.notes ?? entry.notes,
          };
        }),
      };
    }),
    updatedAt: new Date().toISOString(),
  };
}

/**
 * The backend rejected an edit that the UI already shows. Adopt the backend copy instead of
 * restoring an older snapshot, which would also discard newer edits that were accepted.
 */
async function resyncAfterFailedEdit(previousDirty: boolean, revisionAfterEdit: number) {
  const editedSince = useProjectStore.getState().editRevision !== revisionAfterEdit;
  try {
    await refreshProjectFromBackend(previousDirty || editedSince);
  } catch {
    // The rejection that brought us here is the error worth reporting; keep the UI state if the backend cannot answer.
  }
}

/**
 * Hook that combines project store state with IPC operations.
 * Components use this hook — never raw invoke() or store directly.
 */
export function useProject() {
  const project = useProjectStore((s) => s.project);
  const activeFile = useProjectStore((s) => s.activeFile);
  const isDirty = useProjectStore((s) => s.isDirty);
  const isLoading = useProjectStore((s) => s.isLoading);
  const error = useProjectStore((s) => s.error);
  const setProject = useProjectStore((s) => s.setProject);
  const replaceProject = useProjectStore((s) => s.replaceProject);
  const setActiveFile = useProjectStore((s) => s.setActiveFile);
  const setLoading = useProjectStore((s) => s.setLoading);
  const setError = useProjectStore((s) => s.setError);

  const create = useCallback(
    async (gameDir: string, name: string, sourceLang: string, targetLang: string) => {
      setLoading(true);
      setError(null);
      try {
        const proj = await ipc.projectCreate(gameDir, name, sourceLang, targetLang);
        setProject(proj, { markDirty: true });
        void refreshProjectLocation();
        return proj;
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        setError(msg);
        throw e;
      } finally {
        setLoading(false);
      }
    },
    [setProject, setLoading, setError],
  );

  const open = useCallback(
    async (path: string) => {
      setLoading(true);
      setError(null);
      try {
        const proj = await ipc.projectOpen(path);
        setProject(proj, { markDirty: false });
        void refreshProjectLocation();
        return proj;
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        setError(msg);
        throw e;
      } finally {
        setLoading(false);
      }
    },
    [setProject, setLoading, setError],
  );

  const save = useCallback(
    async (path?: string) => {
      setLoading(true);
      setError(null);
      try {
        await persistProject(() => ipc.projectSave(path));
        void refreshProjectLocation();
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        setError(msg);
        throw e;
      } finally {
        setLoading(false);
      }
    },
    [setLoading, setError],
  );

  const updateEntry = useCallback(
    async (
      filePath: string,
      entryId: string,
      translation: string,
      status: EntryStatus,
      notes?: string,
    ) => {
      const store = useProjectStore.getState();
      const previousDirty = store.isDirty;
      const normalizedUpdate = normalizeProjectEntryUpdate({
        filePath,
        entryId,
        translation,
        status,
        notes,
      });
      if (store.project) {
        replaceProject(applyEntryUpdates(store.project, [normalizedUpdate]), { markDirty: true });
      }
      const revisionAfterEdit = useProjectStore.getState().editRevision;

      try {
        await trackEdit(
          ipc.projectUpdateEntry(
            normalizedUpdate.filePath,
            normalizedUpdate.entryId,
            normalizedUpdate.translation,
            normalizedUpdate.status,
            normalizedUpdate.notes,
          ),
        );
      } catch (e) {
        await resyncAfterFailedEdit(previousDirty, revisionAfterEdit);
        const msg = e instanceof Error ? e.message : String(e);
        setError(msg);
        throw e;
      }
    },
    [replaceProject, setError],
  );

  const updateEntriesBatch = useCallback(
    async (updates: ProjectEntryUpdate[]) => {
      if (updates.length === 0) {
        return;
      }

      const store = useProjectStore.getState();
      const previousDirty = store.isDirty;
      const normalizedUpdates = updates.map(normalizeProjectEntryUpdate);
      if (store.project) {
        replaceProject(applyEntryUpdates(store.project, normalizedUpdates), { markDirty: true });
      }
      const revisionAfterEdit = useProjectStore.getState().editRevision;

      try {
        await trackEdit(ipc.projectUpdateEntries(normalizedUpdates));
      } catch (e) {
        await resyncAfterFailedEdit(previousDirty, revisionAfterEdit);
        const msg = e instanceof Error ? e.message : String(e);
        setError(msg);
        throw e;
      }
    },
    [replaceProject, setError],
  );

  const exportProject = useCallback(
    async (outputDir: string): Promise<ExportResult> => {
      setLoading(true);
      setError(null);
      try {
        // The exporter reads the backend copy: make sure every edit shown in the grid has reached it.
        await flushPendingEdits();
        const result = await ipc.exportProject(outputDir);
        return result;
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        setError(msg);
        throw e;
      } finally {
        setLoading(false);
      }
    },
    [setLoading, setError],
  );

  return {
    project,
    activeFile,
    isDirty,
    isLoading,
    error,
    setActiveFile,
    create,
    open,
    save,
    updateEntry,
    updateEntriesBatch,
    exportProject,
  };
}
