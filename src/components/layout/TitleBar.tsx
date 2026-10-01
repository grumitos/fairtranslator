import { useEffect, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { BookOpen, ChevronDown, FilePlus, FolderOpen, Languages, Save, SaveAll, SlidersHorizontal, Upload } from "lucide-react";
import { BrandMark } from "@/components/brand/BrandMark";
import { engineLabel } from "@/components/common/engineLabel";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuShortcut,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { cn } from "@/lib/utils";
import type { Engine } from "@/types/domain";

export interface TitleBarProject {
  name: string;
  engine: Engine;
  sourceLang: string;
  targetLang: string;
}

interface TitleBarProps {
  project: TitleBarProject | null;
  isDirty: boolean;
  busy: boolean;
  onNewProject: () => void;
  onOpenProject: () => void;
  onSave: () => void;
  onSaveAs: () => void;
  /** Dónde está guardado el proyecto, si ya se sabe. */
  location?: string | null;
  onExport: () => void;
  onTranslate: () => void;
  onGlossary: () => void;
  onSettings: () => void;
}

/*
 * Barra de título propia (ventana sin marco): marca, proyecto, acciones y controles de ventana.
 * Toda la franja libre arrastra la ventana; doble clic la maximiza (lo resuelve Tauri).
 */
export function TitleBar({
  project,
  isDirty,
  busy,
  onNewProject,
  onOpenProject,
  onSave,
  onSaveAs,
  location,
  onExport,
  onTranslate,
  onGlossary,
  onSettings,
}: TitleBarProps) {
  const { t } = useTranslation(["common", "editor"]);

  return (
    <header
      data-tauri-drag-region
      className="relative z-[60] flex h-12 shrink-0 items-center gap-4 border-b border-line bg-surface-0 pl-4 select-none"
    >
      <div data-tauri-drag-region className="flex shrink-0 items-center gap-[9px] [&>*]:pointer-events-none">
        <BrandMark className="size-5 text-ink" />
        <span translate="no" className="text-[14px] font-semibold tracking-[-0.01em]">
          {t("common:appName")}
        </span>
      </div>

      {project && (
        <div className="flex min-w-0 items-center gap-3">
          <span aria-hidden="true" className="h-5 w-px bg-line" />
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <button
                type="button"
                className="flex h-[30px] min-w-0 items-center gap-[9px] rounded-md pr-2 pl-2.5 text-sm text-ink outline-none hover:bg-surface-3 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ink data-[state=open]:bg-surface-3"
                aria-label={t("common:project")}
              >
                <span className="truncate font-medium">{project.name}</span>
                <span className="inline-flex h-[18px] shrink-0 items-center rounded-sm border border-line-chip px-1.5 font-mono text-[10px] font-bold tracking-[0.08em] text-ink-2">
                  {engineLabel(project.engine).toUpperCase()}
                </span>
                <span className="shrink-0 font-mono text-xs text-ink-3">
                  {project.sourceLang.toUpperCase()} → {project.targetLang.toUpperCase()}
                </span>
                <ChevronDown className="size-3.5 shrink-0 text-ink-3" strokeWidth={1.8} />
              </button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="start" className="max-w-[360px]">
              {location && (
                <>
                  <DropdownMenuLabel className="truncate font-mono text-[11.5px] font-normal text-ink-3" title={location}>
                    {location}
                  </DropdownMenuLabel>
                  <DropdownMenuSeparator />
                </>
              )}
              <DropdownMenuItem onSelect={onNewProject}>
                <FilePlus strokeWidth={1.6} />
                {t("common:newProject")}
                <DropdownMenuShortcut>Ctrl N</DropdownMenuShortcut>
              </DropdownMenuItem>
              <DropdownMenuItem onSelect={onOpenProject}>
                <FolderOpen strokeWidth={1.6} />
                {t("common:openProject")}
                <DropdownMenuShortcut>Ctrl O</DropdownMenuShortcut>
              </DropdownMenuItem>
              <DropdownMenuSeparator />
              <DropdownMenuItem onSelect={onSave} disabled={busy}>
                <Save strokeWidth={1.6} />
                {t("common:save")}
                <DropdownMenuShortcut>Ctrl S</DropdownMenuShortcut>
              </DropdownMenuItem>
              <DropdownMenuItem onSelect={onSaveAs} disabled={busy}>
                <SaveAll strokeWidth={1.6} />
                {t("common:saveAs")}
                <DropdownMenuShortcut>Ctrl Shift S</DropdownMenuShortcut>
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
      )}

      <div data-tauri-drag-region className="flex h-full min-w-6 flex-1" />

      <div className="flex h-full items-center gap-2">
        {project && (
          <>
            <span
              role="status"
              className={cn(
                "inline-flex w-24 items-center justify-end gap-1.5 text-xs text-ink-3 transition-opacity duration-(--motion-base)",
                isDirty ? "opacity-100" : "opacity-0",
              )}
            >
              <span aria-hidden="true" className="size-1.5 rounded-full bg-ink-2" />
              {isDirty ? t("common:unsavedChanges.badge") : ""}
            </span>
            <Button variant="outline" className="h-[30px]" onClick={onSave} disabled={busy} title="Ctrl+S">
              {t("common:save")}
            </Button>
            <Button variant="outline" className="h-[30px]" onClick={onExport} disabled={busy}>
              <Upload strokeWidth={1.7} />
              {t("common:export.button")}
            </Button>
            <Button variant="brand" className="h-[30px] px-[13px]" onClick={onTranslate} disabled={busy}>
              <Languages strokeWidth={1.8} />
              {t("editor:toolbar.translate")}
            </Button>
            <span aria-hidden="true" className="mx-1 h-5 w-px bg-line" />
            <IconButton label={t("editor:toolbar.glossary")} onClick={onGlossary}>
              <BookOpen strokeWidth={1.6} />
            </IconButton>
          </>
        )}
        <IconButton label={t("editor:toolbar.settings")} onClick={onSettings}>
          <SlidersHorizontal strokeWidth={1.6} />
        </IconButton>
        <WindowControls />
      </div>
    </header>
  );
}

function IconButton({ label, onClick, children }: { label: string; onClick: () => void; children: ReactNode }) {
  return (
    <Button variant="ghost" size="icon" aria-label={label} title={label} onClick={onClick}>
      {children}
    </Button>
  );
}

function WindowControls() {
  const { t } = useTranslation("common");
  const [maximized, setMaximized] = useState(false);
  const available = isTauri();

  useEffect(() => {
    if (!available) return;
    const appWindow = getCurrentWindow();
    let disposed = false;
    let unlisten: (() => void) | undefined;

    const sync = () => {
      void appWindow
        .isMaximized()
        .then((value) => {
          if (!disposed) setMaximized(value);
        })
        .catch(() => {});
    };

    sync();
    void appWindow
      .onResized(sync)
      .then((dispose) => {
        if (disposed) dispose();
        else unlisten = dispose;
      })
      .catch(() => {});

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [available]);

  if (!available) return <div className="w-2" />;

  const appWindow = getCurrentWindow();
  const control =
    "inline-flex h-12 w-[46px] items-center justify-center text-ink-2 outline-none transition-colors duration-(--motion-fast) hover:bg-surface-3 hover:text-ink focus-visible:bg-surface-3 focus-visible:text-ink";

  return (
    <div className="ml-2 flex h-full self-stretch">
      <button type="button" className={control} aria-label={t("window.minimize")} onClick={() => void appWindow.minimize()}>
        <svg viewBox="0 0 24 24" aria-hidden="true" className="size-3.5 fill-none stroke-current" strokeWidth={1.4} strokeLinecap="round">
          <path d="M6 12h12" />
        </svg>
      </button>
      <button
        type="button"
        className={control}
        aria-label={maximized ? t("window.restore") : t("window.maximize")}
        onClick={() => void appWindow.toggleMaximize()}
      >
        {maximized ? (
          <svg viewBox="0 0 24 24" aria-hidden="true" className="size-3.5 fill-none stroke-current" strokeWidth={1.4}>
            <rect x="6" y="8.5" width="9.5" height="9.5" rx="1.2" />
            <path d="M8.5 8.5V7.2A1.2 1.2 0 0 1 9.7 6H16.8A1.2 1.2 0 0 1 18 7.2V14.3A1.2 1.2 0 0 1 16.8 15.5H15.5" />
          </svg>
        ) : (
          <svg viewBox="0 0 24 24" aria-hidden="true" className="size-3.5 fill-none stroke-current" strokeWidth={1.4}>
            <rect x="6" y="6" width="12" height="12" rx="1.5" />
          </svg>
        )}
      </button>
      <button
        type="button"
        className={cn(control, "hover:bg-[#c42b1c] hover:text-white focus-visible:bg-[#c42b1c] focus-visible:text-white")}
        aria-label={t("window.close")}
        onClick={() => void appWindow.close()}
      >
        <svg viewBox="0 0 24 24" aria-hidden="true" className="size-3.5 fill-none stroke-current" strokeWidth={1.4} strokeLinecap="round">
          <path d="M6.5 6.5l11 11M17.5 6.5l-11 11" />
        </svg>
      </button>
    </div>
  );
}
