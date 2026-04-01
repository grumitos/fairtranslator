import { act } from "@testing-library/react";
import { useSettingsStore } from "@/stores/settingsStore";

const STORAGE_KEY = "fairtranslator-settings";

describe("settingsStore", () => {
  beforeEach(() => {
    localStorage.clear();
    useSettingsStore.setState({
      theme: "dark",
      language: "es",
      wordWrapColumn: 0,
    });
  });

  it("persists theme, language and word wrap settings", () => {
    act(() => {
      useSettingsStore.getState().setTheme("light");
      useSettingsStore.getState().setLanguage("en");
      useSettingsStore.getState().setWordWrapColumn(120);
    });

    const raw = localStorage.getItem(STORAGE_KEY);
    expect(raw).not.toBeNull();

    const saved = JSON.parse(raw ?? "{}") as {
      state?: { theme?: string; language?: string; wordWrapColumn?: number };
    };

    expect(saved.state?.theme).toBe("light");
    expect(saved.state?.language).toBe("en");
    expect(saved.state?.wordWrapColumn).toBe(120);
  });

  it("normalizes invalid word wrap values", () => {
    act(() => {
      useSettingsStore.getState().setWordWrapColumn(Number.NaN);
    });
    expect(useSettingsStore.getState().wordWrapColumn).toBe(0);
  });
});
