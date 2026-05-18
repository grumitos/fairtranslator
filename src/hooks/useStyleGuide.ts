import { useCallback, useState } from "react";
import { isTauri } from "@tauri-apps/api/core";
import { useProjectStore } from "@/stores/projectStore";
import * as ipc from "@/lib/ipc";
import { areStyleGuideRulesEqual, normalizeStyleGuideRules } from "@/lib/styleGuideRules";

export function useStyleGuide() {
  const project = useProjectStore((s) => s.project);
  const replaceProject = useProjectStore((s) => s.replaceProject);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const styleGuide = project?.styleGuide ?? [];

  const applyProjectUpdate = useCallback(
    (items: string[], markDirty: boolean | "preserve") => {
      if (!project) {
        return items;
      }

      if (areStyleGuideRulesEqual(project.styleGuide, items)) {
        return project.styleGuide;
      }

      replaceProject({
        ...project,
        styleGuide: items,
      }, { markDirty });
      return items;
    },
    [project, replaceProject],
  );

  const refresh = useCallback(async () => {
    if (!project) {
      return [];
    }

    setIsLoading(true);
    setError(null);
    try {
      if (!isTauri()) {
        return styleGuide;
      }

      const items = await ipc.styleGuideList();
      const normalized = normalizeStyleGuideRules(items);
      return applyProjectUpdate(normalized, "preserve");
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      setError(message);
      throw e;
    } finally {
      setIsLoading(false);
    }
  }, [applyProjectUpdate, project, styleGuide]);

  const replace = useCallback(
    async (items: string[]) => {
      if (!project) {
        throw new Error("No hay ningún proyecto abierto");
      }

      const normalized = normalizeStyleGuideRules(items);
      setIsLoading(true);
      setError(null);
      try {
        if (!isTauri()) {
          return applyProjectUpdate(normalized, true);
        }

        await ipc.styleGuideReplace(normalized);
        const refreshedProject = await ipc.projectGet();
        if (refreshedProject) {
          replaceProject(refreshedProject, { markDirty: false });
          return refreshedProject.styleGuide;
        }
        return applyProjectUpdate(normalized, false);
      } catch (e) {
        const message = e instanceof Error ? e.message : String(e);
        setError(message);
        throw e;
      } finally {
        setIsLoading(false);
      }
    },
    [applyProjectUpdate, project, replaceProject],
  );

  return {
    styleGuide,
    isLoading,
    error,
    refresh,
    replace,
  };
}
