import { act, renderHook, waitFor } from "@testing-library/react";
import { useTranslation as useTranslationHook } from "@/hooks/useTranslation";
import { useProjectStore } from "@/stores/projectStore";
import type { BatchProgress } from "@/types/domain";

const listeners = new Map<string, (event: { payload: BatchProgress }) => void>();
const translateBatchStartMock = vi.fn();
const translateBatchCancelMock = vi.fn();
const projectGetMock = vi.fn();

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (eventName: string, handler: (event: { payload: BatchProgress }) => void) => {
    listeners.set(eventName, handler);
    return () => {
      listeners.delete(eventName);
    };
  }),
}));

vi.mock("@/lib/ipc", () => ({
  translateBatchStart: (...args: unknown[]) => translateBatchStartMock(...args),
  translateBatchCancel: (...args: unknown[]) => translateBatchCancelMock(...args),
  projectGet: (...args: unknown[]) => projectGetMock(...args),
}));

describe("useTranslation hook", () => {
  beforeEach(() => {
    vi.stubGlobal("__TAURI_INTERNALS__", { transformCallback: vi.fn() });
    listeners.clear();
    translateBatchStartMock.mockReset();
    translateBatchCancelMock.mockReset();
    projectGetMock.mockReset();
    useProjectStore.getState().reset();
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("tracks batch translation lifecycle through events", async () => {
    const initialProgress: BatchProgress = {
      runId: "41",
      cancelled: false,
      filesProcessed: 0,
      totalFiles: 2,
      chunksProcessed: 0,
      totalChunks: 3,
      entriesTranslated: 0,
      totalEntries: 10,
      currentFile: "",
      currentFileChunksProcessed: 0,
      currentFileTotalChunks: 0,
      failedEntries: 0,
      failedBatches: 0,
      warnings: [],
    };
    const progressPayload: BatchProgress = {
      runId: "41",
      cancelled: false,
      filesProcessed: 1,
      totalFiles: 2,
      chunksProcessed: 2,
      totalChunks: 3,
      entriesTranslated: 4,
      totalEntries: 10,
      currentFile: "Data/Map001.json",
      currentFileChunksProcessed: 2,
      currentFileTotalChunks: 2,
      failedEntries: 0,
      failedBatches: 0,
      warnings: [],
    };
    const completePayload: BatchProgress = {
      runId: "41",
      cancelled: false,
      filesProcessed: 2,
      totalFiles: 2,
      chunksProcessed: 3,
      totalChunks: 3,
      entriesTranslated: 10,
      totalEntries: 10,
      currentFile: "",
      currentFileChunksProcessed: 0,
      currentFileTotalChunks: 0,
      failedEntries: 0,
      failedBatches: 0,
      warnings: [],
    };

    translateBatchStartMock.mockResolvedValue(initialProgress);
    projectGetMock.mockResolvedValue(null);

    const { result } = renderHook(() => useTranslationHook());

    await waitFor(() => {
      expect(listeners.has("translation:progress")).toBe(true);
      expect(listeners.has("translation:complete")).toBe(true);
    });

    await act(async () => {
      await result.current.startBatch("openai", []);
    });
    expect(result.current.isTranslating).toBe(true);
    expect(result.current.progress).toEqual(initialProgress);

    await act(async () => {
      await result.current.cancelBatch();
    });
    expect(translateBatchCancelMock).toHaveBeenCalledWith("41");

    await act(async () => {
      listeners.get("translation:progress")?.({ payload: progressPayload });
    });
    expect(result.current.progress).toEqual(progressPayload);

    await act(async () => {
      listeners.get("translation:complete")?.({ payload: completePayload });
    });

    await waitFor(() => {
      expect(result.current.isTranslating).toBe(false);
      expect(result.current.progress).toEqual(completePayload);
    });
  });

  it("preserves a cancelled terminal result", async () => {
    const initialProgress: BatchProgress = {
      runId: "9",
      cancelled: false,
      filesProcessed: 0,
      totalFiles: 1,
      chunksProcessed: 0,
      totalChunks: 3,
      entriesTranslated: 0,
      totalEntries: 3,
      currentFile: "",
      currentFileChunksProcessed: 0,
      currentFileTotalChunks: 0,
      failedEntries: 0,
      failedBatches: 0,
      warnings: [],
    };
    translateBatchStartMock.mockResolvedValue(initialProgress);
    projectGetMock.mockResolvedValue(null);
    const { result } = renderHook(() => useTranslationHook());

    await waitFor(() => expect(listeners.has("translation:complete")).toBe(true));
    await act(async () => {
      await result.current.startBatch("openai", []);
      listeners.get("translation:complete")?.({
        payload: {
          ...initialProgress,
          cancelled: true,
          chunksProcessed: 1,
          entriesTranslated: 1,
        },
      });
    });

    expect(result.current.isTranslating).toBe(false);
    expect(result.current.progress?.cancelled).toBe(true);
    expect(result.current.progress?.chunksProcessed).toBe(1);
  });

  it("does not overwrite an early progress event when start resolves", async () => {
    const initialProgress: BatchProgress = {
      runId: "27",
      cancelled: false,
      filesProcessed: 0,
      totalFiles: 1,
      chunksProcessed: 0,
      totalChunks: 3,
      entriesTranslated: 0,
      totalEntries: 3,
      currentFile: "",
      currentFileChunksProcessed: 0,
      currentFileTotalChunks: 0,
      failedEntries: 0,
      failedBatches: 0,
      warnings: [],
    };
    const earlyProgress: BatchProgress = {
      ...initialProgress,
      chunksProcessed: 1,
      entriesTranslated: 1,
      currentFile: "Data/Map001.json",
      currentFileChunksProcessed: 1,
      currentFileTotalChunks: 3,
    };
    let resolveStart!: (progress: BatchProgress) => void;
    translateBatchStartMock.mockReturnValue(
      new Promise<BatchProgress>((resolve) => {
        resolveStart = resolve;
      }),
    );

    const { result } = renderHook(() => useTranslationHook());
    await waitFor(() => expect(listeners.has("translation:progress")).toBe(true));

    let startPromise!: Promise<BatchProgress>;
    act(() => {
      startPromise = result.current.startBatch("openai", []);
    });
    await waitFor(() => expect(translateBatchStartMock).toHaveBeenCalledTimes(1));

    act(() => {
      listeners.get("translation:progress")?.({ payload: earlyProgress });
    });
    await act(async () => {
      resolveStart(initialProgress);
      await startPromise;
    });

    expect(result.current.progress).toEqual(earlyProgress);
  });
});
