import { act } from "@testing-library/react";
import { useSettingsStore } from "@/stores/settingsStore";

const STORAGE_KEY = "fairtranslator-settings";
const SYSTEM_THEME_QUERY = "(prefers-color-scheme: dark)";

function installMatchMedia(initialMatches: boolean) {
  let matches = initialMatches;
  const listeners = new Set<(event: MediaQueryListEvent) => void>();
  const addEventListener = vi.fn((_type: string, listener: (event: MediaQueryListEvent) => void) => {
    listeners.add(listener);
  });
  const removeEventListener = vi.fn((_type: string, listener: (event: MediaQueryListEvent) => void) => {
    listeners.delete(listener);
  });
  const mediaQuery = {
    get matches() {
      return matches;
    },
    media: SYSTEM_THEME_QUERY,
    onchange: null,
    addEventListener,
    removeEventListener,
    addListener: vi.fn(),
    removeListener: vi.fn(),
    dispatchEvent: vi.fn(() => true),
  } as unknown as MediaQueryList;

  Object.defineProperty(window, "matchMedia", {
    writable: true,
    value: vi.fn(() => mediaQuery),
  });

  return {
    addEventListener,
    removeEventListener,
    listenerCount: () => listeners.size,
    setMatches(nextMatches: boolean) {
      matches = nextMatches;
      const event = { matches, media: SYSTEM_THEME_QUERY } as MediaQueryListEvent;
      for (const listener of [...listeners]) listener(event);
    },
  };
}

describe("settingsStore", () => {
  beforeEach(() => {
    act(() => useSettingsStore.getState().setTheme("light"));
    localStorage.clear();
    document.documentElement.lang = "es";
    useSettingsStore.setState({
      theme: "dark",
      language: "es",
      wordWrapColumn: 0,
    });
  });

  afterEach(() => {
    act(() => useSettingsStore.getState().setTheme("light"));
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

  it("restores the document language from persisted settings", async () => {
    localStorage.setItem(STORAGE_KEY, JSON.stringify({
      state: {
        theme: "dark",
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
  });

  it("follows runtime system color-scheme changes", () => {
    const scheme = installMatchMedia(false);

    act(() => useSettingsStore.getState().setTheme("system"));
    expect(document.documentElement).not.toHaveClass("dark");
    expect(scheme.listenerCount()).toBe(1);

    act(() => scheme.setMatches(true));
    expect(document.documentElement).toHaveClass("dark");

    act(() => scheme.setMatches(false));
    expect(document.documentElement).not.toHaveClass("dark");
  });

  it.each(["light", "dark"] as const)(
    "stops following the system after selecting %s",
    (theme) => {
      const scheme = installMatchMedia(false);
      act(() => useSettingsStore.getState().setTheme("system"));

      act(() => useSettingsStore.getState().setTheme(theme));
      expect(scheme.removeEventListener).toHaveBeenCalledOnce();
      expect(scheme.listenerCount()).toBe(0);

      act(() => scheme.setMatches(theme !== "dark"));
      expect(document.documentElement.classList.contains("dark")).toBe(theme === "dark");
    },
  );

  it("does not accumulate system-theme listeners while rehydrating", async () => {
    const scheme = installMatchMedia(false);
    localStorage.setItem(STORAGE_KEY, JSON.stringify({
      state: {
        theme: "system",
        language: "es",
        wordWrapColumn: 0,
        sidebarWidth: 224,
        columnSizes: {},
      },
      version: 0,
    }));

    await act(async () => useSettingsStore.persist.rehydrate());
    expect(scheme.listenerCount()).toBe(1);

    await act(async () => useSettingsStore.persist.rehydrate());
    expect(scheme.listenerCount()).toBe(1);
    expect(scheme.addEventListener).toHaveBeenCalledTimes(2);
    expect(scheme.removeEventListener).toHaveBeenCalledOnce();
  });
});
