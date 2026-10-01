import { create } from "zustand";
import { immer } from "zustand/middleware/immer";
import type { Project, TranslationFile } from "@/types/domain";

type DirtyMode = boolean | "preserve";

interface ProjectReplaceOptions {
  markDirty?: DirtyMode;
}

/** Project fields that can be patched in place. `files` goes through replaceProject so activeFile stays valid. */
type ProjectPatch = Partial<Omit<Project, "id" | "files">>;

interface ProjectPatchOptions extends ProjectReplaceOptions {
  /** Ignore the patch when a different project is open by the time an async call resolves */
  projectId?: string;
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
  /**
   * Increments on every local change that dirties the project. Async flows that persist or
   * refresh the whole project compare it before and after to detect edits made in the meantime.
   */
  editRevision: number;

  // Actions
  setProject: (project: Project | null, options?: ProjectReplaceOptions) => void;
  replaceProject: (project: Project | null, options?: ProjectReplaceOptions) => void;
  /** Applies a partial change to the CURRENT project, so it never overwrites edits that landed while awaiting IPC */
  patchProject: (patch: ProjectPatch, options?: ProjectPatchOptions) => void;
  /** Flags the project as saved. Only valid when no local edit happened after the snapshot that reached the disk. */
  markClean: () => void;
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
  editRevision: 0,
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
        if (project && options?.markDirty === true) state.editRevision += 1;
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
        if (project && options?.markDirty === true) state.editRevision += 1;
        state.error = null;
      }),

    patchProject: (patch, options) =>
      set((state) => {
        if (!state.project) return;
        if (options?.projectId !== undefined && state.project.id !== options.projectId) return;
        Object.assign(state.project, patch);
        state.isDirty = resolveDirty(state.isDirty, options?.markDirty, true);
        if (options?.markDirty === true) state.editRevision += 1;
      }),

    markClean: () =>
      set((state) => {
        state.isDirty = false;
      }),

    setActiveFile: (file) =>
      set((state) => {
        state.activeFile = file;
      }),

    setLoading: (loading) =>
      set((state) => {
        state.isLoading = loading;
      }),

    // Reporting an error must not end a loading state: callers clear it in their own finally block.
    setError: (error) =>
      set((state) => {
        state.error = error;
      }),

    reset: () => set(() => initialState),
  })),
);
