import { create } from "zustand";
import { immer } from "zustand/middleware/immer";
import type { Project, TranslationFile } from "@/types/domain";

interface ProjectState {
  /** Current open project, null if none */
  project: Project | null;
  /** Currently selected file in the sidebar */
  activeFile: TranslationFile | null;
  /** Loading state for async operations */
  isLoading: boolean;
  /** Last error message */
  error: string | null;

  // Actions
  setProject: (project: Project | null) => void;
  replaceProject: (project: Project | null) => void;
  setActiveFile: (file: TranslationFile | null) => void;
  setLoading: (loading: boolean) => void;
  setError: (error: string | null) => void;
  reset: () => void;
}

const initialState = {
  project: null,
  activeFile: null,
  isLoading: false,
  error: null,
};

export const useProjectStore = create<ProjectState>()(
  immer((set) => ({
    ...initialState,

    setProject: (project) =>
      set((state) => {
        state.project = project;
        state.activeFile = null;
        state.error = null;
      }),

    replaceProject: (project) =>
      set((state) => {
        const activePath = state.activeFile?.relativePath;
        state.project = project;
        state.activeFile =
          project && activePath
            ? project.files.find((file) => file.relativePath === activePath) ?? null
            : null;
        state.error = null;
      }),

    setActiveFile: (file) =>
      set((state) => {
        state.activeFile = file;
      }),

    setLoading: (loading) =>
      set((state) => {
        state.isLoading = loading;
      }),

    setError: (error) =>
      set((state) => {
        state.error = error;
        state.isLoading = false;
      }),

    reset: () => set(() => initialState),
  })),
);
