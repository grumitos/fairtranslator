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

  it("mounts one provider form and makes OpenCode discoverable first", () => {
    render(<SettingsPanel open onOpenChange={() => undefined} />);

    expect(screen.getByTestId("provider-form")).toHaveTextContent("opencode");
    expect(screen.getAllByTestId("provider-form")).toHaveLength(1);

    fireEvent.click(screen.getByRole("button", { name: /OpenAI/ }));

    expect(screen.getByTestId("provider-form")).toHaveTextContent("openai");
    expect(screen.getAllByTestId("provider-form")).toHaveLength(1);
  }, 10_000);

  it("opens directly on the provider requested by the translation flow", () => {
    render(
      <SettingsPanel
        open
        onOpenChange={() => undefined}
        initialProvider="google"
      />,
    );

    expect(screen.getByTestId("provider-form")).toHaveTextContent("google");
    expect(screen.getByRole("button", { name: /Google Translate/ })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
  }, 10_000);

  it("resets to OpenCode when reopened manually after a provider request", () => {
    const { rerender } = render(
      <SettingsPanel
        open
        onOpenChange={() => undefined}
        initialProvider="google"
      />,
    );
    expect(screen.getByTestId("provider-form")).toHaveTextContent("google");

    rerender(
      <SettingsPanel
        open={false}
        onOpenChange={() => undefined}
        initialProvider="google"
      />,
    );
    rerender(<SettingsPanel open onOpenChange={() => undefined} />);

    expect(screen.getByTestId("provider-form")).toHaveTextContent("opencode");
    expect(screen.getByRole("button", { name: /OpenCode/ })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
  }, 10_000);
});
