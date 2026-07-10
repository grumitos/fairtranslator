import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import i18n from "@/lib/i18n";
import { ErrorBoundary } from "@/components/ErrorBoundary";

describe("ErrorBoundary recovery", () => {
  beforeEach(async () => {
    await i18n.changeLanguage("es");
    vi.spyOn(console, "error").mockImplementation(() => undefined);
  });

  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  it("recarga una vez sin volver a montar un árbol que siempre falla", () => {
    const reloadApp = vi.fn();
    let childRenders = 0;
    function AlwaysBroken(): never {
      childRenders += 1;
      throw new Error("Fallo determinista");
    }

    render(
      <ErrorBoundary reloadApp={reloadApp}>
        <AlwaysBroken />
      </ErrorBoundary>,
    );

    expect(screen.getByRole("alert")).toHaveTextContent("Algo salió mal");
    const rendersAfterFailure = childRenders;

    fireEvent.click(screen.getByRole("button"));

    expect(reloadApp).toHaveBeenCalledOnce();
    expect(childRenders).toBe(rendersAfterFailure);
    expect(screen.getByRole("button", { name: "Recargar aplicación" })).toBeInTheDocument();
    expect(screen.getByRole("alert")).toHaveTextContent("Fallo determinista");
  });
});
