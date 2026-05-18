import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import i18n from "@/lib/i18n";
import { StyleGuidePanel } from "@/components/layout/StyleGuidePanel";
import { useProjectStore } from "@/stores/projectStore";
import type { Project } from "@/types/domain";

vi.mock("@/components/ui/scroll-area", () => ({
  ScrollArea: ({ children, className }: { children: React.ReactNode; className?: string }) => (
    <div className={className}>{children}</div>
  ),
}));

function makeProject(): Project {
  return {
    id: "style-guide-panel",
    name: "Style Guide Project",
    sourceLang: "ja",
    targetLang: "es",
    engine: "mv",
    gameDir: "C:/Games/StyleGuide",
    files: [],
    glossary: [],
    styleGuide: ["Mantener nombres de menú cortos.", "Conservar códigos de control."],
    createdAt: "2026-01-01T00:00:00.000Z",
    updatedAt: "2026-01-01T00:00:00.000Z",
  };
}

describe("StyleGuidePanel", () => {
  beforeEach(async () => {
    await i18n.changeLanguage("es");
    useProjectStore.getState().reset();
    useProjectStore.getState().setProject(makeProject(), { markDirty: false });
  });

  afterEach(() => {
    cleanup();
    useProjectStore.getState().reset();
  });

  it("opens with existing rules outside Tauri without render loops", async () => {
    render(<StyleGuidePanel open onOpenChange={() => undefined} />);

    expect(await screen.findByText("Guía de estilo del proyecto")).toBeInTheDocument();
    expect(screen.getByText("Reglas (2)")).toBeInTheDocument();
    expect(screen.getByDisplayValue("Mantener nombres de menú cortos.")).toBeInTheDocument();
    expect(screen.getByDisplayValue("Conservar códigos de control.")).toBeInTheDocument();
  });

  it("edits and removes the visible normalized rule", async () => {
    useProjectStore.getState().replaceProject({
      ...makeProject(),
      styleGuide: ["Regla A", "", "Regla A", "Regla B"],
    }, { markDirty: false });

    render(<StyleGuidePanel open onOpenChange={() => undefined} />);

    expect(await screen.findByText("Reglas (2)")).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText("Regla 2"), { target: { value: "Regla C" } });
    expect(screen.queryByDisplayValue("Regla B")).not.toBeInTheDocument();
    expect(screen.getByDisplayValue("Regla C")).toBeInTheDocument();

    fireEvent.click(screen.getByLabelText("Eliminar regla de estilo 1"));
    expect(screen.queryByDisplayValue("Regla A")).not.toBeInTheDocument();
    expect(screen.getByDisplayValue("Regla C")).toBeInTheDocument();
  });
});
