import { create } from "zustand";
import * as ipc from "@/lib/ipc";
import type { ProjectLocation } from "@/types/domain";

/** Dónde vive el proyecto abierto en disco (lo sabe el backend; no forma parte de `Project`). */
export const useProjectLocationStore = create<{ location: ProjectLocation | null }>(() => ({ location: null }));

/**
 * Relee la ubicación tras crear, abrir o guardar. `undefined` si el backend no la puede dar (sin
 * Tauri o comando no disponible); `null` si no hay ruta de guardado y hace falta "Guardar como".
 */
export async function refreshProjectLocation(): Promise<ProjectLocation | null | undefined> {
  try {
    const location = await ipc.projectLocation();
    useProjectLocationStore.setState({ location });
    return location;
  } catch {
    useProjectLocationStore.setState({ location: null });
    return undefined;
  }
}
