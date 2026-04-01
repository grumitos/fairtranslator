import { useEffect } from "react";
import { useProject } from "@/hooks/useProject";
import { useEditorStore } from "@/stores/editorStore";

export function useKeyboardShortcuts() {
  const { save, project } = useProject();
  const toggleFindReplace = useEditorStore((s) => s.toggleFindReplace);
  const showFindReplace = useEditorStore((s) => s.showFindReplace);
  const nextMatch = useEditorStore((s) => s.nextMatch);
  const prevMatch = useEditorStore((s) => s.prevMatch);
  const matchCount = useEditorStore((s) => s.matchCount);

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const mod = e.ctrlKey || e.metaKey;
      if (!project) return;

      if (mod && e.key === "s") {
        e.preventDefault();
        save();
      }
      if (mod && e.key === "f") {
        e.preventDefault();
        if (!showFindReplace) {
          toggleFindReplace();
        }
      }
      if (mod && e.key === "h") {
        e.preventDefault();
        toggleFindReplace();
      }
      if (mod && e.key === "g" && showFindReplace) {
        e.preventDefault();
        if (e.shiftKey) {
          prevMatch(matchCount);
        } else {
          nextMatch(matchCount);
        }
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [project, save, toggleFindReplace, showFindReplace, nextMatch, prevMatch, matchCount]);
}
