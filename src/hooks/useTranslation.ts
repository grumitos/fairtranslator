import { useCallback, useEffect, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import * as ipc from "@/lib/ipc";
import { refreshProjectFromBackend } from "@/lib/projectSync";
import type { BatchProgress, ProviderType } from "@/types/domain";

function canListenToTauriEvents(): boolean {
  const tauriInternals = (globalThis as {
    __TAURI_INTERNALS__?: { transformCallback?: unknown };
  }).__TAURI_INTERNALS__;

  return typeof tauriInternals?.transformCallback === "function";
}

function hasAdvancedBeyondInitial(
  current: BatchProgress,
  initial: BatchProgress,
): boolean {
  return (
    current.cancelled ||
    current.filesProcessed > initial.filesProcessed ||
    current.chunksProcessed > initial.chunksProcessed ||
    current.entriesTranslated > initial.entriesTranslated ||
    current.failedEntries > initial.failedEntries ||
    current.failedBatches > initial.failedBatches
  );
}

/**
 * Hook for batch translation operations.
 * Manages the translation lifecycle: start, progress tracking, completion.
 * Listens to Tauri events for real-time progress updates.
 */
export function useTranslation() {
  const [isTranslating, setIsTranslating] = useState(false);
  const [progress, setProgress] = useState<BatchProgress | null>(null);
  const [error, setError] = useState<string | null>(null);

  // Listen for progress and completion events
  useEffect(() => {
    if (!canListenToTauriEvents()) {
      return;
    }

    // listen() resolves asynchronously: a listener that resolves after cleanup (StrictMode's
    // double mount, fast unmount) must be released immediately or it would live forever.
    let disposed = false;
    const unlisteners: UnlistenFn[] = [];

    const register = async (
      eventName: string,
      handler: (payload: BatchProgress) => void,
    ) => {
      const unlisten = await listen<BatchProgress>(eventName, (event) => handler(event.payload));
      if (disposed) {
        unlisten();
      } else {
        unlisteners.push(unlisten);
      }
    };

    const setup = async () => {
      await register("translation:progress", (payload) => {
        setProgress(payload);
      });
      if (disposed) {
        return;
      }
      await register("translation:complete", (payload) => {
        setProgress(payload);
        setIsTranslating(false);

        // Refresh project state from backend to get updated translations. A run that translated
        // nothing left the project as it was, so it must not turn a clean project dirty.
        void refreshProjectFromBackend(payload.entriesTranslated > 0 ? true : "preserve")
          .catch((refreshError) => {
            const msg =
              refreshError instanceof Error
                ? refreshError.message
                : String(refreshError);
            setError(msg);
          });
      });
    };

    void setup().catch((listenError) => {
      const msg =
        listenError instanceof Error
          ? listenError.message
          : String(listenError);
      setError(msg);
    });

    return () => {
      disposed = true;
      unlisteners.forEach((u) => u());
    };
  }, []);

  const startBatch = useCallback(
    async (provider: ProviderType, filePaths: string[] = [], model?: string) => {
      setIsTranslating(true);
      setProgress(null);
      setError(null);
      try {
        const result = await ipc.translateBatchStart(provider, filePaths, model);
        setProgress((current) =>
          current?.runId === result.runId && hasAdvancedBeyondInitial(current, result)
            ? current
            : result,
        );
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

  const cancelBatch = useCallback(async () => {
    const runId = progress?.runId;
    if (!runId) {
      return false;
    }
    return ipc.translateBatchCancel(runId);
  }, [progress?.runId]);

  return {
    isTranslating,
    progress,
    error,
    startBatch,
    cancelBatch,
  };
}
