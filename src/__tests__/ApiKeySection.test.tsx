import { cleanup, render, screen, waitFor } from "@testing-library/react";
import i18n from "@/lib/i18n";
import { ApiKeySection } from "@/components/layout/settings/ApiKeySection";

const load = vi.fn();

vi.mock("@/hooks/useApiKeys", () => ({
  useApiKeys: () => ({
    load,
    save: vi.fn(),
    remove: vi.fn(),
    validate: vi.fn(),
    listModels: vi.fn(),
    inspectCodex: vi.fn(),
    loginCodex: vi.fn(),
    isLoading: false,
  }),
}));

describe("ApiKeySection", () => {
  beforeEach(async () => {
    await i18n.changeLanguage("es");
    load.mockReset();
  });

  afterEach(cleanup);

  it("keeps the credential field blank when stored config loads", async () => {
    load.mockResolvedValue({
      provider: "opencode",
      model: "model-from-store",
      isConfigured: true,
    });

    render(<ApiKeySection provider="opencode" />);

    await waitFor(() => expect(load).toHaveBeenCalledWith("opencode"));
    expect(screen.getByLabelText("Clave de API")).toHaveValue("");
    expect(screen.getByRole("button", { name: "Guardar" })).toBeEnabled();
  }, 10_000);
});
