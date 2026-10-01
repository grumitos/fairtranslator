import { StrictMode } from "react";
import { act, renderHook, waitFor } from "@testing-library/react";
import { backend, makeProject } from "@/test/fakeBackend";
import { useProject } from "@/hooks/useProject";
import { useTranslation } from "@/hooks/useTranslation";
import { useProjectStore } from "@/stores/projectStore";

vi.mock("@tauri-apps/api/core", async () => (await import("@/test/fakeBackend")).tauriCoreMock);
vi.mock("@tauri-apps/api/event", async () => (await import("@/test/fakeBackend")).tauriEventMock);

const ACTORS = "data/Actors.json";
const HERO = "Actors:name:1";
const MAGE = "Actors:name:3";
const POTION = "Items:name:1";

function storeEntry(id: string) {
  return useProjectStore
    .getState()
    .project?.files.flatMap((file) => file.entries)
    .find((entry) => entry.id === id);
}

async function openSession(options: { translated?: boolean } = {}) {
  const project = makeProject();
  if (options.translated) {
    for (const file of project.files) {
      for (const entry of file.entries) {
        entry.translation = "ya traducido";
        entry.status = "reviewed";
      }
    }
  }
  backend.disk = project;
  const session = renderHook(() => ({ project: useProject(), translation: useTranslation() }));
  await waitFor(() => expect(backend.listenerCount("translation:complete")).toBe(1));
  await act(async () => {
    await session.result.current.project.open("C:/Games/Fake.fairtl");
  });
  return session;
}

beforeEach(() => {
  backend.reset();
  useProjectStore.getState().reset();
  vi.stubGlobal("__TAURI_INTERNALS__", { transformCallback: vi.fn() });
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("batch translation against the backend", () => {
  it("shows batch results and keeps edits typed while the run was in flight", async () => {
    const { result } = await openSession();

    await act(async () => {
      await result.current.translation.startBatch("openai", []);
    });
    expect(result.current.translation.isTranslating).toBe(true);
    expect(result.current.translation.progress).toMatchObject({ runId: "1", totalEntries: 5 });

    act(() => backend.reportProgress({ chunksProcessed: 1, entriesTranslated: 2 }));
    expect(result.current.translation.progress?.entriesTranslated).toBe(2);
    // Partial results are only visible after the terminal event: the grid still shows the old text.
    expect(storeEntry(HERO)?.translation).toBe("");

    await act(async () => {
      await result.current.project.updateEntry(ACTORS, MAGE, "Mago", "reviewed");
    });

    act(() => {
      backend.finishRun({ translations: { [HERO]: "Héroe", [POTION]: "Poción" } });
    });
    await waitFor(() => expect(result.current.translation.isTranslating).toBe(false));
    await waitFor(() => expect(storeEntry(HERO)?.translation).toBe("Héroe"));

    expect(result.current.translation.progress).toMatchObject({ entriesTranslated: 2, cancelled: false });
    expect(storeEntry(POTION)).toMatchObject({ translation: "Poción", status: "draft" });
    expect(storeEntry(MAGE)).toMatchObject({ translation: "Mago", status: "reviewed" });
    expect(useProjectStore.getState().isDirty).toBe(true);
    expect(backend.callsTo("project_get")).toHaveLength(1);
  });

  it("keeps the partial results of a cancelled run and refuses to cancel twice", async () => {
    const { result } = await openSession();
    await act(async () => {
      await result.current.translation.startBatch("openai", []);
    });

    let accepted = false;
    await act(async () => {
      accepted = await result.current.translation.cancelBatch();
    });
    expect(accepted).toBe(true);

    act(() => {
      backend.finishRun({ cancelled: true, translations: { [HERO]: "Héroe" } });
    });
    await waitFor(() => expect(storeEntry(HERO)?.translation).toBe("Héroe"));

    expect(result.current.translation.isTranslating).toBe(false);
    expect(result.current.translation.progress).toMatchObject({ cancelled: true, entriesTranslated: 1 });
    expect(useProjectStore.getState().isDirty).toBe(true);

    await act(async () => {
      accepted = await result.current.translation.cancelBatch();
    });
    expect(accepted).toBe(false);
  });

  it("does not dirty a clean project when a run translates nothing", async () => {
    const { result } = await openSession();
    await act(async () => {
      await result.current.translation.startBatch("openai", []);
    });

    act(() => {
      backend.finishRun({ failedEntries: 5 });
    });
    await waitFor(() => expect(result.current.translation.isTranslating).toBe(false));

    expect(result.current.translation.progress).toMatchObject({ failedEntries: 5, entriesTranslated: 0 });
    expect(useProjectStore.getState().isDirty).toBe(false);
  });

  it("ends in a terminal state when the completion event arrives before start returns", async () => {
    const { result } = await openSession({ translated: true });

    await act(async () => {
      await result.current.translation.startBatch("openai", []);
    });

    expect(result.current.translation.isTranslating).toBe(false);
    expect(result.current.translation.progress).toMatchObject({ totalEntries: 0, cancelled: false });
    expect(useProjectStore.getState().isDirty).toBe(false);
  });

  it("reports a start rejection and lets the user retry", async () => {
    const { result } = await openSession();
    backend.failNext("translate_batch_start", "No API key configured for this provider");

    let failure: unknown;
    await act(async () => {
      await result.current.translation.startBatch("openai", []).catch((error: unknown) => {
        failure = error;
      });
    });
    expect(failure).toBeInstanceOf(Error);
    expect(result.current.translation.isTranslating).toBe(false);
    expect(result.current.translation.error).toBe("No API key configured for this provider");

    await act(async () => {
      await result.current.translation.startBatch("openai", []);
    });
    expect(result.current.translation.isTranslating).toBe(true);
    expect(result.current.translation.error).toBeNull();
  });
});

describe("batch event subscriptions", () => {
  it("registers each listener once under StrictMode", async () => {
    renderHook(() => useTranslation(), { wrapper: StrictMode });
    await waitFor(() => expect(backend.listenerCount("translation:complete")).toBeGreaterThan(0));
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 20));
    });

    expect(backend.listenerCount("translation:progress")).toBe(1);
    expect(backend.listenerCount("translation:complete")).toBe(1);
  });

  it("releases listeners that resolve after the hook unmounted", async () => {
    const { unmount } = renderHook(() => useTranslation());
    unmount();
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 20));
    });

    expect(backend.listenerCount("translation:progress")).toBe(0);
    expect(backend.listenerCount("translation:complete")).toBe(0);
  });
});
