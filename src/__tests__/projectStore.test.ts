import { act } from "@testing-library/react";
import { useProjectStore } from "@/stores/projectStore";
import type { Project } from "@/types/domain";

function makeProject(name: string, filePath = "data/Actors.json"): Project {
  return {
    id: `${name}-id`,
    name,
    sourceLang: "ja",
    targetLang: "es",
    engine: "mv",
    gameDir: `C:/Games/${name}`,
    files: [
      {
        relativePath: filePath,
        entries: [],
      },
    ],
    glossary: [],
    createdAt: "2026-01-01T00:00:00.000Z",
    updatedAt: "2026-01-01T00:00:00.000Z",
  };
}

describe("projectStore", () => {
  beforeEach(() => {
    useProjectStore.getState().reset();
  });

  it("preserves the active file when replacing the current project snapshot", () => {
    const initialProject = makeProject("initial");
    const refreshedProject = {
      ...initialProject,
      updatedAt: "2026-01-02T00:00:00.000Z",
      files: [
        {
          relativePath: "data/Actors.json",
          entries: [
            {
              id: "Actors:name:1",
              fileContext: "Actors > #1 > name",
              original: "Harold",
              translation: "Haroldo",
              status: "draft" as const,
              notes: "",
            },
          ],
        },
      ],
    };

    act(() => {
      useProjectStore.getState().setProject(initialProject);
      useProjectStore.getState().setActiveFile(initialProject.files[0] ?? null);
    });

    act(() => {
      useProjectStore.getState().replaceProject(refreshedProject);
    });

    const state = useProjectStore.getState();
    expect(state.project?.updatedAt).toBe("2026-01-02T00:00:00.000Z");
    expect(state.activeFile?.relativePath).toBe("data/Actors.json");
    expect(state.activeFile?.entries).toHaveLength(1);
  });
});
