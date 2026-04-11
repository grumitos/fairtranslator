import { useCallback, useEffect, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useProjectStore } from "@/stores/projectStore";
import * as ipc from "@/lib/ipc";
import type { BatchProgress, ProviderType } from "@/types/domain";

/**
 * Hook for batch translation operations.
 * Manages the translation lifecycle: start, progress tracking, completion.
 * Listens to Tauri events for real-time progress updates.
 */
export function useTranslation() {
  const [isTranslating, setIsTranslating] = useState(false);
  const [progress, setProgress] = useState<BatchProgress | null>(null);
  const [error, setError] = useState<string | null>(null);
  const replaceProject = useProjectStore((s) => s.replaceProject);

  // Listen for progress and completion events
  useEffect(() => {
    const unlisteners: UnlistenFn[] = [];

    const setup = async () => {
      const unProgress = await listen<BatchProgress>(
        "translation:progress",
        (event) => {
          setProgress(event.payload);
        },
      );
      unlisteners.push(unProgress);

      const unComplete = await listen<BatchProgress>(
        "translation:complete",
        (event) => {
          setProgress(event.payload);
          setIsTranslating(false);

          // Refresh project state from backend to get updated translations
          void ipc.projectGet()
            .then((proj) => {
              if (proj) {
                replaceProject(proj, { markDirty: true });
              }
            })
            .catch((refreshError) => {
              const msg =
                refreshError instanceof Error
                  ? refreshError.message
                  : String(refreshError);
              setError(msg);
            });
        },
      );
      unlisteners.push(unComplete);
    };

    setup();

    return () => {
      unlisteners.forEach((u) => u());
    };
  }, [replaceProject]);

  const startBatch = useCallback(
    async (provider: ProviderType, filePaths: string[] = []) => {
      setIsTranslating(true);
      setProgress(null);
      setError(null);
      try {
        const result = await ipc.translateBatchStart(provider, filePaths);
        return result;
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        setError(msg);
        setIsTranslating(false);
        throw e;
      }
    },
    [],
  );

  const reset = useCallback(() => {
    setProgress(null);
    setError(null);
    setIsTranslating(false);
  }, []);

  return {
    isTranslating,
    progress,
    error,
    startBatch,
    reset,
  };
}
