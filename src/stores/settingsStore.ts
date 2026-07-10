import { create } from "zustand";
import { createJSONStorage, persist } from "zustand/middleware";
import { immer } from "zustand/middleware/immer";
import i18n from "@/lib/i18n";

type Theme = "light" | "dark" | "system";
type Language = "es" | "en";

interface SettingsState {
  /** UI theme preference */
  theme: Theme;
  /** UI language */
  language: Language;
  /** Word wrap column limit (0 = disabled) */
  wordWrapColumn: number;
  /** Sidebar width in pixels */
  sidebarWidth: number;
  /** Persisted column widths by column ID */
  columnSizes: Record<string, number>;

  // Actions
  setTheme: (theme: Theme) => void;
  setLanguage: (language: Language) => void;
  setWordWrapColumn: (column: number) => void;
  setSidebarWidth: (width: number) => void;
  setColumnSizes: (sizes: Record<string, number>) => void;
}

const STORAGE_KEY = "fairtranslator-settings";
const MAX_WORD_WRAP_COLUMN = 500;
const MIN_SIDEBAR_WIDTH = 150;
const MAX_SIDEBAR_WIDTH = 400;
const DEFAULT_SIDEBAR_WIDTH = 224;
let systemThemeMedia: MediaQueryList | null = null;
let systemThemeListener: ((event: MediaQueryListEvent) => void) | null = null;

function clearSystemThemeListener() {
  if (systemThemeMedia && systemThemeListener) {
    systemThemeMedia.removeEventListener("change", systemThemeListener);
  }
  systemThemeMedia = null;
  systemThemeListener = null;
}

function applyTheme(theme: Theme) {
  if (typeof document === "undefined" || typeof window === "undefined") {
    return;
  }

  clearSystemThemeListener();

  if (theme === "dark") {
    document.documentElement.classList.add("dark");
    return;
  }

  if (theme === "light") {
    document.documentElement.classList.remove("dark");
    return;
  }

  systemThemeMedia = window.matchMedia("(prefers-color-scheme: dark)");
  systemThemeListener = (event) => {
    document.documentElement.classList.toggle("dark", event.matches);
  };
  document.documentElement.classList.toggle("dark", systemThemeMedia.matches);
  systemThemeMedia.addEventListener("change", systemThemeListener);
}

function applyLanguage(language: Language) {
  if (typeof document !== "undefined") document.documentElement.lang = language;
  void i18n.changeLanguage(language);
}

function normalizeWordWrapColumn(column: number): number {
  if (!Number.isFinite(column)) {
    return 0;
  }

  const normalized = Math.floor(column);
  return Math.max(0, Math.min(MAX_WORD_WRAP_COLUMN, normalized));
}

function normalizeSidebarWidth(width: number): number {
  if (!Number.isFinite(width)) {
    return DEFAULT_SIDEBAR_WIDTH;
  }
  return Math.max(MIN_SIDEBAR_WIDTH, Math.min(MAX_SIDEBAR_WIDTH, Math.floor(width)));
}

export const useSettingsStore = create<SettingsState>()(
  persist(
    immer((set) => ({
      theme: "dark",
      language: "es",
      wordWrapColumn: 0,
      sidebarWidth: DEFAULT_SIDEBAR_WIDTH,
      columnSizes: {},

      setTheme: (theme) =>
        set((state) => {
          state.theme = theme;
          applyTheme(theme);
        }),

      setLanguage: (language) =>
        set((state) => {
          state.language = language;
          applyLanguage(language);
        }),

      setWordWrapColumn: (column) =>
        set((state) => {
          state.wordWrapColumn = normalizeWordWrapColumn(column);
        }),

      setSidebarWidth: (width) =>
        set((state) => {
          state.sidebarWidth = normalizeSidebarWidth(width);
        }),

      setColumnSizes: (sizes) =>
        set((state) => {
          state.columnSizes = sizes;
        }),
    })),
    {
      name: STORAGE_KEY,
      storage: createJSONStorage(() => localStorage),
      partialize: (state) => ({
        theme: state.theme,
        language: state.language,
        wordWrapColumn: state.wordWrapColumn,
        sidebarWidth: state.sidebarWidth,
        columnSizes: state.columnSizes,
      }),
      onRehydrateStorage: () => (state) => {
        if (!state) {
          return;
        }

        state.wordWrapColumn = normalizeWordWrapColumn(state.wordWrapColumn);
        state.sidebarWidth = normalizeSidebarWidth(state.sidebarWidth);
        applyTheme(state.theme);
        applyLanguage(state.language);
      },
    },
  ),
);
