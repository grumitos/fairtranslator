import { useCallback, useState } from "react";
import { isTauri } from "@tauri-apps/api/core";
import { useProjectStore } from "@/stores/projectStore";
import * as ipc from "@/lib/ipc";
import { trackEdit } from "@/lib/projectSync";
import { areStyleGuideRulesEqual, normalizeStyleGuideRules } from "@/lib/styleGuideRules";

const NO_RULES: string[] = [];

export function useStyleGuide() {
  const styleGuide = useProjectStore((s) => s.project?.styleGuide) ?? NO_RULES;
  const patchProject = useProjectStore((s) => s.patchProject);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    const current = useProjectStore.getState().project;
    if (!current) {
      return [];
    }

    setIsLoading(true);
    setError(null);
    try {
      if (!isTauri()) {
        return current.styleGuide;
      }

      const items = normalizeStyleGuideRules(await ipc.styleGuideList());
      const latest = useProjectStore.getState().project;
      if (latest && !areStyleGuideRulesEqual(latest.styleGuide, items)) {
        patchProject({ styleGuide: items }, { markDirty: "preserve", projectId: current.id });
      }
      return items;
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      setError(message);
      throw e;
    } finally {
      setIsLoading(false);
    }
  }, [patchProject]);

  const replace = useCallback(
    async (items: string[]) => {
      const current = useProjectStore.getState().project;
      if (!current) {
        throw new Error("No hay ningún proyecto abierto");
      }

      const normalized = normalizeStyleGuideRules(items);
      setIsLoading(true);
      setError(null);
      try {
        if (!isTauri()) {
          patchProject({ styleGuide: normalized }, { markDirty: true, projectId: current.id });
          return normalized;
        }

        // The backend rewrites only project.json (when the project exists on disk): pending entry
        // edits stay unsaved, so the dirty flag is preserved rather than cleared.
        const saved = await trackEdit(ipc.styleGuideReplace(normalized));
        patchProject({ styleGuide: saved }, { markDirty: "preserve", projectId: current.id });
        return saved;
      } catch (e) {
        const message = e instanceof Error ? e.message : String(e);
        setError(message);
        throw e;
      } finally {
        setIsLoading(false);
      }
    },
    [patchProject],
  );

  return {
    styleGuide,
    isLoading,
    error,
    refresh,
    replace,
  };
}
