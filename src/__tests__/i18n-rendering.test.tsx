import { render, screen } from "@testing-library/react";
import i18n from "@/lib/i18n";
import { StatusBadge } from "@/components/editor/StatusBadge";

describe("i18n rendering", () => {
  it("renders translated status labels", async () => {
    await i18n.changeLanguage("es");
    render(<StatusBadge status="draft" />);
    expect(screen.getByText("Borrador")).toBeInTheDocument();
  });
});
