import { act, renderHook, waitFor } from "@testing-library/react";
import { backend, makeProject } from "@/test/fakeBackend";
import { useGlossary } from "@/hooks/useGlossary";
import { useProject } from "@/hooks/useProject";
import { useStyleGuide } from "@/hooks/useStyleGuide";
import { useProjectStore } from "@/stores/projectStore";

vi.mock("@tauri-apps/api/core", async () => (await import("@/test/fakeBackend")).tauriCoreMock);
vi.mock("@tauri-apps/api/event", async () => (await import("@/test/fakeBackend")).tauriEventMock);

const ACTORS = "data/Actors.json";
const HERO = "Actors:name:1";
const MAGE = "Actors:name:2";

type AnyProject = ReturnType<typeof makeProject> | null | undefined;

function entryOf(project: AnyProject, id: string) {
  return project?.files.flatMap((file) => file.entries).find((entry) => entry.id === id);
}

const storeEntry = (id: string) => entryOf(useProjectStore.getState().project, id);

/** Timestamps are stamped by whichever side wrote last and are never shown, so they are not part of "same project". */
const withoutTimestamps = (project: AnyProject) => ({ ...project, updatedAt: "" });

/** The hooks a real screen combines, sharing one store and one (fake) backend. */
function renderSession() {
  return renderHook(() => ({
    project: useProject(),
    glossary: useGlossary(),
    styleGuide: useStyleGuide(),
  }));
}

async function openSavedProject() {
  backend.disk = makeProject();
  const session = renderSession();
  await act(async () => {
    await session.result.current.project.open("C:/Games/Fake.fairtl");
  });
  return session;
}

/** Starts an async action inside act() without awaiting it, so the test can interleave other actions. */
function start<T>(action: () => Promise<T>): Promise<T> {
  let pending!: Promise<T>;
  act(() => {
    pending = action();
  });
  return pending;
}

async function settle(...pending: Array<Promise<unknown>>) {
  await act(async () => {
    await Promise.allSettled(pending);
  });
}

beforeEach(() => {
  backend.reset();
  useProjectStore.getState().reset();
});

describe("project session against the backend", () => {
  it("creates, edits, saves and reopens a project without diverging from the backend", async () => {
    backend.gameFixture = makeProject();
    const { result } = renderSession();

    await act(async () => {
      await result.current.project.create("C:/Games/Fake", "Fake Game", "ja", "es");
    });
    expect(useProjectStore.getState().isDirty).toBe(true);

    await act(async () => {
      await result.current.project.updateEntry(ACTORS, HERO, "Héroe", "reviewed");
      // A blank translation can never keep a status: the store and the backend must agree on that.
      await result.current.project.updateEntry(ACTORS, MAGE, "   ", "final");
    });
    expect(storeEntry(HERO)).toMatchObject({ translation: "Héroe", status: "reviewed" });
    expect(storeEntry(MAGE)).toMatchObject({ translation: "   ", status: "untranslated" });
    expect(entryOf(backend.project, MAGE)?.status).toBe("untranslated");

    await act(async () => {
      await result.current.project.save();
    });
    expect(useProjectStore.getState().isDirty).toBe(false);
    expect(withoutTimestamps(backend.disk)).toEqual(withoutTimestamps(useProjectStore.getState().project));
    // Saving must not ship the whole project back over IPC: it is tens of MiB on large games.
    expect(backend.callsTo("project_get")).toHaveLength(0);

    useProjectStore.getState().reset();
    await act(async () => {
      await result.current.project.open("C:/Games/Fake.fairtl");
    });
    expect(useProjectStore.getState().project).toEqual(backend.disk);
    expect(useProjectStore.getState().isDirty).toBe(false);
  });

  it("reports loading while open and save are in flight and surfaces backend errors", async () => {
    backend.disk = makeProject();
    const { result } = renderSession();

    const releaseOpen = backend.hold("project_open");
    const opening = start(() => result.current.project.open("C:/Games/Fake.fairtl"));
    await waitFor(() => expect(backend.callsTo("project_open")).toHaveLength(1));
    expect(result.current.project.isLoading).toBe(true);
    releaseOpen();
    await settle(opening);
    expect(result.current.project.isLoading).toBe(false);

    const releaseSave = backend.hold("project_save");
    const saving = start(() => result.current.project.save());
    await waitFor(() => expect(backend.callsTo("project_save")).toHaveLength(1));
    expect(result.current.project.isLoading).toBe(true);
    releaseSave();
    await settle(saving);
    expect(result.current.project.isLoading).toBe(false);

    backend.failNext("project_save", "Disco lleno");
    let failure: unknown;
    await act(async () => {
      await result.current.project.save().catch((error: unknown) => {
        failure = error;
      });
    });
    expect(failure).toBeInstanceOf(Error);
    expect(result.current.project.error).toBe("Disco lleno");
    expect(result.current.project.isLoading).toBe(false);
  });

  it("keeps the project dirty when the user edits while a save is running", async () => {
    const { result } = await openSavedProject();
    await act(async () => {
      await result.current.project.updateEntry(ACTORS, HERO, "Héroe", "draft");
    });

    const releaseSave = backend.hold("project_save");
    const saving = start(() => result.current.project.save());
    await waitFor(() => expect(backend.callsTo("project_save")).toHaveLength(1));
    const editing = start(() => result.current.project.updateEntry(ACTORS, MAGE, "Mago", "draft"));
    releaseSave();
    await settle(saving, editing);

    // The disk copy has the first edit only: claiming "saved" would hide unsaved work.
    expect(entryOf(backend.disk, HERO)?.translation).toBe("Héroe");
    expect(entryOf(backend.disk, MAGE)?.translation).toBe("");
    expect(storeEntry(MAGE)?.translation).toBe("Mago");
    expect(useProjectStore.getState().isDirty).toBe(true);

    await act(async () => {
      await result.current.project.save();
    });
    expect(useProjectStore.getState().isDirty).toBe(false);
    expect(withoutTimestamps(backend.disk)).toEqual(withoutTimestamps(useProjectStore.getState().project));
  });

  it("mirrors the backend when an edit is rejected, without dropping newer accepted edits", async () => {
    const { result } = await openSavedProject();
    backend.failNext("project_update_entry", "Entrada bloqueada");

    const rejected = start(() =>
      result.current.project.updateEntry(ACTORS, HERO, "Héroe", "draft").catch(() => undefined),
    );
    const accepted = start(() => result.current.project.updateEntry(ACTORS, MAGE, "Mago", "draft"));
    await settle(rejected, accepted);

    expect(storeEntry(HERO)?.translation).toBe("");
    expect(storeEntry(MAGE)?.translation).toBe("Mago");
    expect(entryOf(backend.project, MAGE)?.translation).toBe("Mago");
    expect(result.current.project.error).toBe("Entrada bloqueada");
    expect(useProjectStore.getState().isDirty).toBe(true);
  });

  it("returns to clean when the only edit since the last save is rejected", async () => {
    const { result } = await openSavedProject();
    backend.failNext("project_update_entry", "Entrada bloqueada");

    await act(async () => {
      await result.current.project.updateEntry(ACTORS, HERO, "Héroe", "draft").catch(() => undefined);
    });

    expect(storeEntry(HERO)?.translation).toBe("");
    expect(useProjectStore.getState().isDirty).toBe(false);
  });

  it("applies a rejected batch of edits atomically on both sides", async () => {
    const { result } = await openSavedProject();

    await act(async () => {
      await result.current.project
        .updateEntriesBatch([
          { filePath: ACTORS, entryId: HERO, translation: "Héroe", status: "draft" },
          { filePath: ACTORS, entryId: "Actors:name:99", translation: "??", status: "draft" },
        ])
        .catch(() => undefined);
    });

    expect(result.current.project.error).toContain("Entrada no encontrada");
    expect(entryOf(backend.project, HERO)?.translation).toBe("");
    expect(storeEntry(HERO)?.translation).toBe("");
    expect(useProjectStore.getState().isDirty).toBe(false);
  });

  it("exports only after pending edits reached the backend", async () => {
    const { result } = await openSavedProject();

    const releaseEdit = backend.hold("project_update_entry");
    const editing = start(() => result.current.project.updateEntry(ACTORS, HERO, "Héroe", "draft"));
    await waitFor(() => expect(backend.callsTo("project_update_entry")).toHaveLength(1));

    let exported: Awaited<ReturnType<typeof result.current.project.exportProject>> | undefined;
    const exporting = start(async () => {
      exported = await result.current.project.exportProject("C:/Out");
    });
    await new Promise((resolve) => setTimeout(resolve, 10));
    expect(backend.callsTo("export_project")).toHaveLength(0);

    releaseEdit();
    await settle(editing, exporting);
    expect(backend.callsTo("export_project")).toHaveLength(1);
    expect(exported?.entriesApplied).toBe(1);
  });
});

describe("glossary and style guide against the backend", () => {
  it("rejects duplicate terms with the backend message and leaves the project untouched", async () => {
    const { result } = await openSavedProject();

    await act(async () => {
      await result.current.glossary.add("Hero", "Héroe", true);
    });
    expect(result.current.glossary.glossary).toEqual([
      { source: "Hero", target: "Héroe", caseSensitive: true },
    ]);
    expect(useProjectStore.getState().isDirty).toBe(true);

    await act(async () => {
      await result.current.glossary.add("Hero", "Otro", false).catch(() => undefined);
    });
    expect(result.current.glossary.error).toContain("ya existe");
    expect(useProjectStore.getState().project?.glossary).toHaveLength(1);

    await act(async () => {
      await result.current.glossary.update("Hero", "Campeón", false);
      await result.current.glossary.remove("Hero");
    });
    expect(useProjectStore.getState().project?.glossary).toEqual([]);
  });

  it("keeps an entry edited while a glossary change was in flight", async () => {
    const { result } = await openSavedProject();

    const releaseGlossary = backend.hold("glossary_add");
    const adding = start(() => result.current.glossary.add("Hero", "Héroe", true));
    await waitFor(() => expect(backend.callsTo("glossary_add")).toHaveLength(1));
    const editing = start(() => result.current.project.updateEntry(ACTORS, HERO, "Héroe", "draft"));
    releaseGlossary();
    await settle(adding, editing);

    expect(storeEntry(HERO)?.translation).toBe("Héroe");
    expect(entryOf(backend.project, HERO)?.translation).toBe("Héroe");
    expect(useProjectStore.getState().project?.glossary).toHaveLength(1);
  });

  it("ignores a glossary reply that arrives after another project was opened", async () => {
    const { result } = await openSavedProject();

    const releaseGlossary = backend.hold("glossary_add");
    const adding = start(() => result.current.glossary.add("Hero", "Héroe", true));
    await waitFor(() => expect(backend.callsTo("glossary_add")).toHaveLength(1));
    act(() => {
      useProjectStore.getState().setProject(makeProject({ id: "other-project" }), { markDirty: false });
    });
    releaseGlossary();
    await settle(adding);

    expect(useProjectStore.getState().project?.id).toBe("other-project");
    expect(useProjectStore.getState().project?.glossary).toEqual([]);
    expect(useProjectStore.getState().isDirty).toBe(false);
  });

  it("persists only the style guide and keeps unsaved entry edits dirty", async () => {
    const { result } = await openSavedProject();
    await act(async () => {
      await result.current.project.updateEntry(ACTORS, HERO, "Héroe", "draft");
    });

    await act(async () => {
      await result.current.styleGuide.replace(["  Tutea al jugador  ", "", "Tutea al jugador", "Frases cortas"]);
    });
    // Only the guide reached the disk; the earlier entry edit is still pending.
    expect(backend.disk?.styleGuide).toEqual(["Tutea al jugador", "Frases cortas"]);
    expect(entryOf(backend.disk, HERO)?.translation).toBe("");
    expect(useProjectStore.getState().project?.styleGuide).toEqual(["Tutea al jugador", "Frases cortas"]);
    expect(useProjectStore.getState().isDirty).toBe(true);

    await act(async () => {
      await result.current.project.save();
    });
    expect(useProjectStore.getState().isDirty).toBe(false);

    const releaseGuide = backend.hold("style_guide_replace");
    const replacing = start(() => result.current.styleGuide.replace(["Frases cortas"]));
    await waitFor(() => expect(backend.callsTo("style_guide_replace")).toHaveLength(2));
    const editing = start(() => result.current.project.updateEntry(ACTORS, MAGE, "Mago", "draft"));
    releaseGuide();
    await settle(replacing, editing);

    expect(storeEntry(MAGE)?.translation).toBe("Mago");
    expect(entryOf(backend.disk, MAGE)?.translation).toBe("");
    expect(useProjectStore.getState().project?.styleGuide).toEqual(["Frases cortas"]);
    expect(useProjectStore.getState().isDirty).toBe(true);
  });
});
