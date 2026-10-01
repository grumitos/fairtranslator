import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import i18n from "@/lib/i18n";
import { Toolbar } from "@/components/layout/Toolbar";
import { usePanelStore } from "@/components/layout/panelStore";
import { TooltipProvider } from "@/components/ui/tooltip";
import { useEditorStore } from "@/stores/editorStore";
import { useProjectStore } from "@/stores/projectStore";
import type { Project, ProviderType, TextFitReport } from "@/types/domain";

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
vi.mock("@/components/layout/ContextPanel", () => ({ ContextPanel: () => null }));
vi.mock("@/components/layout/SettingsPanel", () => ({
  SettingsPanel: ({
    open,
    initialProvider,
    onOpenChange,
  }: {
    open: boolean;
    initialProvider?: ProviderType;
    onOpenChange: (open: boolean) => void;
  }) => open
    ? (
        <div role="dialog" aria-label="Settings panel mock">
          <span>{initialProvider ?? "opencode"}</span>
          <button type="button" onClick={() => onOpenChange(false)}>Close settings mock</button>
        </div>
      )
    : null,
}));
vi.mock("@/components/layout/BatchTranslatePanel", () => ({
  BatchTranslatePanel: ({
    open,
    onConfigureProvider,
  }: {
    open: boolean;
    onConfigureProvider?: (provider: ProviderType) => void;
  }) => open
    ? (
        <button type="button" onClick={() => onConfigureProvider?.("google")}>
          Configure provider mock
        </button>
      )
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

function textFit(overrides: Partial<TextFitReport> = {}): TextFitReport {
  return {
    wrapPlugin: null,
    calibration: [],
    wrappedEntries: 0,
    paginatedEntries: 0,
    pagesAdded: 0,
    overflowCount: 0,
    overflow: [],
    ...overrides,
  };
}

function renderToolbar() {
  return render(
    <TooltipProvider>
      <Toolbar />
    </TooltipProvider>,
  );
}

async function openExportPanel() {
  fireEvent.click(screen.getByRole("button", { name: "Exportar" }));
  return screen.findByRole("dialog", { name: "Exportar" });
}

// Cantidad mostrada en la fila de un estado del resumen de exportación.
function statusCount(panel: HTMLElement, label: string) {
  const row = within(panel).getByText(label).closest("div");
  return row?.querySelector("dd")?.textContent;
}

describe("Title bar controller and export panel", () => {
  beforeEach(async () => {
    await i18n.changeLanguage("es");
    useEditorStore.getState().reset();
    usePanelStore.getState().close();
    mocks.project = makeProject(["untranslated", "draft", "reviewed", "final"]);
    mocks.dialogOpen.mockReset();
    mocks.dialogOpen.mockResolvedValue("C:/Exports/Toolbar");
    mocks.exportProject.mockReset();
    mocks.exportProject.mockResolvedValue({
      filesExported: 3,
      entriesApplied: 3,
      totalEntries: 4,
      warnings: ["Advertencia uno", "Advertencia dos", "Advertencia tres"],
      textFit: textFit(),
    });
  });

  afterEach(() => {
    cleanup();
    useEditorStore.getState().reset();
    useProjectStore.getState().reset();
    usePanelStore.getState().close();
  });

  it("shows the translatable entries per status and closes without exporting", async () => {
    renderToolbar();

    const panel = await openExportPanel();
    // La entrada en blanco no cuenta: no es traducible.
    expect(statusCount(panel, "Sin traducir")).toBe("1");
    expect(statusCount(panel, "Borrador")).toBe("1");
    expect(statusCount(panel, "Revisado")).toBe("1");
    expect(statusCount(panel, "Final")).toBe("1");
    expect(within(panel).getByText("La carpeta del juego no se modifica.")).toBeInTheDocument();

    fireEvent.click(within(panel).getByRole("button", { name: "Cerrar" }));

    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Exportar" })).not.toBeInTheDocument());
    expect(mocks.dialogOpen).not.toHaveBeenCalled();
    expect(mocks.exportProject).not.toHaveBeenCalled();
  });

  it("filters the grid to drafts from the export panel without exporting", async () => {
    renderToolbar();

    const panel = await openExportPanel();
    fireEvent.click(within(panel).getByRole("button", { name: "Revisar borradores" }));

    expect(useEditorStore.getState().statusFilter).toBe("draft");
    expect(usePanelStore.getState().panel).toBe("none");
    expect(mocks.dialogOpen).not.toHaveBeenCalled();
    expect(mocks.exportProject).not.toHaveBeenCalled();
  });

  it("asks for the destination, exports and keeps the full report until closed", async () => {
    renderToolbar();

    const panel = await openExportPanel();
    fireEvent.click(within(panel).getByRole("button", { name: "Exportar" }));

    await waitFor(() => {
      expect(mocks.dialogOpen).toHaveBeenCalledOnce();
      expect(mocks.exportProject).toHaveBeenCalledWith("C:/Exports/Toolbar");
    });
    const report = await screen.findByRole("dialog", { name: "Exportado con advertencias" });
    expect(within(report).getByText("C:/Exports/Toolbar")).toBeInTheDocument();
    expect(within(report).getByText("3 de 4")).toBeInTheDocument();
    expect(within(report).getByText("Advertencia uno")).toBeInTheDocument();
    expect(within(report).getByText("Advertencia dos")).toBeInTheDocument();
    expect(within(report).getByText("Advertencia tres")).toBeInTheDocument();
  });

  it("offers no draft review when everything translatable is reviewed or final", async () => {
    mocks.project = makeProject(["reviewed", "final"]);
    mocks.exportProject.mockResolvedValue({
      filesExported: 1,
      entriesApplied: 2,
      totalEntries: 2,
      warnings: [],
      textFit: textFit(),
    });
    renderToolbar();

    const panel = await openExportPanel();
    expect(within(panel).getByRole("button", { name: "Revisar borradores" })).toBeDisabled();
    fireEvent.click(within(panel).getByRole("button", { name: "Exportar" }));

    await waitFor(() => expect(mocks.exportProject).toHaveBeenCalledWith("C:/Exports/Toolbar"));
    expect(await screen.findByRole("dialog", { name: "Exportado" })).toBeInTheDocument();
  });

  it("lists the entries that still do not fit and opens one in the grid", async () => {
    mocks.exportProject.mockResolvedValue({
      filesExported: 1,
      entriesApplied: 3,
      totalEntries: 4,
      warnings: [],
      textFit: textFit({
        wrappedEntries: 2,
        pagesAdded: 1,
        overflowCount: 1,
        overflow: [{
          filePath: "data/Map001.json",
          entryId: "entry-2",
          kind: "choice",
          width: 612,
          maxWidth: 560,
          lines: 1,
          maxLines: null,
        }],
      }),
    });
    useProjectStore.getState().setProject(mocks.project!, { markDirty: false });
    useEditorStore.getState().setStatusFilter("final");
    renderToolbar();

    const panel = await openExportPanel();
    fireEvent.click(within(panel).getByRole("button", { name: "Exportar" }));

    const list = await screen.findByRole("list", { name: "Entradas que no caben" });
    const item = within(list).getByRole("button", { name: /Texto 2/ });
    expect(item).toHaveTextContent("Elección");
    expect(item).toHaveTextContent("612/560 px");

    fireEvent.click(item);

    expect(usePanelStore.getState().panel).toBe("none");
    expect(useEditorStore.getState().statusFilter).toBeNull();
    expect(useEditorStore.getState().scrollToEntryId).toBe("entry-2");
    expect(useProjectStore.getState().activeFile?.relativePath).toBe("data/Map001.json");
  });

  it("closes batch translation and opens Settings from provider configuration", () => {
    renderToolbar();

    fireEvent.click(screen.getByRole("button", { name: "Traducir" }));
    fireEvent.click(screen.getByRole("button", { name: "Configure provider mock" }));

    expect(screen.queryByRole("button", { name: "Configure provider mock" })).not.toBeInTheDocument();
    expect(screen.getByRole("dialog", { name: "Settings panel mock" })).toHaveTextContent("google");
  });

  it("resets the provider when Settings is reopened manually", () => {
    renderToolbar();

    fireEvent.click(screen.getByRole("button", { name: "Traducir" }));
    fireEvent.click(screen.getByRole("button", { name: "Configure provider mock" }));
    expect(screen.getByRole("dialog", { name: "Settings panel mock" })).toHaveTextContent("google");

    fireEvent.click(screen.getByRole("button", { name: "Close settings mock" }));
    fireEvent.click(screen.getByRole("button", { name: "Ajustes" }));

    expect(screen.getByRole("dialog", { name: "Settings panel mock" })).toHaveTextContent("opencode");
  });
});
