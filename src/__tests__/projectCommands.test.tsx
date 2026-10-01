import { act, renderHook } from "@testing-library/react";
import i18n from "@/lib/i18n";
import { backend, makeProject } from "@/test/fakeBackend";
import { useProjectCommands } from "@/components/layout/useProjectCommands";
import { useProjectLocationStore } from "@/lib/projectLocation";
import { useProjectStore } from "@/stores/projectStore";
import { useSettingsStore } from "@/stores/settingsStore";

const dialogs = vi.hoisted(() => ({ open: vi.fn(), save: vi.fn() }));

vi.mock("@tauri-apps/api/core", async () => (await import("@/test/fakeBackend")).tauriCoreMock);
vi.mock("@tauri-apps/api/event", async () => (await import("@/test/fakeBackend")).tauriEventMock);
vi.mock("@tauri-apps/plugin-dialog", () => dialogs);

beforeEach(async () => {
  await i18n.changeLanguage("es");
  backend.reset();
  useProjectStore.getState().reset();
  useProjectLocationStore.setState({ location: null });
  useSettingsStore.setState({ recentProjects: [] });
  dialogs.open.mockReset();
  dialogs.save.mockReset();
});

describe("project commands: save as and recent projects", () => {
  it("opens a project, then saves it elsewhere and keeps the recent list on the final path", async () => {
    backend.disk = makeProject({ name: "Bruma" });
    dialogs.open.mockResolvedValue("D:/Proyectos/bruma.fairtl");
    dialogs.save.mockResolvedValue("E:/Copias/bruma.fairtl");
    const { result } = renderHook(() => useProjectCommands());

    await act(async () => {
      await result.current.openProject();
    });
    expect(useSettingsStore.getState().recentProjects.map((recent) => recent.path)).toEqual(["D:/Proyectos/bruma.fairtl"]);

    await act(async () => {
      await result.current.saveProjectAs();
    });
    expect(dialogs.save).toHaveBeenCalledWith(expect.objectContaining({ defaultPath: "D:/Proyectos/bruma.fairtl" }));
    expect(backend.callsTo("project_save")[0].args).toEqual({ path: "E:/Copias/bruma.fairtl" });
    expect(useProjectLocationStore.getState().location).toEqual({ path: "E:/Copias/bruma.fairtl", saved: true });
    expect(useSettingsStore.getState().recentProjects.map((recent) => recent.path)).toEqual([
      "E:/Copias/bruma.fairtl",
      "D:/Proyectos/bruma.fairtl",
    ]);
  });

  it("asks where to save when the backend has no save path", async () => {
    backend.gameFixture = makeProject();
    await act(async () => {
      await backend.invoke("project_create", { gameDir: "C:/Games/Fake", name: "Fake", sourceLang: "ja", targetLang: "es" });
    });
    useProjectStore.getState().setProject(structuredClone(backend.project!), { markDirty: true });
    backend.savePath = null; // sin carpeta Documentos
    dialogs.save.mockResolvedValue(null);
    const { result } = renderHook(() => useProjectCommands());

    await act(async () => {
      await result.current.saveProject();
    });

    expect(dialogs.save).toHaveBeenCalledOnce();
    expect(backend.callsTo("project_save")).toHaveLength(0);
  });
});
