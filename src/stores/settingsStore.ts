import { create } from "zustand";
import { createJSONStorage, persist } from "zustand/middleware";
import { immer } from "zustand/middleware/immer";
import i18n from "@/lib/i18n";
import type { Engine, ProviderType } from "@/types/domain";

type Language = "es" | "en";

/** Proyecto abierto o guardado hace poco (pantalla de inicio). */
export interface RecentProject {
  path: string;
  name: string;
  engine: Engine;
  /** Porcentaje traducido al abrirlo o guardarlo por última vez. */
  percent: number;
  /** ISO 8601. */
  openedAt: string;
}

interface SettingsState {
  /** UI language */
  language: Language;
  /** Word wrap column limit (0 = disabled) */
  wordWrapColumn: number;
  /** Sidebar width in pixels */
  sidebarWidth: number;
  /** Persisted column widths by column ID */
  columnSizes: Record<string, number>;
  /** Provider of the last batch that started; preselected in the batch panel */
  lastProvider: ProviderType | null;
  /** Most recent first, at most MAX_RECENT_PROJECTS */
  recentProjects: RecentProject[];

  // Actions
  setLanguage: (language: Language) => void;
  setWordWrapColumn: (column: number) => void;
  setSidebarWidth: (width: number) => void;
  setColumnSizes: (sizes: Record<string, number>) => void;
  setLastProvider: (provider: ProviderType) => void;
  rememberProject: (project: RecentProject) => void;
  forgetProject: (path: string) => void;
}

const STORAGE_KEY = "fairtranslator-settings";
const MAX_WORD_WRAP_COLUMN = 500;
const MIN_SIDEBAR_WIDTH = 150;
const MAX_SIDEBAR_WIDTH = 400;
const DEFAULT_SIDEBAR_WIDTH = 224;
const MAX_RECENT_PROJECTS = 6;
const PROVIDERS: readonly ProviderType[] = ["openai", "deepl", "google", "codex", "opencode"];

// Windows no distingue mayúsculas ni el tipo de barra en las rutas.
function samePath(a: string, b: string) {
  const normalize = (path: string) => path.replace(/[\\/]+/g, "/").replace(/\/$/, "").toLowerCase();
  return normalize(a) === normalize(b);
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
      language: "es",
      wordWrapColumn: 0,
      sidebarWidth: DEFAULT_SIDEBAR_WIDTH,
      columnSizes: {},
      lastProvider: null,
      recentProjects: [],

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

      setLastProvider: (provider) =>
        set((state) => {
          state.lastProvider = provider;
        }),

      rememberProject: (project) =>
        set((state) => {
          const others = state.recentProjects.filter((recent) => !samePath(recent.path, project.path));
          state.recentProjects = [project, ...others].slice(0, MAX_RECENT_PROJECTS);
        }),

      forgetProject: (path) =>
        set((state) => {
          state.recentProjects = state.recentProjects.filter((recent) => !samePath(recent.path, path));
        }),
    })),
    {
      name: STORAGE_KEY,
      storage: createJSONStorage(() => localStorage),
      partialize: (state) => ({
        language: state.language,
        wordWrapColumn: state.wordWrapColumn,
        sidebarWidth: state.sidebarWidth,
        columnSizes: state.columnSizes,
        lastProvider: state.lastProvider,
        recentProjects: state.recentProjects,
      }),
      onRehydrateStorage: () => (state) => {
        if (!state) {
          return;
        }

        state.wordWrapColumn = normalizeWordWrapColumn(state.wordWrapColumn);
        state.sidebarWidth = normalizeSidebarWidth(state.sidebarWidth);
        if (!PROVIDERS.includes(state.lastProvider as ProviderType)) state.lastProvider = null;
        if (!Array.isArray(state.recentProjects)) state.recentProjects = [];
        applyLanguage(state.language);
      },
    },
  ),
);
