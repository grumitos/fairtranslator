import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import i18n from "@/lib/i18n";
import { SettingsPanel } from "@/components/layout/SettingsPanel";

vi.mock("@/components/layout/settings/ApiKeySection", () => ({
  ApiKeySection: ({ provider }: { provider: string }) => {
    const [mountedProvider] = useState(provider);
    return <div data-testid="provider-form">{mountedProvider}</div>;
  },
}));

describe("SettingsPanel providers", () => {
  beforeEach(async () => {
    await i18n.changeLanguage("es");
  });

  afterEach(cleanup);

  it("expands one provider form at a time", () => {
    render(<SettingsPanel open onOpenChange={() => undefined} />);

    expect(screen.queryByTestId("provider-form")).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /OpenAI/ }));
    expect(screen.getByTestId("provider-form")).toHaveTextContent("openai");

    fireEvent.click(screen.getByRole("button", { name: /Google Translate/ }));
    expect(screen.getByTestId("provider-form")).toHaveTextContent("google");
    expect(screen.getAllByTestId("provider-form")).toHaveLength(1);
    expect(screen.getByRole("button", { name: /OpenAI/ })).toHaveAttribute("aria-expanded", "false");
  }, 10_000);

  it("opens directly on the provider requested by the translation flow", () => {
    render(<SettingsPanel open onOpenChange={() => undefined} initialProvider="google" />);

    expect(screen.getByTestId("provider-form")).toHaveTextContent("google");
    expect(screen.getByRole("button", { name: /Google Translate/ })).toHaveAttribute("aria-expanded", "true");
  }, 10_000);

  it("starts collapsed when reopened manually after a provider request", () => {
    const { rerender } = render(
      <SettingsPanel open onOpenChange={() => undefined} initialProvider="google" />,
    );
    expect(screen.getByTestId("provider-form")).toHaveTextContent("google");

    rerender(<SettingsPanel open={false} onOpenChange={() => undefined} initialProvider="google" />);
    rerender(<SettingsPanel open onOpenChange={() => undefined} />);

    expect(screen.queryByTestId("provider-form")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Google Translate/ })).toHaveAttribute("aria-expanded", "false");
  }, 10_000);
});
