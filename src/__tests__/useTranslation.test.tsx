import { act, renderHook, waitFor } from "@testing-library/react";
import { useTranslation as useTranslationHook } from "@/hooks/useTranslation";
import { useProjectStore } from "@/stores/projectStore";
import type { BatchProgress } from "@/types/domain";

const listeners = new Map<string, (event: { payload: BatchProgress }) => void>();
const translateBatchStartMock = vi.fn();
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
  projectGet: (...args: unknown[]) => projectGetMock(...args),
}));

describe("useTranslation hook", () => {
  beforeEach(() => {
    vi.stubGlobal("__TAURI_INTERNALS__", { transformCallback: vi.fn() });
    listeners.clear();
    translateBatchStartMock.mockReset();
    projectGetMock.mockReset();
    useProjectStore.getState().reset();
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("tracks batch translation lifecycle through events", async () => {
    const initialProgress: BatchProgress = {
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
});
