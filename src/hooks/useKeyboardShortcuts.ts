import { useEffect } from "react";
import { useProject } from "@/hooks/useProject";
import { useProjectCommands } from "@/components/layout/useProjectCommands";
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
  const { project } = useProject();
  const { saveProject, saveProjectAs } = useProjectCommands();
  const toggleFindReplace = useEditorStore((s) => s.toggleFindReplace);
  const showFindReplace = useEditorStore((s) => s.showFindReplace);
  const nextMatch = useEditorStore((s) => s.nextMatch);
  const prevMatch = useEditorStore((s) => s.prevMatch);
  const matchCount = useEditorStore((s) => s.matchCount);

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const mod = e.ctrlKey || e.metaKey;
      const key = e.key.toLowerCase();
      if (!project || !mod) return;

      // Ctrl+S guarda también desde un campo de texto: la edición en línea se confirma (blur) antes de
      // guardar, y save() espera las ediciones en vuelo. Dentro de un cajón o diálogo no guarda el
      // proyecto (el cajón tiene sus propios cambios sin confirmar), pero sí evita el "guardar página".
      if (key === "s") {
        e.preventDefault();
        const target = e.target instanceof HTMLElement ? e.target : null;
        if (target?.closest("[role='dialog'], [role='alertdialog']")) return;
        const inlineEditor = target?.closest("[data-ft-inline-edit]");
        if (inlineEditor instanceof HTMLElement) inlineEditor.blur();
        void (e.shiftKey ? saveProjectAs() : saveProject());
        return;
      }
      if (shouldIgnoreShortcutTarget(e.target)) return;

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
  }, [project, saveProject, saveProjectAs, toggleFindReplace, showFindReplace, nextMatch, prevMatch, matchCount]);
}
