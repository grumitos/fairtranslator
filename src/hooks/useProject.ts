import { useCallback } from "react";
import { useProjectStore } from "@/stores/projectStore";
import * as ipc from "@/lib/ipc";
import type { Project, EntryStatus, ExportResult } from "@/types/domain";

/**
 * Hook that combines project store state with IPC operations.
 * Components use this hook — never raw invoke() or store directly.
 */
export function useProject() {
  const project = useProjectStore((s) => s.project);
  const activeFile = useProjectStore((s) => s.activeFile);
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
        setProject(proj);
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
        setProject(proj);
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
        replaceProject(refreshedProject);
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
      try {
        await ipc.projectUpdateEntry(filePath, entryId, translation, status, notes);
        // Update local store to reflect change without re-fetching
        const store = useProjectStore.getState();
        const proj = store.project;
        if (!proj) return;

        const updatedFiles = proj.files.map((f) => {
          if (f.relativePath !== filePath) return f;
          return {
            ...f,
            entries: f.entries.map((e) => {
              if (e.id !== entryId) return e;
              return { ...e, translation, status, notes: notes ?? e.notes };
            }),
          };
        });

        const updatedProject: Project = {
          ...proj,
          files: updatedFiles,
          updatedAt: new Date().toISOString(),
        };
        replaceProject(updatedProject);
      } catch (e) {
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
    isLoading,
    error,
    setActiveFile,
    create,
    open,
    save,
    updateEntry,
    exportProject,
  };
}
