import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import i18n from "@/lib/i18n";
import { Toolbar } from "@/components/layout/Toolbar";
import { TooltipProvider } from "@/components/ui/tooltip";
import { useEditorStore } from "@/stores/editorStore";
import type { Project } from "@/types/domain";

const mocks = vi.hoisted(() => ({
  project: null as Project | null,
  dialogOpen: vi.fn(),
  exportProject: vi.fn(),
  save: vi.fn(),
  openProject: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: mocks.dialogOpen }));
vi.mock("@/hooks/useProject", () => ({
  useProject: () => ({
    project: mocks.project,
    save: mocks.save,
    isDirty: false,
    isLoading: false,
    open: mocks.openProject,
    exportProject: mocks.exportProject,
  }),
}));

vi.mock("@/components/layout/NewProjectDialog", () => ({ NewProjectDialog: () => null }));
vi.mock("@/components/layout/SettingsPanel", () => ({
  SettingsPanel: ({ open }: { open: boolean }) => open
    ? <div role="dialog" aria-label="Settings panel mock" />
    : null,
}));
vi.mock("@/components/layout/GlossaryPanel", () => ({ GlossaryPanel: () => null }));
vi.mock("@/components/layout/StyleGuidePanel", () => ({ StyleGuidePanel: () => null }));
vi.mock("@/components/layout/BatchTranslatePanel", () => ({
  BatchTranslatePanel: ({
    open,
    onConfigureProvider,
  }: {
    open: boolean;
    onConfigureProvider?: () => void;
  }) => open
    ? <button type="button" onClick={onConfigureProvider}>Configure provider mock</button>
    : null,
}));

function makeProject(statuses: Array<"untranslated" | "draft" | "reviewed" | "final">): Project {
  return {
    id: "toolbar-project",
    name: "Toolbar Project",
    sourceLang: "en",
    targetLang: "es",
    engine: "mv",
    gameDir: "C:/Games/Toolbar",
    files: [{
      relativePath: "data/Map001.json",
      entries: [
        ...statuses.map((status, index) => ({
          id: `entry-${index}`,
          fileContext: "Map001",
          original: `Text ${index}`,
          translation: status === "untranslated" ? "" : `Texto ${index}`,
          status,
          notes: "",
        })),
        {
          id: "blank-entry",
          fileContext: "Map001",
          original: "   ",
          translation: "",
          status: "untranslated",
          notes: "",
        },
      ],
    }],
    glossary: [],
    styleGuide: [],
    createdAt: "2026-01-01T00:00:00.000Z",
    updatedAt: "2026-01-01T00:00:00.000Z",
  };
}

function renderToolbar() {
  return render(
    <TooltipProvider>
      <Toolbar />
    </TooltipProvider>,
  );
}

describe("Toolbar export readiness", () => {
  beforeEach(async () => {
    await i18n.changeLanguage("es");
    useEditorStore.getState().reset();
    mocks.project = makeProject(["untranslated", "draft", "reviewed", "final"]);
    mocks.dialogOpen.mockReset();
    mocks.dialogOpen.mockResolvedValue("C:/Exports/Toolbar");
    mocks.exportProject.mockReset();
    mocks.exportProject.mockResolvedValue({
      filesExported: 3,
      entriesApplied: 3,
      totalEntries: 4,
      warnings: ["Advertencia uno", "Advertencia dos", "Advertencia tres"],
    });
  });

  afterEach(() => {
    cleanup();
    useEditorStore.getState().reset();
  });

  it("cancela el preflight sin abrir carpeta ni exportar", async () => {
    renderToolbar();

    fireEvent.click(screen.getByRole("button", { name: "Exportar" }));

    expect(await screen.findByRole("heading", { name: "Revisar antes de exportar" })).toBeInTheDocument();
    expect(screen.getByText("Sin traducir: 1")).toBeInTheDocument();
    expect(screen.getByText("Borradores: 1")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Cancelar" }));

    expect(mocks.dialogOpen).not.toHaveBeenCalled();
    expect(mocks.exportProject).not.toHaveBeenCalled();
  });

  it("filtra borradores desde el preflight sin exportar", async () => {
    renderToolbar();

    fireEvent.click(screen.getByRole("button", { name: "Exportar" }));
    fireEvent.click(await screen.findByRole("button", { name: "Revisar borradores" }));

    expect(useEditorStore.getState().statusFilter).toBe("draft");
    expect(mocks.dialogOpen).not.toHaveBeenCalled();
    expect(mocks.exportProject).not.toHaveBeenCalled();
  });

  it("exporta bajo confirmación y conserva el informe completo hasta cerrarlo", async () => {
    renderToolbar();

    fireEvent.click(screen.getByRole("button", { name: "Exportar" }));
    fireEvent.click(await screen.findByRole("button", { name: "Exportar de todos modos" }));

    await waitFor(() => {
      expect(mocks.dialogOpen).toHaveBeenCalledOnce();
      expect(mocks.exportProject).toHaveBeenCalledWith("C:/Exports/Toolbar");
    });
    expect(await screen.findByRole("heading", { name: "Exportación completada con advertencias" })).toBeInTheDocument();
    expect(screen.getByText("C:/Exports/Toolbar")).toBeInTheDocument();
    expect(screen.getByText("Archivos exportados: 3. Traducciones aplicadas: 3/4.")).toBeInTheDocument();
    expect(screen.getByText("Advertencia uno")).toBeInTheDocument();
    expect(screen.getByText("Advertencia dos")).toBeInTheDocument();
    expect(screen.getByText("Advertencia tres")).toBeInTheDocument();
  });

  it("omite el preflight cuando todas las entradas traducibles están revisadas o finales", async () => {
    mocks.project = makeProject(["reviewed", "final"]);
    mocks.exportProject.mockResolvedValue({
      filesExported: 1,
      entriesApplied: 2,
      totalEntries: 2,
      warnings: [],
    });
    renderToolbar();

    fireEvent.click(screen.getByRole("button", { name: "Exportar" }));

    await waitFor(() => expect(mocks.dialogOpen).toHaveBeenCalledOnce());
    expect(screen.queryByRole("heading", { name: "Revisar antes de exportar" })).not.toBeInTheDocument();
    expect(mocks.exportProject).toHaveBeenCalledWith("C:/Exports/Toolbar");
    expect(await screen.findByRole("heading", { name: "Exportación completada" })).toBeInTheDocument();
  });

  it("closes batch translation and opens Settings from provider configuration", () => {
    renderToolbar();

    fireEvent.click(screen.getByRole("button", { name: "Traducción por lotes" }));
    fireEvent.click(screen.getByRole("button", { name: "Configure provider mock" }));

    expect(screen.queryByRole("button", { name: "Configure provider mock" })).not.toBeInTheDocument();
    expect(screen.getByRole("dialog", { name: "Settings panel mock" })).toBeInTheDocument();
  });
});
