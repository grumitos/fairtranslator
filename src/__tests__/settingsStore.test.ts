import { act } from "@testing-library/react";
import { useSettingsStore } from "@/stores/settingsStore";

const STORAGE_KEY = "fairtranslator-settings";

describe("settingsStore", () => {
  beforeEach(() => {
    localStorage.clear();
    document.documentElement.lang = "es";
    useSettingsStore.setState({
      language: "es",
      wordWrapColumn: 0,
      lastProvider: null,
    });
  });

  it("persists language, word wrap and the last batch provider", () => {
    act(() => {
      useSettingsStore.getState().setLanguage("en");
      useSettingsStore.getState().setWordWrapColumn(120);
      useSettingsStore.getState().setLastProvider("codex");
    });

    const raw = localStorage.getItem(STORAGE_KEY);
    expect(raw).not.toBeNull();

    const saved = JSON.parse(raw ?? "{}") as {
      state?: { language?: string; wordWrapColumn?: number; lastProvider?: string | null };
    };

    expect(saved.state?.language).toBe("en");
    expect(saved.state?.wordWrapColumn).toBe(120);
    expect(saved.state?.lastProvider).toBe("codex");
  });

  it("normalizes invalid word wrap values", () => {
    act(() => {
      useSettingsStore.getState().setWordWrapColumn(Number.NaN);
    });
    expect(useSettingsStore.getState().wordWrapColumn).toBe(0);
  });

  it("keeps the document language in sync when the UI language changes", () => {
    act(() => {
      useSettingsStore.getState().setLanguage("en");
    });
    expect(document.documentElement.lang).toBe("en");

    act(() => {
      useSettingsStore.getState().setLanguage("es");
    });
    expect(document.documentElement.lang).toBe("es");
  });

  it("restores persisted settings and drops an unknown provider", async () => {
    localStorage.setItem(STORAGE_KEY, JSON.stringify({
      state: {
        lastProvider: "removed-provider",
        language: "en",
        wordWrapColumn: 0,
        sidebarWidth: 224,
        columnSizes: {},
      },
      version: 0,
    }));

    await act(async () => {
      await useSettingsStore.persist.rehydrate();
    });

    expect(useSettingsStore.getState().language).toBe("en");
    expect(document.documentElement.lang).toBe("en");
    expect(useSettingsStore.getState().lastProvider).toBeNull();
  });
});
