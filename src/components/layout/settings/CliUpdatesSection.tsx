import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { isTauri } from "@tauri-apps/api/core";
import { toast } from "sonner";
import { Checkbox } from "@/components/ui/checkbox";
import * as ipc from "@/lib/ipc";
import { useCliUpdateStore } from "@/stores/cliUpdateStore";
import type { CliUpdateStatus } from "@/types/domain";

const TONE: Record<CliUpdateStatus["state"], string> = {
  checking: "text-ink-2",
  updating: "text-ink-2",
  up_to_date: "text-success",
  updated: "text-success",
  offline: "text-warning",
  failed: "text-danger",
  skipped: "text-ink-3",
};

/** Actualización automática de los CLI al arrancar y el último estado de cada uno. */
export function CliUpdatesSection({ open }: { open: boolean }) {
  const { t } = useTranslation(["settings", "common"]);
  const statuses = useCliUpdateStore((s) => s.statuses);
  const [autoUpdate, setAutoUpdate] = useState<boolean | null>(null);

  useEffect(() => {
    if (!open || !isTauri()) return;
    let active = true;
    ipc
      .cliAutoUpdateGet()
      .then((enabled) => {
        if (active) setAutoUpdate(enabled);
      })
      .catch(() => undefined);
    return () => {
      active = false;
    };
  }, [open]);

  const toggle = async (enabled: boolean) => {
    setAutoUpdate(enabled);
    try {
      await ipc.cliAutoUpdateSet(enabled);
    } catch (error) {
      setAutoUpdate(!enabled);
      toast.error(t("common:error"), { description: error instanceof Error ? error.message : String(error) });
    }
  };

  const list = Object.values(statuses);

  return (
    <section aria-labelledby="settings-cli-title" className="flex flex-col gap-2">
      <h3 id="settings-cli-title" className="text-xs font-medium text-ink-3">
        {t("settings:cli.title")}
      </h3>
      <label className="flex w-fit cursor-pointer items-center gap-2.5 text-[13px] has-disabled:cursor-default has-disabled:text-ink-3">
        <Checkbox
          checked={autoUpdate ?? true}
          disabled={autoUpdate === null}
          onCheckedChange={(checked) => void toggle(checked === true)}
        />
        {t("settings:cli.autoUpdate")}
      </label>
      {list.length > 0 && (
        <dl className="m-0 flex flex-col border-t border-line">
          {list.map((status) => (
            <div key={status.cli} className="flex items-center justify-between gap-3 border-b border-line-soft py-2">
              <dt className="text-[13px]">{t(`settings:cli.names.${status.cli}`, { defaultValue: status.cli })}</dt>
              <dd className="m-0 flex items-center gap-2 text-[12.5px]" title={status.message}>
                <span className={TONE[status.state]}>{stateLabel(status, t)}</span>
                {versionLabel(status) && <span className="font-mono text-ink-3">{versionLabel(status)}</span>}
              </dd>
            </div>
          ))}
        </dl>
      )}
    </section>
  );
}

function stateLabel(status: CliUpdateStatus, t: (key: string) => string) {
  if (status.state === "skipped") {
    return t(status.skipReason ? `settings:cli.skip.${status.skipReason}` : "settings:cli.states.skipped");
  }
  return t(`settings:cli.states.${status.state}`);
}

// Versiones como dato: la instalada, o "de → a" cuando hay (o hubo) una actualización.
function versionLabel(status: CliUpdateStatus) {
  const { fromVersion: from, toVersion: to, state } = status;
  if ((state === "updating" || state === "updated" || state === "failed") && from && to) return `${from} → ${to}`;
  if (state === "up_to_date" || state === "offline") return from ?? "";
  return "";
}
