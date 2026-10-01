import { useCallback, useState } from "react";
import { useProjectStore } from "@/stores/projectStore";
import * as ipc from "@/lib/ipc";
import type { GlossaryTerm } from "@/types/domain";

const NO_TERMS: GlossaryTerm[] = [];

/**
 * Hook for glossary CRUD operations.
 * Wraps IPC calls and updates the local project store.
 */
export function useGlossary() {
  const glossary = useProjectStore((s) => s.project?.glossary) ?? NO_TERMS;
  const patchProject = useProjectStore((s) => s.patchProject);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  /**
   * Runs a glossary IPC call and stores the returned term list on the project that was open when
   * it started, patching the CURRENT store state so edits made while awaiting are not overwritten.
   */
  const mutate = useCallback(
    async (request: () => Promise<GlossaryTerm[]>) => {
      setIsLoading(true);
      setError(null);
      const projectId = useProjectStore.getState().project?.id;
      try {
        const updated = await request();
        patchProject(
          { glossary: updated, updatedAt: new Date().toISOString() },
          { markDirty: true, projectId },
        );
        return updated;
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        setError(msg);
        throw e;
      } finally {
        setIsLoading(false);
      }
    },
    [patchProject],
  );

  const add = useCallback(
    (source: string, target: string, caseSensitive: boolean) =>
      mutate(() => ipc.glossaryAdd(source, target, caseSensitive)),
    [mutate],
  );

  const update = useCallback(
    (source: string, target: string, caseSensitive: boolean) =>
      mutate(() => ipc.glossaryUpdate(source, target, caseSensitive)),
    [mutate],
  );

  const remove = useCallback(
    (source: string) => mutate(() => ipc.glossaryRemove(source)),
    [mutate],
  );

  const refresh = useCallback(async () => {
    const projectId = useProjectStore.getState().project?.id;
    try {
      const terms = await ipc.glossaryList();
      patchProject({ glossary: terms }, { markDirty: "preserve", projectId });
      return terms;
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      setError(msg);
      throw e;
    }
  }, [patchProject]);

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
