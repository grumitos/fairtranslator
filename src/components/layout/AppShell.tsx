import { useEffect, useRef, useState } from "react";
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
  const project = useProjectStore((s) => s.project);
  const showFindReplace = useEditorStore((s) => s.showFindReplace);
  const sidebarWidth = useSettingsStore((s) => s.sidebarWidth);
  const setSidebarWidth = useSettingsStore((s) => s.setSidebarWidth);
  const [isResizingSidebar, setIsResizingSidebar] = useState(false);
  const [draftSidebarWidth, setDraftSidebarWidth] = useState(sidebarWidth);
  const draftSidebarWidthRef = useRef(sidebarWidth);
  draftSidebarWidthRef.current = draftSidebarWidth;

  useKeyboardShortcuts();

  useEffect(() => {
    if (!isResizingSidebar) {
      setDraftSidebarWidth(sidebarWidth);
    }
  }, [sidebarWidth, isResizingSidebar]);

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
    <div className="flex h-screen flex-col overflow-hidden">
      {/* Toolbar */}
      <Toolbar />

      {/* Find & Replace bar */}
      {showFindReplace && <FindReplaceBar />}

      {/* Main area: sidebar + resize handle + content */}
      <div className="flex flex-1 overflow-hidden">
        {/* Sidebar */}
        <div className="flex-shrink-0" style={{ width: draftSidebarWidth }}>
          <Sidebar />
        </div>

        {/* Resize handle */}
        <div
          onMouseDown={(e) => { e.preventDefault(); setIsResizingSidebar(true); }}
          className={cn(
            "w-1 flex-shrink-0 cursor-col-resize transition-colors",
            "hover:bg-primary/30",
            isResizingSidebar && "bg-primary/50",
          )}
        />

        {/* Editor area */}
        <main className="flex-1 overflow-hidden">
          <TranslationGrid />
        </main>
      </div>

      {/* Status bar */}
      <StatusBar />
    </div>
  );
}
