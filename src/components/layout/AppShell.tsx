import { useEffect, useRef, useState } from "react";
import { confirm as dialogConfirm } from "@tauri-apps/plugin-dialog";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { isTauri } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";
import { Files, TableProperties } from "lucide-react";
import { Sidebar } from "./Sidebar";
import { Toolbar } from "./Toolbar";
import { StatusBar } from "./StatusBar";
import { useProjectStore } from "@/stores/projectStore";
import { useEditorStore } from "@/stores/editorStore";
import { useSettingsStore } from "@/stores/settingsStore";
import { useKeyboardShortcuts } from "@/hooks/useKeyboardShortcuts";
import { WelcomeScreen } from "./WelcomeScreen";
import { TranslationGrid } from "@/components/editor/TranslationGrid";
import { FindReplaceBar } from "@/components/editor/FindReplaceBar";
import { cn } from "@/lib/utils";

export function AppShell() {
  const { t } = useTranslation(["common", "editor"]);
  const project = useProjectStore((s) => s.project);
  const isDirty = useProjectStore((s) => s.isDirty);
  const showFindReplace = useEditorStore((s) => s.showFindReplace);
  const sidebarWidth = useSettingsStore((s) => s.sidebarWidth);
  const setSidebarWidth = useSettingsStore((s) => s.setSidebarWidth);
  const [isResizingSidebar, setIsResizingSidebar] = useState(false);
  const [draftSidebarWidth, setDraftSidebarWidth] = useState(sidebarWidth);
  const [mobilePane, setMobilePane] = useState<"files" | "editor">("editor");
  const draftSidebarWidthRef = useRef(sidebarWidth);
  draftSidebarWidthRef.current = draftSidebarWidth;

  useKeyboardShortcuts();

  useEffect(() => {
    if (!import.meta.env.DEV || !window.location.search.includes("visualDemo=1")) {
      return;
    }

    let disposed = false;
    void import("@/dev/visualDemoProject").then(({ createVisualDemoProject }) => {
      if (!disposed && !useProjectStore.getState().project) {
        useProjectStore.getState().setProject(createVisualDemoProject(), { markDirty: false });
      }
    });

    return () => {
      disposed = true;
    };
  }, []);

  useEffect(() => {
    const appName = t("common:appName");
    if (!project) {
      document.title = appName;
      return;
    }

    document.title = `${isDirty ? "* " : ""}${project.name} - ${appName}`;
  }, [isDirty, project, t]);

  useEffect(() => {
    if (!isResizingSidebar) {
      setDraftSidebarWidth(sidebarWidth);
    }
  }, [sidebarWidth, isResizingSidebar]);

  useEffect(() => {
    if (isTauri()) {
      return;
    }

    const handleBeforeUnload = (event: BeforeUnloadEvent) => {
      const { project: currentProject, isDirty: currentDirty } = useProjectStore.getState();
      if (!currentProject || !currentDirty) {
        return;
      }

      event.preventDefault();
      event.returnValue = "";
    };

    window.addEventListener("beforeunload", handleBeforeUnload);
    return () => window.removeEventListener("beforeunload", handleBeforeUnload);
  }, []);

  useEffect(() => {
    if (!isTauri()) {
      return;
    }

    let disposed = false;
    let unlisten: (() => void) | undefined;

    void getCurrentWindow()
      .onCloseRequested(async (event) => {
        const { project: currentProject, isDirty: currentDirty } = useProjectStore.getState();
        if (!currentProject || !currentDirty) {
          return;
        }

        const shouldClose = await dialogConfirm(t("common:unsavedChanges.closeApp"), {
          title: t("common:appName"),
          kind: "warning",
          okLabel: t("common:close"),
          cancelLabel: t("common:cancel"),
        });

        if (!shouldClose) {
          event.preventDefault();
        }
      })
      .then((dispose) => {
        if (disposed) {
          dispose();
          return;
        }
        unlisten = dispose;
      })
      .catch(() => {
        // Browser/test environments can reach this path even with Tauri packages installed.
      });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [t]);

  useEffect(() => {
    if (!isResizingSidebar) return;

    const handleMouseMove = (e: MouseEvent) => {
      const newWidth = Math.min(400, Math.max(150, e.clientX));
      setDraftSidebarWidth(newWidth);
    };
    const handleMouseUp = () => {
      setIsResizingSidebar(false);
      setSidebarWidth(draftSidebarWidthRef.current);
    };

    document.addEventListener("mousemove", handleMouseMove);
    document.addEventListener("mouseup", handleMouseUp);
    document.body.style.userSelect = "none";
    document.body.style.cursor = "col-resize";

    return () => {
      document.removeEventListener("mousemove", handleMouseMove);
      document.removeEventListener("mouseup", handleMouseUp);
      document.body.style.userSelect = "";
      document.body.style.cursor = "";
    };
  }, [isResizingSidebar, setSidebarWidth]);

  if (!project) {
    return <WelcomeScreen />;
  }

  return (
    <div className="flex h-screen flex-col overflow-hidden bg-background">
      <Toolbar />

      {showFindReplace && <FindReplaceBar />}

      <div className="flex min-h-0 flex-1 overflow-hidden p-1.5 pt-1">
        <div
          className={cn(
            "min-h-0 flex-shrink-0 overflow-hidden rounded-md border bg-sidebar shadow-sm md:block",
            mobilePane === "files" ? "block w-full" : "hidden",
          )}
          style={{ width: mobilePane === "files" ? undefined : draftSidebarWidth }}
        >
          <Sidebar />
        </div>

        <div
          onMouseDown={(e) => { e.preventDefault(); setIsResizingSidebar(true); }}
          onKeyDown={(e) => {
            if (e.key !== "ArrowLeft" && e.key !== "ArrowRight") {
              return;
            }
            e.preventDefault();
            const delta = e.key === "ArrowLeft" ? -24 : 24;
            setSidebarWidth(Math.min(400, Math.max(150, sidebarWidth + delta)));
          }}
          className={cn(
            "ft-focus mx-1 hidden w-1 flex-shrink-0 cursor-col-resize rounded-full transition-colors duration-150 md:block",
            "hover:bg-primary/30",
            isResizingSidebar && "bg-primary/50",
          )}
          role="separator"
          tabIndex={0}
          aria-label={t("editor:sidebar.resize")}
          aria-orientation="vertical"
          aria-valuemin={150}
          aria-valuemax={400}
          aria-valuenow={sidebarWidth}
        />

        <main
          className={cn(
            "min-w-0 flex-1 overflow-hidden rounded-md border bg-card shadow-sm md:block",
            mobilePane === "editor" ? "block" : "hidden",
          )}
        >
          <TranslationGrid />
        </main>
      </div>

      <nav className="grid h-11 grid-cols-2 gap-1 border-t bg-card/95 p-1 md:hidden">
        <button
          type="button"
          onClick={() => setMobilePane("files")}
          className={cn(
            "ft-focus inline-flex items-center justify-center gap-2 rounded-md text-xs font-medium transition-colors duration-150",
            mobilePane === "files" ? "bg-accent text-accent-foreground" : "text-muted-foreground hover:bg-accent/70",
          )}
          aria-pressed={mobilePane === "files"}
        >
          <Files className="size-3.5" />
          {t("editor:sidebar.files")}
        </button>
        <button
          type="button"
          onClick={() => setMobilePane("editor")}
          className={cn(
            "ft-focus inline-flex items-center justify-center gap-2 rounded-md text-xs font-medium transition-colors duration-150",
            mobilePane === "editor" ? "bg-accent text-accent-foreground" : "text-muted-foreground hover:bg-accent/70",
          )}
          aria-pressed={mobilePane === "editor"}
        >
          <TableProperties className="size-3.5" />
          {t("editor:grid.translation")}
        </button>
      </nav>

      <StatusBar />
    </div>
  );
}
