// Regresiones de componentes halladas en la auditoría de frontend (C-01 … C-07). Cada caso fallaba
// con los componentes anteriores; solo usan roles, nombres accesibles y el estado de la tienda o del
// backend falso, así que sobreviven a cambios de estilo.
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import i18n from "@/lib/i18n";
import { backend, makeProject } from "@/test/fakeBackend";
import { TranslationGrid } from "@/components/editor/TranslationGrid";
import { FindReplaceBar } from "@/components/editor/FindReplaceBar";
import { TooltipProvider } from "@/components/ui/tooltip";
import { StyleGuideEditor } from "@/components/layout/StyleGuidePanel";
import { BatchTranslatePanel } from "@/components/layout/BatchTranslatePanel";
import { InlineEditCell } from "@/components/editor/translation-grid/InlineEditCell";
import { EditEntryDialog } from "@/components/editor/EditEntryDialog";
import { useProjectStore } from "@/stores/projectStore";
import { useEditorStore } from "@/stores/editorStore";
import type { EntryWithPath } from "@/components/editor/translation-grid/types";

vi.mock("@tauri-apps/api/core", async () => {
  const mod = (await import("@/test/fakeBackend")).tauriCoreMock;
  return { ...mod, isTauri: () => false };
});
vi.mock("@tauri-apps/api/event", async () => (await import("@/test/fakeBackend")).tauriEventMock);

vi.mock("@tanstack/react-virtual", () => ({
  useVirtualizer: (options: { count: number }) => ({
    getVirtualItems: () =>
      Array.from({ length: options.count }, (_, index) => ({ key: index, index, size: 52, start: index * 52 })),
    getTotalSize: () => options.count * 52,
    measureElement: () => undefined,
    measure: () => undefined,
    scrollToIndex: () => undefined,
  }),
}));

vi.mock("@/components/ui/scroll-area", () => ({
  ScrollArea: ({ children, className }: { children: React.ReactNode; className?: string }) => (
    <div className={className}>{children}</div>
  ),
}));

beforeAll(() => {
  for (const name of ["hasPointerCapture", "setPointerCapture", "releasePointerCapture", "scrollIntoView"]) {
    Object.defineProperty(HTMLElement.prototype, name, { configurable: true, value: () => false });
  }
});

beforeEach(async () => {
  await i18n.changeLanguage("es");
  backend.reset();
  useProjectStore.getState().reset();
  useEditorStore.getState().reset();
  vi.stubGlobal("__TAURI_INTERNALS__", { transformCallback: vi.fn() });
});

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

function seedTranslated() {
  const project = makeProject();
  project.files[0].entries[0].translation = "oro: 10 de oro";
  project.files[0].entries[0].status = "draft";
  project.files[0].entries[1].translation = "más oro";
  project.files[0].entries[1].status = "draft";
  useProjectStore.getState().setProject(project, { markDirty: false });
  backend.project = structuredClone(project);
  return project;
}

describe("grid and entry panel keyboard (C-01)", () => {
  it("does not let the grid shortcuts swallow Enter, arrows or Home typed in the entry panel", async () => {
    seedTranslated();
    render(<TranslationGrid />);

    const rows = screen.getAllByRole("row").filter((row) => Number(row.getAttribute("aria-rowindex")) > 1);
    expect(rows.length).toBeGreaterThan(0);
    fireEvent.doubleClick(within(rows[0]).getAllByRole("gridcell")[1]);

    const textarea = await screen.findByRole("textbox", { name: "Traducción" });
    // fireEvent devuelve false si alguien llamó preventDefault(): el navegador no insertaría el salto de línea.
    expect(fireEvent.keyDown(textarea, { key: "Enter" })).toBe(true);
    expect(fireEvent.keyDown(textarea, { key: "ArrowDown" })).toBe(true);
    expect(fireEvent.keyDown(textarea, { key: "Home" })).toBe(true);
  });
});

describe("find and replace (C-02, C-03)", () => {
  function renderBar() {
    useEditorStore.getState().setShowFindReplace(true);
    return render(
      <TooltipProvider>
        <FindReplaceBar />
      </TooltipProvider>,
    );
  }

  it("treats the replacement as literal text when regex mode is off", async () => {
    seedTranslated();
    renderBar();
    act(() => {
      useEditorStore.getState().setFindQuery("oro");
      useEditorStore.getState().setReplaceText("$$");
    });

    fireEvent.click(screen.getByRole("button", { name: "Reemplazar todas en Traducción" }));

    await waitFor(() => {
      expect(useProjectStore.getState().project?.files[0].entries[0].translation).toBe("$$: 10 de $$");
    });
  });

  it("counts and replaces only the rows the grid shows (C-09)", async () => {
    const project = makeProject();
    project.files[0].entries[0].translation = "oro: 10 de oro";
    project.files[0].entries[0].status = "draft";
    project.files[0].entries[1].translation = "más oro";
    project.files[0].entries[1].status = "final";
    useProjectStore.getState().setProject(project, { markDirty: false });
    backend.project = structuredClone(project);
    renderBar();
    act(() => {
      useEditorStore.getState().setStatusFilter("final");
      useEditorStore.getState().setFindQuery("oro");
      useEditorStore.getState().setReplaceText("plata");
    });
    expect(screen.getByText("1 de 1 coincidencias")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Reemplazar todas en Traducción" }));

    await waitFor(() => {
      expect(useProjectStore.getState().project?.files[0].entries[1].translation).toBe("más plata");
    });
    // La entrada en borrador queda fuera del filtro: no se toca.
    expect(useProjectStore.getState().project?.files[0].entries[0].translation).toBe("oro: 10 de oro");
  });

  it("keeps the current match inside the list after replacing the last match", async () => {
    seedTranslated();
    renderBar();
    act(() => {
      useEditorStore.getState().setFindQuery("oro");
    });
    act(() => {
      useEditorStore.getState().nextMatch(2);
    });
    expect(screen.getByText("2 de 2 coincidencias")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Reemplazar coincidencia en Traducción" }));
    await waitFor(() => {
      expect(useProjectStore.getState().project?.files[0].entries[1].translation).toBe("más ");
    });

    expect(screen.queryByText(/2 de 1 coincidencias/)).not.toBeInTheDocument();
  });
});

describe("style guide editor (C-04)", () => {
  function seedGuide(rules: string[]) {
    useProjectStore.getState().setProject(makeProject({ styleGuide: rules }), { markDirty: false });
  }

  it("keeps a trailing space typed at the end of a rule", async () => {
    seedGuide(["Regla A"]);
    render(<StyleGuideEditor active />);

    const input = await screen.findByLabelText("Regla 1");
    fireEvent.change(input, { target: { value: "Regla A " } });

    expect(screen.getByLabelText("Regla 1")).toHaveValue("Regla A ");
  });

  it("clearing a rule while editing does not move the next rule into it", async () => {
    seedGuide(["Regla A", "Regla B", "Regla C"]);
    render(<StyleGuideEditor active />);

    fireEvent.change(await screen.findByLabelText("Regla 2"), { target: { value: "" } });
    fireEvent.change(screen.getByLabelText("Regla 2"), { target: { value: "X" } });

    const values = screen.getAllByRole("textbox").map((el) => (el as HTMLInputElement).value);
    expect(values).toContain("Regla C");
    expect(values).toContain("X");
    expect(values).not.toContain("CX");
  });
});

describe("batch panel provider configuration (C-05)", () => {
  it("re-reads the provider configuration when the panel is reopened", async () => {
    seedTranslated();
    let configured = false;
    vi.spyOn(backend, "invoke").mockImplementation(async (command: string, args?: Record<string, unknown>) => {
      if (command === "apikey_load") return { provider: args?.provider, isConfigured: configured };
      return null;
    });

    const props = { onOpenChange: () => undefined, onConfigureProvider: () => undefined };
    const { rerender } = render(<BatchTranslatePanel open {...props} />);
    fireEvent.click(screen.getByRole("radio", { name: /Google Translate/ }));
    expect(await screen.findByRole("button", { name: "Configurar proveedor" })).toBeInTheDocument();

    // El usuario configura el proveedor en Ajustes y vuelve al panel de lotes.
    configured = true;
    rerender(<BatchTranslatePanel open={false} {...props} />);
    rerender(<BatchTranslatePanel open {...props} />);

    await waitFor(() =>
      expect(screen.queryByRole("button", { name: "Configurar proveedor" })).not.toBeInTheDocument(),
    );
  });
});

describe("IME composition (C-06)", () => {
  it("does not commit the inline edit while an IME composition is confirmed with Enter", () => {
    const entry: EntryWithPath = {
      id: "Map001:text:1",
      _filePath: "data/Map001.json",
      fileContext: "Map001",
      original: "Hello",
      translation: "",
      status: "untranslated",
      notes: "",
    };
    const onCommit = vi.fn();
    const onNavigate = vi.fn();
    render(
      <InlineEditCell
        entry={entry}
        initialValue=""
        onCommit={onCommit}
        onCancel={vi.fn()}
        onNavigate={onNavigate}
        onOpenDialog={vi.fn()}
        ariaLabel="Translation"
      />,
    );
    const textarea = screen.getByRole("textbox");
    fireEvent.change(textarea, { target: { value: "こんに" } });

    fireEvent.keyDown(textarea, { key: "Enter", isComposing: true, keyCode: 229 });

    expect(onCommit).not.toHaveBeenCalled();
    expect(onNavigate).not.toHaveBeenCalled();
  });
});

describe("entry panel status (C-07)", () => {
  it("promotes an untranslated entry to draft when a translation is typed and saved", async () => {
    const project = makeProject();
    useProjectStore.getState().setProject(project, { markDirty: false });
    backend.project = structuredClone(project);

    render(
      <EditEntryDialog
        entry={project.files[0].entries[0]}
        filePath={project.files[0].relativePath}
        open
        onOpenChange={() => undefined}
      />,
    );
    fireEvent.change(screen.getByLabelText("Traducción"), { target: { value: "Héroe" } });
    fireEvent.click(screen.getByRole("button", { name: "Guardar" }));

    await waitFor(() => expect(backend.callsTo("project_update_entry")).toHaveLength(1));
    expect(backend.callsTo("project_update_entry")[0].args).toMatchObject({ translation: "Héroe", status: "draft" });
  });
});
