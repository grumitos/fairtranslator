import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import i18n from "@/lib/i18n";
import { BatchTranslatePanel } from "@/components/layout/BatchTranslatePanel";
import { useProjectStore } from "@/stores/projectStore";
import type { BatchProgress, Project, ProviderType } from "@/types/domain";

const mocks = vi.hoisted(() => ({
  apikeyLoad: vi.fn(),
  startBatch: vi.fn(),
  cancelBatch: vi.fn(),
  configureProvider: vi.fn(),
  translation: {
    isTranslating: false,
    progress: null as BatchProgress | null,
    error: null as string | null,
  },
}));

vi.mock("@/hooks/useTranslation", () => ({
  useTranslation: () => ({
    ...mocks.translation,
    startBatch: mocks.startBatch,
    cancelBatch: mocks.cancelBatch,
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

function makeProgress(overrides: Partial<BatchProgress> = {}): BatchProgress {
  return {
    runId: "12",
    cancelled: false,
    filesProcessed: 0,
    totalFiles: 1,
    chunksProcessed: 0,
    totalChunks: 3,
    entriesTranslated: 0,
    totalEntries: 3,
    currentFile: "",
    currentFileChunksProcessed: 0,
    currentFileTotalChunks: 0,
    failedEntries: 0,
    failedBatches: 0,
    warnings: [],
    ...overrides,
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
    mocks.cancelBatch.mockReset();
    mocks.configureProvider.mockReset();
    mocks.cancelBatch.mockResolvedValue(true);
    mocks.translation.isTranslating = false;
    mocks.translation.progress = null;
    mocks.translation.error = null;
    useProjectStore.getState().reset();
    useProjectStore.getState().setProject(makeProject(), { markDirty: false });
  });

  afterEach(() => {
    cleanup();
    useProjectStore.getState().reset();
  });

  it("gives the provider selector an accessible name", () => {
    render(<BatchTranslatePanel open onOpenChange={() => undefined} onConfigureProvider={mocks.configureProvider} />);

    expect(screen.getByRole("combobox", { name: "Proveedor" })).toBeInTheDocument();
  }, 10_000);

  it("offers provider configuration without starting when Google has no API key", async () => {
    render(<BatchTranslatePanel open onOpenChange={() => undefined} onConfigureProvider={mocks.configureProvider} />);

    await selectProvider("Google Translate");

    await waitFor(() => expect(mocks.apikeyLoad).toHaveBeenCalledWith("google"));
    expect(screen.getByText("Sin configurar")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Iniciar traducción" })).toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: "Configurar proveedor" }));
    expect(mocks.configureProvider).toHaveBeenCalledWith("google");
    expect(mocks.startBatch).not.toHaveBeenCalled();
  }, 10_000);

  it("surfaces provider configuration load failures with the same CTA", async () => {
    mocks.apikeyLoad.mockRejectedValueOnce(new Error("credential store unavailable"));
    render(<BatchTranslatePanel open onOpenChange={() => undefined} onConfigureProvider={mocks.configureProvider} />);

    await selectProvider("Google Translate");

    expect(await screen.findByText("No se pudo cargar la configuración")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Iniciar traducción" })).toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: "Configurar proveedor" }));
    expect(mocks.configureProvider).toHaveBeenCalledWith("google");
    expect(mocks.startBatch).not.toHaveBeenCalled();
  }, 10_000);

  it("identifies OpenCode as the integrated API and labels its single model field", async () => {
    render(<BatchTranslatePanel open onOpenChange={() => undefined} onConfigureProvider={mocks.configureProvider} />);

    await selectProvider("OpenCode Go · API integrada");

    expect(screen.getByRole("textbox", { name: "Modelo para esta ejecución" })).toBeInTheDocument();
    expect(screen.getAllByRole("textbox", { name: "Modelo para esta ejecución" })).toHaveLength(1);
  }, 10_000);

  it("offers cooperative cancellation for the active run", async () => {
    mocks.translation.isTranslating = true;
    mocks.translation.progress = makeProgress();
    render(<BatchTranslatePanel open onOpenChange={() => undefined} onConfigureProvider={mocks.configureProvider} />);

    fireEvent.click(screen.getByRole("button", { name: "Cancelar" }));

    await waitFor(() => expect(mocks.cancelBatch).toHaveBeenCalledTimes(1));
  }, 10_000);

  it("shows a cancelled terminal state with real progress", () => {
    mocks.translation.progress = makeProgress({
      cancelled: true,
      chunksProcessed: 1,
      entriesTranslated: 1,
    });
    render(<BatchTranslatePanel open onOpenChange={() => undefined} onConfigureProvider={mocks.configureProvider} />);

    expect(screen.getByText("Traducción cancelada")).toBeInTheDocument();
    expect(screen.getByText("Traducidas antes de cancelar: 1/3")).toBeInTheDocument();
  }, 10_000);

  it("shows provider validation failures before any chunk starts", () => {
    mocks.translation.progress = makeProgress({
      failedEntries: 3,
      failedBatches: 3,
      warnings: ["No se pudo validar el proveedor 'OpenAI': timeout"],
    });
    render(<BatchTranslatePanel open onOpenChange={() => undefined} onConfigureProvider={mocks.configureProvider} />);

    expect(screen.getByText("No se pudo validar el proveedor 'OpenAI': timeout")).toBeInTheDocument();
    expect(screen.getByText("Traducidas: 0/3. Fallidas: 3")).toBeInTheDocument();
  }, 10_000);
});
