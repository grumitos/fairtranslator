import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import i18n from "@/lib/i18n";
import { BatchTranslatePanel } from "@/components/layout/BatchTranslatePanel";
import { useProjectStore } from "@/stores/projectStore";
import type { Project, ProviderType } from "@/types/domain";

const mocks = vi.hoisted(() => ({
  apikeyLoad: vi.fn(),
  startBatch: vi.fn(),
}));

vi.mock("@/hooks/useTranslation", () => ({
  useTranslation: () => ({
    isTranslating: false,
    progress: null,
    error: null,
    startBatch: mocks.startBatch,
  }),
}));

vi.mock("@/lib/ipc", () => ({
  apikeyLoad: mocks.apikeyLoad,
  apikeyModels: vi.fn(),
  projectGet: vi.fn(),
  translateAiBatchApply: vi.fn(),
  translateAiBatchExport: vi.fn(),
}));

vi.mock("@/components/ui/scroll-area", () => ({
  ScrollArea: ({ children, className }: { children: React.ReactNode; className?: string }) => (
    <div className={className}>{children}</div>
  ),
}));

function makeProject(): Project {
  return {
    id: "batch-panel",
    name: "Batch Panel",
    sourceLang: "ja",
    targetLang: "es",
    engine: "mv",
    gameDir: "C:/Games/BatchPanel",
    files: [{
      relativePath: "data/Actors.json",
      entries: [{
        id: "actor-1",
        fileContext: "Actor 1",
        original: "勇者",
        translation: "",
        status: "untranslated",
        notes: "",
      }],
    }],
    glossary: [],
    styleGuide: [],
    createdAt: "2026-01-01T00:00:00.000Z",
    updatedAt: "2026-01-01T00:00:00.000Z",
  };
}

async function selectProvider(name: string) {
  fireEvent.pointerDown(screen.getByRole("combobox"), {
    button: 0,
    ctrlKey: false,
    pointerType: "mouse",
  });
  const option = await screen.findByRole("option", { name });
  await act(async () => {
    fireEvent.click(option);
    await Promise.resolve();
  });
}

describe("BatchTranslatePanel", () => {
  beforeAll(() => {
    Object.defineProperty(HTMLElement.prototype, "hasPointerCapture", {
      configurable: true,
      value: () => false,
    });
    Object.defineProperty(HTMLElement.prototype, "setPointerCapture", {
      configurable: true,
      value: () => undefined,
    });
    Object.defineProperty(HTMLElement.prototype, "releasePointerCapture", {
      configurable: true,
      value: () => undefined,
    });
    Object.defineProperty(HTMLElement.prototype, "scrollIntoView", {
      configurable: true,
      value: () => undefined,
    });
  });

  beforeEach(async () => {
    await i18n.changeLanguage("es");
    mocks.apikeyLoad.mockImplementation(async (provider: ProviderType) => ({
      provider,
      isConfigured: false,
    }));
    mocks.startBatch.mockReset();
    useProjectStore.getState().reset();
    useProjectStore.getState().setProject(makeProject(), { markDirty: false });
  });

  afterEach(() => {
    cleanup();
    useProjectStore.getState().reset();
  });

  it("gives the provider selector an accessible name", () => {
    render(<BatchTranslatePanel open onOpenChange={() => undefined} />);

    expect(screen.getByRole("combobox", { name: "Proveedor" })).toBeInTheDocument();
  }, 10_000);

  it("keeps translation disabled when Google has no configured API key", async () => {
    render(<BatchTranslatePanel open onOpenChange={() => undefined} />);

    await selectProvider("Google Translate");

    await waitFor(() => expect(mocks.apikeyLoad).toHaveBeenCalledWith("google"));
    expect(screen.getByRole("button", { name: "Iniciar traducción" })).toBeDisabled();
  }, 10_000);

  it("identifies OpenCode as the integrated API and labels its single model field", async () => {
    render(<BatchTranslatePanel open onOpenChange={() => undefined} />);

    await selectProvider("OpenCode Go · API integrada");

    expect(screen.getByRole("textbox", { name: "Modelo para esta ejecución" })).toBeInTheDocument();
    expect(screen.getAllByRole("textbox", { name: "Modelo para esta ejecución" })).toHaveLength(1);
  }, 10_000);
});
