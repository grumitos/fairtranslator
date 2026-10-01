import { create } from "zustand";
import type { CliUpdateStatus } from "@/types/domain";

interface CliUpdateState {
  /** Último estado de cada CLI, por id (`codex`, …). */
  statuses: Record<string, CliUpdateStatus>;
  setStatus: (status: CliUpdateStatus) => void;
  setStatuses: (statuses: CliUpdateStatus[]) => void;
}

export const useCliUpdateStore = create<CliUpdateState>((set) => ({
  statuses: {},
  setStatus: (status) => set((state) => ({ statuses: { ...state.statuses, [status.cli]: status } })),
  setStatuses: (statuses) => set({ statuses: Object.fromEntries(statuses.map((status) => [status.cli, status])) }),
}));
