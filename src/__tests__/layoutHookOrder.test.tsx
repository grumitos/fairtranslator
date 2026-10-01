import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { vi } from "vitest";
import i18n from "@/lib/i18n";
import { Sidebar } from "@/components/layout/Sidebar";
import { EditorHeader } from "@/components/editor/EditorHeader";
import { useEditorStore } from "@/stores/editorStore";
import { useProjectStore } from "@/stores/projectStore";
import type { Project } from "@/types/domain";

vi.mock("@/components/ui/scroll-area", () => ({
  ScrollArea: ({ children, className }: { children: React.ReactNode; className?: string }) => (
    <div className={className}>{children}</div>
  ),
}));

function makeProject(): Project {
  return {
    id: "layout-flow",
    name: "Layout Flow Project",
    sourceLang: "ja",
    targetLang: "en",
    engine: "mv",
    gameDir: "C:/Games/LayoutFlow",
    files: [
      {
        relativePath: "data/Actors.json",
        entries: [
          {
            id: "Actors:name:1",
            fileContext: "Actors > #1 > name",
            original: "ハロルド",
            translation: "Harold",
            status: "reviewed",
            notes: "",
          },
        ],
      },
      {
        relativePath: "data/Map001.json",
        entries: [
          {
            id: "Map001:EV001:1",
            fileContext: "Map001 > EV001 > 1",
            original: "こんにちは、\\N[1]。",
            translation: "",
            status: "untranslated",
            notes: "",
          },
          {
            id: "Map001:EV002:1",
            fileContext: "Map001 > EV002 > 1",
            original: "宿屋へようこそ。",
            translation: "Welcome to the inn.",
            status: "final",
            notes: "",
          },
        ],
      },
    ],
    glossary: [],
    styleGuide: [],
    createdAt: "2026-01-01T00:00:00.000Z",
    updatedAt: "2026-01-01T00:00:00.000Z",
  };
}

describe("editor layout", () => {
  beforeEach(async () => {
    await i18n.changeLanguage("en");
    useProjectStore.getState().reset();
    useEditorStore.getState().reset();
  });

  afterEach(() => {
    cleanup();
    useProjectStore.getState().reset();
    useEditorStore.getState().reset();
  });

  it("follows the open project: global progress, file switching and status filtering", async () => {
    const { container } = render(
      <>
        <Sidebar />
        <EditorHeader />
      </>,
    );
    expect(container).toBeEmptyDOMElement();

    await act(async () => {
      useProjectStore.getState().setProject(makeProject(), { markDirty: false });
    });

    // Avance global en la barra lateral y alcance «todas las entradas» en la cabecera.
    const sidebar = screen.getByRole("complementary", { name: "Files" });
    expect(within(sidebar).getByRole("progressbar", { name: "67% complete" })).toBeInTheDocument();
    expect(within(sidebar).getByText("2 of 3")).toBeInTheDocument();
    expect(screen.getByRole("heading", { level: 1, name: "All entries" })).toBeInTheDocument();

    // Al elegir un archivo, la cabecera muestra su nombre, su ruta y sus cifras por estado.
    fireEvent.click(within(sidebar).getByRole("button", { name: /Map001/ }));
    expect(screen.getByRole("heading", { level: 1, name: "Map001" })).toBeInTheDocument();
    expect(screen.getByText("data/Map001.json")).toBeInTheDocument();
    const filters = screen.getByRole("radiogroup", { name: "Filter by status" });
    expect(within(filters).getByRole("radio", { name: /All\s*2/ })).toHaveAttribute("aria-checked", "true");
    expect(within(filters).getByRole("radio", { name: /Untranslated\s*1/ })).toBeInTheDocument();

    // El filtro por estado va a la tienda del editor, que es la que filtra el grid.
    fireEvent.click(within(filters).getByRole("radio", { name: /Final\s*1/ }));
    expect(useEditorStore.getState().statusFilter).toBe("final");
    expect(within(filters).getByRole("radio", { name: /Final\s*1/ })).toHaveAttribute("aria-checked", "true");
  });
});
