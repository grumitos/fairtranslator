import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import i18n from "@/lib/i18n";
import { ConfirmDialogHost, confirmAction } from "@/components/common/ConfirmDialog";

describe("ConfirmDialogHost", () => {
  beforeEach(async () => {
    await i18n.changeLanguage("es");
  });

  afterEach(cleanup);

  it("resolves true on the action and false on cancel or Escape", async () => {
    render(<ConfirmDialogHost />);
    const options = { message: "¿Descartar los cambios?", confirmLabel: "Descartar", destructive: true };

    let pending!: Promise<boolean>;
    act(() => {
      pending = confirmAction(options);
    });
    const dialog = await screen.findByRole("alertdialog", { name: "¿Descartar los cambios?" });
    fireEvent.click(screen.getByRole("button", { name: "Descartar" }));
    await expect(pending).resolves.toBe(true);
    await waitFor(() => expect(dialog).not.toBeInTheDocument());

    act(() => {
      pending = confirmAction(options);
    });
    fireEvent.click(await screen.findByRole("button", { name: "Cancelar" }));
    await expect(pending).resolves.toBe(false);

    act(() => {
      pending = confirmAction(options);
    });
    fireEvent.keyDown(await screen.findByRole("alertdialog"), { key: "Escape" });
    await expect(pending).resolves.toBe(false);
  });
});
