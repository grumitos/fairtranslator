import { useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { isTauri } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";
import { Sidebar } from "./Sidebar";
import { Toolbar } from "./Toolbar";
import { useProjectStore } from "@/stores/projectStore";
import { useEditorStore } from "@/stores/editorStore";
import { useSettingsStore } from "@/stores/settingsStore";
import { useKeyboardShortcuts } from "@/hooks/useKeyboardShortcuts";
import { WelcomeScreen } from "./WelcomeScreen";
import { EditorHeader } from "@/components/editor/EditorHeader";
import { TranslationGrid } from "@/components/editor/TranslationGrid";
import { FindReplaceBar } from "@/components/editor/FindReplaceBar";
import { cn } from "@/lib/utils";
import { confirmAction } from "@/components/common/ConfirmDialog";

const SIDEBAR_MIN = 220;
const SIDEBAR_MAX = 420;

const clampSidebar = (width: number) => Math.min(SIDEBAR_MAX, Math.max(SIDEBAR_MIN, width));

export function AppShell() {
  const { t } = useTranslation(["common", "editor"]);
  const project = useProjectStore((s) => s.project);
  const isDirty = useProjectStore((s) => s.isDirty);
  const showFindReplace = useEditorStore((s) => s.showFindReplace);
  const sidebarWidth = useSettingsStore((s) => s.sidebarWidth);
  const setSidebarWidth = useSettingsStore((s) => s.setSidebarWidth);
  const [isResizingSidebar, setIsResizingSidebar] = useState(false);
  const [draftSidebarWidth, setDraftSidebarWidth] = useState(() => clampSidebar(sidebarWidth));
  const draftSidebarWidthRef = useRef(draftSidebarWidth);
  draftSidebarWidthRef.current = draftSidebarWidth;

  useKeyboardShortcuts();

  useEffect(() => {
    if (!import.meta.env.DEV || !window.location.search.includes("visualDemo=1")) {
      return;
    }

    let disposed = false;
    void import("@/dev/visualDemoProject").then((demo) => {
      if (disposed || useProjectStore.getState().project) return;
      demo.installVisualDemoBackend();
      const params = new URLSearchParams(window.location.search);
      if (params.get("screen") === "welcome") {
        demo.seedVisualDemoRecents();
        return;
      }
      useProjectStore.getState().setProject(demo.createVisualDemoProject(), { markDirty: false });
      void import("@/lib/projectLocation").then(({ refreshProjectLocation }) => refreshProjectLocation());
      demo.openVisualDemoPanel(params);
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
      setDraftSidebarWidth(clampSidebar(sidebarWidth));
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

        const shouldClose = await confirmAction({
          message: t("common:unsavedChanges.closeApp"),
          confirmLabel: t("common:discard"),
          destructive: true,
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
      setDraftSidebarWidth(clampSidebar(e.clientX));
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

  return (
    <div className="flex h-screen min-w-0 flex-col overflow-hidden bg-surface-0 text-ink">
      <Toolbar />

      {!project ? (
        <WelcomeScreen />
      ) : (
        <div className="flex min-h-0 flex-1 overflow-hidden">
          <div className="relative h-full shrink-0 overflow-hidden" style={{ width: draftSidebarWidth }}>
            <Sidebar />
            <div
              onMouseDown={(e) => {
                e.preventDefault();
                setIsResizingSidebar(true);
              }}
              onKeyDown={(e) => {
                if (e.key !== "ArrowLeft" && e.key !== "ArrowRight") {
                  return;
                }
                e.preventDefault();
                const delta = e.key === "ArrowLeft" ? -24 : 24;
                setSidebarWidth(clampSidebar(draftSidebarWidth + delta));
              }}
              className={cn(
                "absolute top-0 right-0 z-10 h-full w-1.5 cursor-col-resize outline-none",
                "after:absolute after:top-0 after:right-0 after:h-full after:w-px after:bg-transparent after:transition-colors after:duration-(--motion-fast)",
                "hover:after:bg-line-chip focus-visible:after:bg-ink",
                isResizingSidebar && "after:bg-line-chip",
              )}
              role="separator"
              tabIndex={0}
              aria-label={t("editor:sidebar.resize")}
              aria-orientation="vertical"
              aria-valuemin={SIDEBAR_MIN}
              aria-valuemax={SIDEBAR_MAX}
              aria-valuenow={draftSidebarWidth}
            />
          </div>

          <main className="flex min-w-0 flex-1 flex-col overflow-hidden">
            <EditorHeader />
            {showFindReplace && <FindReplaceBar />}
            <div className="min-h-0 flex-1">
              <TranslationGrid />
            </div>
          </main>
        </div>
      )}
    </div>
  );
}
