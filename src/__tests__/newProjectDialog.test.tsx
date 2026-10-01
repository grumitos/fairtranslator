import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import i18n from "@/lib/i18n";
import { backend, makeProject } from "@/test/fakeBackend";
import { NewProjectDialog } from "@/components/layout/NewProjectDialog";
import { useProjectStore } from "@/stores/projectStore";

vi.mock("@tauri-apps/api/core", async () => (await import("@/test/fakeBackend")).tauriCoreMock);
vi.mock("@tauri-apps/api/event", async () => (await import("@/test/fakeBackend")).tauriEventMock);
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn().mockResolvedValue("C:/Games/Fake") }));

beforeEach(async () => {
  await i18n.changeLanguage("es");
  backend.reset();
  useProjectStore.getState().reset();
});

afterEach(cleanup);

async function chooseFolder() {
  render(<NewProjectDialog open onOpenChange={() => undefined} />);
  fireEvent.click(screen.getByRole("button", { name: "Elegir…" }));
  await waitFor(() => expect(backend.callsTo("game_inspect")).toHaveLength(1));
}

describe("new project dialog: game inspection", () => {
  it("shows what will be imported and creates the project", async () => {
    backend.gameFixture = makeProject();
    await chooseFolder();

    expect(await screen.findByText("RPG Maker MV")).toBeInTheDocument();
    const create = screen.getByRole("button", { name: "Crear proyecto" });
    await waitFor(() => expect(create).toBeEnabled());

    fireEvent.click(create);
    await waitFor(() => expect(backend.callsTo("project_create")).toHaveLength(1));
  });

  it("blocks creation when the folder only holds a packed game", async () => {
    backend.inspection = {
      gameRoot: null,
      engine: "vxAce",
      packedArchive: "Game.rgss3a",
      blockReason: "packed_archive",
      dataFiles: 0,
      textFiles: null,
      entries: null,
    };
    await chooseFolder();

    expect(await screen.findByRole("alert")).toHaveTextContent("Juego empaquetado (Game.rgss3a)");
    const create = screen.getByRole("button", { name: "Crear proyecto" });
    expect(create).toBeDisabled();
    fireEvent.click(create);
    expect(backend.callsTo("project_create")).toHaveLength(0);
  });
});
