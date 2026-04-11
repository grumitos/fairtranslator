import { create } from "zustand";
import { immer } from "zustand/middleware/immer";
import type { Project, TranslationFile } from "@/types/domain";

type DirtyMode = boolean | "preserve";

interface ProjectReplaceOptions {
  markDirty?: DirtyMode;
}

interface ProjectState {
  /** Current open project, null if none */
  project: Project | null;
  /** Currently selected file in the sidebar */
  activeFile: TranslationFile | null;
  /** True when the in-memory project diverges from the last saved-on-disk snapshot */
  isDirty: boolean;
  /** Loading state for async operations */
  isLoading: boolean;
  /** Last error message */
  error: string | null;

  // Actions
  setProject: (project: Project | null, options?: ProjectReplaceOptions) => void;
  replaceProject: (project: Project | null, options?: ProjectReplaceOptions) => void;
  setActiveFile: (file: TranslationFile | null) => void;
  setLoading: (loading: boolean) => void;
  setError: (error: string | null) => void;
  reset: () => void;
}

const initialState = {
  project: null,
  activeFile: null,
  isDirty: false,
  isLoading: false,
  error: null,
};

function resolveDirty(current: boolean, next: DirtyMode | undefined, hasProject: boolean) {
  if (!hasProject) {
    return false;
  }
  if (next === undefined || next === "preserve") {
    return current;
  }
  return next;
}

export const useProjectStore = create<ProjectState>()(
  immer((set) => ({
    ...initialState,

    setProject: (project, options) =>
      set((state) => {
        state.project = project;
        state.activeFile = null;
        state.isDirty = resolveDirty(state.isDirty, options?.markDirty, Boolean(project));
        state.error = null;
      }),

    replaceProject: (project, options) =>
      set((state) => {
        const activePath = state.activeFile?.relativePath;
        state.project = project;
        state.activeFile =
          project && activePath
            ? project.files.find((file) => file.relativePath === activePath) ?? null
            : null;
        state.isDirty = resolveDirty(state.isDirty, options?.markDirty, Boolean(project));
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
