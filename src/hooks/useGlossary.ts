import { useCallback, useState } from "react";
import { useProjectStore } from "@/stores/projectStore";
import * as ipc from "@/lib/ipc";

/**
 * Hook for glossary CRUD operations.
 * Wraps IPC calls and updates the local project store.
 */
export function useGlossary() {
  const project = useProjectStore((s) => s.project);
  const replaceProject = useProjectStore((s) => s.replaceProject);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const glossary = project?.glossary ?? [];

  const add = useCallback(
    async (source: string, target: string, caseSensitive: boolean) => {
      setIsLoading(true);
      setError(null);
      try {
        const updated = await ipc.glossaryAdd(source, target, caseSensitive);
        if (project) {
          replaceProject({
            ...project,
            glossary: updated,
            updatedAt: new Date().toISOString(),
          });
        }
        return updated;
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        setError(msg);
        throw e;
      } finally {
        setIsLoading(false);
      }
    },
    [project, replaceProject],
  );

  const update = useCallback(
    async (source: string, target: string, caseSensitive: boolean) => {
      setIsLoading(true);
      setError(null);
      try {
        const updated = await ipc.glossaryUpdate(source, target, caseSensitive);
        if (project) {
          replaceProject({
            ...project,
            glossary: updated,
            updatedAt: new Date().toISOString(),
          });
        }
        return updated;
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        setError(msg);
        throw e;
      } finally {
        setIsLoading(false);
      }
    },
    [project, replaceProject],
  );

  const remove = useCallback(
    async (source: string) => {
      setIsLoading(true);
      setError(null);
      try {
        const updated = await ipc.glossaryRemove(source);
        if (project) {
          replaceProject({
            ...project,
            glossary: updated,
            updatedAt: new Date().toISOString(),
          });
        }
        return updated;
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        setError(msg);
        throw e;
      } finally {
        setIsLoading(false);
      }
    },
    [project, replaceProject],
  );

  const refresh = useCallback(async () => {
    try {
      const terms = await ipc.glossaryList();
      if (project) {
        replaceProject({
          ...project,
          glossary: terms,
          updatedAt: new Date().toISOString(),
        });
      }
      return terms;
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      setError(msg);
      throw e;
    }
  }, [project, replaceProject]);

  return {
    glossary,
    isLoading,
    error,
    add,
    update,
    remove,
    refresh,
  };
}
