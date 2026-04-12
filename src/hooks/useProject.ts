import { useCallback } from "react";
import { useProjectStore } from "@/stores/projectStore";
import * as ipc from "@/lib/ipc";
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
        await ipc.projectSave(path);
        const refreshedProject = await ipc.projectGet();
        replaceProject(refreshedProject, { markDirty: false });
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        setError(msg);
        throw e;
      } finally {
        setLoading(false);
      }
    },
    [replaceProject, setLoading, setError],
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
      const currentProject = store.project;
      const previousDirty = store.isDirty;
      const update: ProjectEntryUpdate = {
        filePath,
        entryId,
        translation,
        status,
        notes,
      };
      const normalizedUpdate = normalizeProjectEntryUpdate(update);
      if (currentProject) {
        replaceProject(applyEntryUpdates(currentProject, [normalizedUpdate]), { markDirty: true });
      }

      try {
        await ipc.projectUpdateEntry(
          normalizedUpdate.filePath,
          normalizedUpdate.entryId,
          normalizedUpdate.translation,
          normalizedUpdate.status,
          normalizedUpdate.notes,
        );
      } catch (e) {
        if (currentProject) {
          replaceProject(currentProject, { markDirty: previousDirty });
        }
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
      const currentProject = store.project;
      const previousDirty = store.isDirty;
      const normalizedUpdates = updates.map(normalizeProjectEntryUpdate);
      if (currentProject) {
        replaceProject(applyEntryUpdates(currentProject, normalizedUpdates), { markDirty: true });
      }

      try {
        await ipc.projectUpdateEntries(normalizedUpdates);
      } catch (e) {
        if (currentProject) {
          replaceProject(currentProject, { markDirty: previousDirty });
        }
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
