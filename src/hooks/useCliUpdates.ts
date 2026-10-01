import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { toast } from "sonner";
import * as ipc from "@/lib/ipc";
import { useCliUpdateStore } from "@/stores/cliUpdateStore";
import type { CliUpdateStatus } from "@/types/domain";

/**
 * Sigue la actualización en segundo plano de los CLI que usa la app (el backend la lanza al
 * arrancar): carga el estado actual, escucha los cambios y avisa cuando un CLI se actualizó.
 */
export function useCliUpdates() {
  const { t } = useTranslation("settings");

  useEffect(() => {
    if (!isTauri()) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    const { setStatus, setStatuses } = useCliUpdateStore.getState();

    void ipc
      .cliUpdateStatus()
      .then((statuses) => {
        if (!disposed) setStatuses(statuses);
      })
      .catch(() => undefined);

    void listen<CliUpdateStatus>(ipc.CLI_UPDATE_EVENT, (event) => {
      setStatus(event.payload);
      if (event.payload.state === "updated") {
        toast.success(
          t("cli.updatedToast", {
            name: t(`cli.names.${event.payload.cli}`, { defaultValue: event.payload.cli }),
            version: event.payload.toVersion ?? "",
          }),
        );
      }
    })
      .then((dispose) => {
        if (disposed) dispose();
        else unlisten = dispose;
      })
      .catch(() => undefined);

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [t]);
}
