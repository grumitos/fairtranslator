import { create } from "zustand";
import type { ProviderType } from "@/types/domain";

/*
 * Qué panel lateral o diálogo de la ventana está abierto. Es estado de interfaz: lo comparten la
 * barra de título, la pantalla de inicio y el controlador de paneles (Toolbar).
 */
export type Panel = "none" | "newProject" | "settings" | "context" | "batch" | "export";
export type ContextTab = "glossary" | "styleGuide";

interface PanelState {
  panel: Panel;
  settingsProvider: ProviderType | undefined;
  contextTab: ContextTab;
  open: (panel: Exclude<Panel, "none">) => void;
  openSettings: (provider?: ProviderType) => void;
  openContext: (tab?: ContextTab) => void;
  setContextTab: (tab: ContextTab) => void;
  close: () => void;
}

export const usePanelStore = create<PanelState>()((set) => ({
  panel: "none",
  settingsProvider: undefined,
  contextTab: "glossary",
  open: (panel) => set({ panel }),
  openSettings: (provider) => set({ panel: "settings", settingsProvider: provider }),
  openContext: (tab = "glossary") => set({ panel: "context", contextTab: tab }),
  setContextTab: (tab) => set({ contextTab: tab }),
  close: () => set({ panel: "none" }),
}));
