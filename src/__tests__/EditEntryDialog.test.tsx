import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import i18n from "@/lib/i18n";
import { EditEntryDialog } from "@/components/editor/EditEntryDialog";
import type { TranslationEntry } from "@/types/domain";

const mocks = vi.hoisted(() => ({
  updateEntry: vi.fn(),
  confirmAction: vi.fn(),
}));

vi.mock("@/hooks/useProject", () => ({
  useProject: () => ({ updateEntry: mocks.updateEntry }),
}));
vi.mock("@/components/common/ConfirmDialog", () => ({ confirmAction: mocks.confirmAction }));

const entry: TranslationEntry = {
  id: "Map001:text:1",
  fileContext: "Map001 > Evento 1",
  original: "Hello",
  translation: "Hola",
  status: "draft",
  notes: "",
};

function renderDirtyDialog(onOpenChange = vi.fn()) {
  render(
    <EditEntryDialog
      entry={entry}
      filePath="data/Map001.json"
      open
      onOpenChange={onOpenChange}
    />,
  );
  const translation = screen.getByLabelText("Traducción");
  fireEvent.change(translation, { target: { value: "Hola editada" } });
  return { onOpenChange, translation };
}

describe("EditEntryDialog unsaved changes", () => {
  beforeEach(async () => {
    await i18n.changeLanguage("es");
    mocks.updateEntry.mockReset();
    mocks.updateEntry.mockResolvedValue(undefined);
    mocks.confirmAction.mockReset();
    mocks.confirmAction.mockResolvedValue(false);
  });

  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  it.each([
    ["Escape", () => fireEvent.keyDown(document, { key: "Escape" })],
    ["el botón cerrar", () => fireEvent.click(screen.getByRole("button", { name: "Cerrar" }))],
    ["Cancelar", () => fireEvent.click(screen.getByRole("button", { name: "Cancelar" }))],
    ["un clic exterior", async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
      // Radix 1.6+ defers outside dismissal until the click completes, so a bare pointerdown is not a gesture.
      fireEvent.pointerDown(document.body, { button: 0, pointerType: "mouse" });
      fireEvent.pointerUp(document.body, { button: 0, pointerType: "mouse" });
      fireEvent.click(document.body, { button: 0 });
    }],
  ])("conserva el borrador cuando se rechaza cerrar con %s", async (_label, closeDialog) => {
    const { onOpenChange, translation } = renderDirtyDialog();

    await closeDialog();

    await waitFor(() => expect(mocks.confirmAction).toHaveBeenCalledOnce());
    expect(mocks.confirmAction).toHaveBeenCalledWith(expect.objectContaining({
      message: "Hay cambios sin guardar en esta entrada. ¿Descartarlos?",
      destructive: true,
    }));
    await Promise.resolve();
    expect(onOpenChange).not.toHaveBeenCalled();
    expect(translation).toHaveValue("Hola editada");
  });

  it("cierra cuando se confirma descartar los cambios", async () => {
    mocks.confirmAction.mockResolvedValue(true);
    const { onOpenChange } = renderDirtyDialog();

    fireEvent.click(screen.getByRole("button", { name: "Cancelar" }));

    await waitFor(() => expect(onOpenChange).toHaveBeenCalledWith(false));
  });

  it("guarda y cierra sin pedir una segunda confirmación", async () => {
    const { onOpenChange } = renderDirtyDialog();

    fireEvent.click(screen.getByRole("button", { name: "Guardar" }));

    await waitFor(() => {
      expect(mocks.updateEntry).toHaveBeenCalledWith(
        "data/Map001.json",
        entry.id,
        "Hola editada",
        "draft",
        "",
      );
      expect(onOpenChange).toHaveBeenCalledWith(false);
    });
    expect(mocks.confirmAction).not.toHaveBeenCalled();
  });
});
