import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import i18n from "@/lib/i18n";
import { GlossaryPanel } from "@/components/layout/GlossaryPanel";
import { useProjectStore } from "@/stores/projectStore";
import type { Project } from "@/types/domain";

const mocks = vi.hoisted(() => ({
  glossaryAdd: vi.fn(),
  glossaryUpdate: vi.fn(),
  glossaryRemove: vi.fn(),
  glossaryList: vi.fn(),
}));

vi.mock("@/lib/ipc", () => mocks);
vi.mock("@/components/ui/scroll-area", () => ({
  ScrollArea: ({ children, className }: { children: React.ReactNode; className?: string }) => (
    <div className={className}>{children}</div>
  ),
}));

const project: Project = {
  id: "glossary-panel",
  name: "Glossary Project",
  sourceLang: "en",
  targetLang: "es",
  engine: "mv",
  gameDir: "C:/Games/Glossary",
  files: [],
  glossary: [{ source: "Hero", target: "Héroe", caseSensitive: true }],
  styleGuide: [],
  createdAt: "2026-01-01T00:00:00.000Z",
  updatedAt: "2026-01-01T00:00:00.000Z",
};

describe("GlossaryPanel deletion", () => {
  beforeEach(async () => {
    await i18n.changeLanguage("es");
    useProjectStore.getState().reset();
    useProjectStore.getState().setProject(project, { markDirty: false });
    mocks.glossaryRemove.mockReset();
    mocks.glossaryRemove.mockResolvedValue([]);
  });

  afterEach(() => {
    cleanup();
    useProjectStore.getState().reset();
    vi.restoreAllMocks();
  });

  it("conserva el término y no llama IPC cuando se cancela", async () => {
    vi.spyOn(window, "confirm").mockReturnValue(false);
    render(<GlossaryPanel open onOpenChange={() => undefined} />);

    fireEvent.click(screen.getByRole("button", { name: "Eliminar término: Hero" }));

    await waitFor(() => {
      expect(window.confirm).toHaveBeenCalledWith(
        "¿Seguro que quieres eliminar el término \"Hero\"?",
      );
    });
    expect(mocks.glossaryRemove).not.toHaveBeenCalled();
    expect(screen.getByText("Hero")).toBeInTheDocument();
    expect(screen.getByText("Héroe")).toBeInTheDocument();
  });

  it("elimina el término una sola vez cuando se confirma", async () => {
    vi.spyOn(window, "confirm").mockReturnValue(true);
    render(<GlossaryPanel open onOpenChange={() => undefined} />);

    fireEvent.click(screen.getByRole("button", { name: "Eliminar término: Hero" }));

    await waitFor(() => expect(mocks.glossaryRemove).toHaveBeenCalledOnce());
    expect(mocks.glossaryRemove).toHaveBeenCalledWith("Hero");
    await waitFor(() => expect(screen.queryByText("Hero")).not.toBeInTheDocument());
  });
});
