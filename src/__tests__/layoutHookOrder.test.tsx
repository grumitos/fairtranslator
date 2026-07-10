import { act, cleanup, render, screen } from "@testing-library/react";
import { vi } from "vitest";
import i18n from "@/lib/i18n";
import { Sidebar } from "@/components/layout/Sidebar";
import { StatusBar } from "@/components/layout/StatusBar";
import { useProjectStore } from "@/stores/projectStore";
import type { Project } from "@/types/domain";

vi.mock("@/components/ui/scroll-area", () => ({
  ScrollArea: ({ children, className }: { children: React.ReactNode; className?: string }) => (
    <div className={className}>{children}</div>
  ),
}));

function makeProject(): Project {
  return {
    id: "layout-hook-order",
    name: "Hook Order Project",
    sourceLang: "ja",
    targetLang: "en",
    engine: "mv",
    gameDir: "C:/Games/HookOrder",
    files: [
      {
        relativePath: "data/Actors.json",
        entries: [
          {
            id: "Actors:name:1",
            fileContext: "Actors > #1 > name",
            original: "Harold",
            translation: "Harold",
            status: "reviewed",
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

describe("layout project transitions", () => {
  beforeEach(async () => {
    await i18n.changeLanguage("en");
    useProjectStore.getState().reset();
  });

  afterEach(() => {
    cleanup();
    useProjectStore.getState().reset();
  });

  it("keeps StatusBar hook order stable when a project opens", async () => {
    render(<StatusBar />);
    expect(screen.getByText("No project")).toBeInTheDocument();

    await act(async () => {
      useProjectStore.getState().setProject(makeProject(), { markDirty: false });
    });

    expect(screen.getByText("Engine: MV")).toBeInTheDocument();
    expect(screen.getByRole("progressbar", { name: "100% complete" })).toBeInTheDocument();
    expect(screen.getByText("100%")).toBeInTheDocument();
  });

  it("labels the narrow Unity engine profile", async () => {
    render(<StatusBar />);

    await act(async () => {
      useProjectStore.getState().setProject(
        { ...makeProject(), engine: "unity" },
        { markDirty: false },
      );
    });

    expect(screen.getByText("Engine: Unity (Loose JSON v0)")).toBeInTheDocument();
  });

  it("keeps Sidebar hook order stable when a project opens", async () => {
    const { container } = render(<Sidebar />);
    expect(container).toBeEmptyDOMElement();

    await act(async () => {
      useProjectStore.getState().setProject(makeProject(), { markDirty: false });
    });

    expect(screen.getByText("Hook Order Project")).toBeInTheDocument();
    expect(screen.getByText("All entries")).toBeInTheDocument();
  });
});
