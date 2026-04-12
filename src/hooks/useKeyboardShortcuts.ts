import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { useProject } from "@/hooks/useProject";
import { useEditorStore } from "@/stores/editorStore";

function shouldIgnoreShortcutTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) {
    return false;
  }

  if (target.closest("[role='dialog']")) {
    return true;
  }

  if (target.isContentEditable) {
    return true;
  }

  return Boolean(
    target.closest("input, textarea, select, [contenteditable='true'], [contenteditable=''], [role='textbox']"),
  );
}

export function useKeyboardShortcuts() {
  const { t } = useTranslation("common");
  const { save, project } = useProject();
  const toggleFindReplace = useEditorStore((s) => s.toggleFindReplace);
  const showFindReplace = useEditorStore((s) => s.showFindReplace);
  const nextMatch = useEditorStore((s) => s.nextMatch);
  const prevMatch = useEditorStore((s) => s.prevMatch);
  const matchCount = useEditorStore((s) => s.matchCount);

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const mod = e.ctrlKey || e.metaKey;
      const key = e.key.toLowerCase();
      if (!project) return;
      if (!mod || shouldIgnoreShortcutTarget(e.target)) return;

      if (key === "s") {
        e.preventDefault();
        void save().catch((error: unknown) => {
          toast.error(t("error"), {
            description: error instanceof Error ? error.message : String(error),
          });
        });
      }
      if (key === "f") {
        e.preventDefault();
        if (!showFindReplace) {
          toggleFindReplace();
        }
      }
      if (key === "h") {
        e.preventDefault();
        toggleFindReplace();
      }
      if (key === "g" && showFindReplace) {
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
  }, [project, save, toggleFindReplace, showFindReplace, nextMatch, prevMatch, matchCount, t]);
}
