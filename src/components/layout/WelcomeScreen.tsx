import { useTranslation } from "react-i18next";
import { FolderOpen, Plus, X } from "lucide-react";
import { BrandMark } from "@/components/brand/BrandMark";
import { Kbd } from "@/components/common/Kbd";
import { Button } from "@/components/ui/button";
import { usePanelStore } from "@/components/layout/panelStore";
import { useProjectCommands } from "@/components/layout/useProjectCommands";
import { useSettingsStore, type RecentProject } from "@/stores/settingsStore";

// Sin proyecto abierto: marca, motores compatibles, las dos acciones de entrada y los recientes.
export function WelcomeScreen() {
  const { t, i18n } = useTranslation("common");
  const openPanel = usePanelStore((s) => s.open);
  const { openProject, openRecentProject } = useProjectCommands();
  const recentProjects = useSettingsStore((s) => s.recentProjects);
  const forgetProject = useSettingsStore((s) => s.forgetProject);

  return (
    <main className="flex min-h-0 flex-1 items-center justify-center overflow-y-auto p-6">
      <div className="flex w-[440px] max-w-full flex-col items-center gap-7">
        <BrandMark className="size-14 text-ink" />
        <div className="flex flex-col items-center gap-2 text-center">
          <h1 translate="no" className="text-2xl font-semibold tracking-[-0.02em]">
            {t("appName")}
          </h1>
          <p className="font-mono text-xs text-ink-3">{t("welcome.engines")}</p>
        </div>
        <div className="flex flex-wrap justify-center gap-2.5">
          <Button variant="brand" className="h-9 px-4" onClick={() => openPanel("newProject")}>
            <Plus strokeWidth={1.9} />
            {t("welcome.newProject")}
            <Kbd className="border-brand-ink/25 text-brand-ink/70">Ctrl N</Kbd>
          </Button>
          <Button variant="outline" className="h-9 px-4" onClick={() => void openProject()}>
            <FolderOpen strokeWidth={1.7} />
            {t("welcome.openProject")}
            <Kbd>Ctrl O</Kbd>
          </Button>
        </div>

        {recentProjects.length > 0 && (
          <section aria-labelledby="welcome-recent-title" className="mt-3 flex w-full flex-col">
            <h2 id="welcome-recent-title" className="m-0 mb-1.5 text-xs font-medium text-ink-3">
              {t("welcome.recent")}
            </h2>
            <ul className="m-0 flex list-none flex-col p-0">
              {recentProjects.map((recent) => (
                <li key={recent.path} className="group relative">
                  <button
                    type="button"
                    onClick={() => void openRecentProject(recent.path)}
                    className="grid w-full grid-cols-[minmax(0,1fr)_auto] items-center gap-x-4 gap-y-0.5 rounded-sm border-b border-line-soft px-2 py-[9px] pr-9 text-left outline-none transition-colors duration-(--motion-fast) hover:bg-surface-1 focus-visible:outline-2 focus-visible:outline-ink"
                  >
                    <span className="truncate text-[13px] font-medium">{recent.name}</span>
                    <span className="justify-self-end font-mono text-xs text-ink-2">{recent.percent}&nbsp;%</span>
                    <span className="truncate font-mono text-[11.5px] text-ink-4" title={recent.path}>
                      {recent.path}
                    </span>
                    <span className="justify-self-end text-[11.5px] text-ink-4">{formatWhen(recent, i18n.language, t)}</span>
                  </button>
                  <button
                    type="button"
                    onClick={() => forgetProject(recent.path)}
                    aria-label={t("welcome.forgetRecent", { name: recent.name })}
                    title={t("welcome.forgetRecent", { name: recent.name })}
                    className="absolute top-1/2 right-1.5 inline-flex size-6 -translate-y-1/2 items-center justify-center rounded-sm text-ink-3 opacity-0 transition-opacity duration-(--motion-fast) outline-none group-hover:opacity-100 hover:bg-surface-3 hover:text-ink focus-visible:opacity-100 focus-visible:outline-2 focus-visible:outline-ink"
                  >
                    <X className="size-3.5" strokeWidth={1.8} />
                  </button>
                </li>
              ))}
            </ul>
          </section>
        )}
      </div>
    </main>
  );
}

// "Hoy", "Ayer" o la fecha corta ("12 sept").
function formatWhen(recent: RecentProject, language: string, t: (key: string) => string) {
  const opened = new Date(recent.openedAt);
  if (Number.isNaN(opened.getTime())) return "";
  const startOfDay = (date: Date) => new Date(date.getFullYear(), date.getMonth(), date.getDate()).getTime();
  const days = Math.round((startOfDay(new Date()) - startOfDay(opened)) / 86_400_000);
  if (days <= 0) return t("welcome.today");
  if (days === 1) return t("welcome.yesterday");
  return new Intl.DateTimeFormat(language, { day: "numeric", month: "short" }).format(opened);
}
