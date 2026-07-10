import { create } from "zustand";
import { immer } from "zustand/middleware/immer";
import type { EntryStatus } from "@/types/domain";

type FindReplaceTarget = "original" | "translation" | "both";

interface EditorState {
  /** Search/filter query */
  searchQuery: string;
  /** Status filter */
  statusFilter: EntryStatus | null;

  // Find/Replace state
  /** Whether the find/replace bar is visible */
  showFindReplace: boolean;
  /** Find query text */
  findQuery: string;
  /** Replace text */
  replaceText: string;
  /** Match case toggle */
  matchCase: boolean;
  /** Use regex toggle */
  useRegex: boolean;
  /** Search target: original, translation, or both */
  searchTarget: FindReplaceTarget;
  /** Index of the currently focused match */
  currentMatchIndex: number;
  /** Total number of find matches (set by FindReplaceBar) */
  matchCount: number;
  /** Incremented when the user explicitly navigates matches */
  matchNavigationVersion: number;
  /** Entry ID to scroll into view */
  scrollToEntryId: string | null;

  // Grid navigation state
  /** Currently selected/highlighted row index in the grid */
  selectedRowIndex: number | null;

  // Actions
  setSearchQuery: (query: string) => void;
  setStatusFilter: (status: EntryStatus | null) => void;

  // Find/Replace actions
  setShowFindReplace: (show: boolean) => void;
  toggleFindReplace: () => void;
  setFindQuery: (query: string) => void;
  setReplaceText: (text: string) => void;
  setMatchCase: (matchCase: boolean) => void;
  setUseRegex: (useRegex: boolean) => void;
  setSearchTarget: (target: FindReplaceTarget) => void;
  setMatchCount: (count: number) => void;
  setScrollToEntryId: (id: string | null) => void;
  nextMatch: (totalMatches: number) => void;
  prevMatch: (totalMatches: number) => void;

  // Grid navigation actions
  setSelectedRowIndex: (index: number | null) => void;

  reset: () => void;
}

const initialState = {
  searchQuery: "",
  statusFilter: null,
  // Find/Replace
  showFindReplace: false,
  findQuery: "",
  replaceText: "",
  matchCase: false,
  useRegex: false,
  searchTarget: "translation" as FindReplaceTarget,
  currentMatchIndex: 0,
  matchCount: 0,
  matchNavigationVersion: 0,
  scrollToEntryId: null as string | null,
  // Grid navigation
  selectedRowIndex: null as number | null,
};

export const useEditorStore = create<EditorState>()(
  immer((set) => ({
    ...initialState,

    setSearchQuery: (query) =>
      set((state) => {
        state.searchQuery = query;
      }),

    setStatusFilter: (status) =>
      set((state) => {
        state.statusFilter = status;
      }),

    // Find/Replace actions
    setShowFindReplace: (show) =>
      set((state) => {
        state.showFindReplace = show;
        if (!show) {
          state.findQuery = "";
          state.replaceText = "";
          state.currentMatchIndex = 0;
          state.matchCount = 0;
          state.matchNavigationVersion = 0;
        }
      }),

    toggleFindReplace: () =>
      set((state) => {
        state.showFindReplace = !state.showFindReplace;
        if (!state.showFindReplace) {
          state.findQuery = "";
          state.replaceText = "";
          state.currentMatchIndex = 0;
          state.matchCount = 0;
          state.matchNavigationVersion = 0;
        }
      }),

    setFindQuery: (query) =>
      set((state) => {
        state.findQuery = query;
        state.currentMatchIndex = 0;
      }),

    setReplaceText: (text) =>
      set((state) => {
        state.replaceText = text;
      }),

    setMatchCase: (matchCase) =>
      set((state) => {
        state.matchCase = matchCase;
        state.currentMatchIndex = 0;
      }),

    setUseRegex: (useRegex) =>
      set((state) => {
        state.useRegex = useRegex;
        state.currentMatchIndex = 0;
      }),

    setSearchTarget: (target) =>
      set((state) => {
        state.searchTarget = target;
        state.currentMatchIndex = 0;
      }),

    setMatchCount: (count) =>
      set((state) => {
        state.matchCount = count;
      }),

    setScrollToEntryId: (id) =>
      set((state) => {
        state.scrollToEntryId = id;
      }),

    nextMatch: (totalMatches) =>
      set((state) => {
        if (totalMatches > 0) {
          state.currentMatchIndex = (state.currentMatchIndex + 1) % totalMatches;
          state.matchNavigationVersion += 1;
        }
      }),

    prevMatch: (totalMatches) =>
      set((state) => {
        if (totalMatches > 0) {
          state.currentMatchIndex =
            (state.currentMatchIndex - 1 + totalMatches) % totalMatches;
          state.matchNavigationVersion += 1;
        }
      }),

    // Grid navigation actions
    setSelectedRowIndex: (index) =>
      set((state) => {
        state.selectedRowIndex = index;
      }),

    reset: () => set(() => initialState),
  })),
);
