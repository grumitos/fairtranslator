import { useCallback, useEffect, useMemo, useRef } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { useEditorStore } from "@/stores/editorStore";
import { useProjectStore } from "@/stores/projectStore";
import { useProject } from "@/hooks/useProject";
import { FindControlsRow } from "@/components/editor/find-replace/FindControlsRow";
import { ReplaceControlsRow } from "@/components/editor/find-replace/ReplaceControlsRow";
import { buildFindRegex, type FindMatch } from "@/components/editor/findReplaceUtils";
import type { ProjectEntryUpdate } from "@/types/domain";

export function FindReplaceBar() {
  const { t } = useTranslation("editor");
  const findInputRef = useRef<HTMLInputElement>(null);
  const project = useProjectStore((s) => s.project);
  const activeFile = useProjectStore((s) => s.activeFile);
  const { updateEntry, updateEntriesBatch } = useProject();

  const findQuery = useEditorStore((s) => s.findQuery);
  const replaceText = useEditorStore((s) => s.replaceText);
  const matchCase = useEditorStore((s) => s.matchCase);
  const useRegex = useEditorStore((s) => s.useRegex);
  const searchTarget = useEditorStore((s) => s.searchTarget);
  const currentMatchIndex = useEditorStore((s) => s.currentMatchIndex);
  const setFindQuery = useEditorStore((s) => s.setFindQuery);
  const setReplaceText = useEditorStore((s) => s.setReplaceText);
  const setMatchCase = useEditorStore((s) => s.setMatchCase);
  const setUseRegex = useEditorStore((s) => s.setUseRegex);
  const setSearchTarget = useEditorStore((s) => s.setSearchTarget);
  const setShowFindReplace = useEditorStore((s) => s.setShowFindReplace);
  const nextMatch = useEditorStore((s) => s.nextMatch);
  const prevMatch = useEditorStore((s) => s.prevMatch);
  const setMatchCount = useEditorStore((s) => s.setMatchCount);
  const matchNavigationVersion = useEditorStore((s) => s.matchNavigationVersion);
  const setScrollToEntryId = useEditorStore((s) => s.setScrollToEntryId);

  useEffect(() => {
    findInputRef.current?.focus();
  }, []);

  const regex = useMemo(
    () => buildFindRegex(findQuery, matchCase, useRegex),
    [findQuery, matchCase, useRegex],
  );
  const isInvalidRegex = useRegex && findQuery.length > 0 && regex === null;
  const projectFiles = activeFile ? null : project?.files;
  const searchableFiles = useMemo(() => {
    if (activeFile) return [activeFile];
    return projectFiles ?? [];
  }, [activeFile, projectFiles]);
  const entryByKey = useMemo(() => {
    const entries = new Map<string, (typeof searchableFiles)[number]["entries"][number]>();
    for (const file of searchableFiles) {
      for (const entry of file.entries) {
        entries.set(`${file.relativePath}:${entry.id}`, entry);
      }
    }
    return entries;
  }, [searchableFiles]);

  const matches = useMemo<FindMatch[]>(() => {
    if (!regex) return [];
    const result: FindMatch[] = [];
    let dataIndex = 0;

    for (const file of searchableFiles) {
      for (const entry of file.entries) {
        if (searchTarget === "original" || searchTarget === "both") {
          if (regex.test(entry.original)) {
            result.push({
              filePath: file.relativePath,
              entryId: entry.id,
              field: "original",
              dataIndex,
            });
          }
          regex.lastIndex = 0;
        }
        if (searchTarget === "translation" || searchTarget === "both") {
          if (regex.test(entry.translation)) {
            result.push({
              filePath: file.relativePath,
              entryId: entry.id,
              field: "translation",
              dataIndex,
            });
          }
          regex.lastIndex = 0;
        }
        dataIndex++;
      }
    }
    return result;
  }, [regex, searchableFiles, searchTarget]);

  // Keep matchCount in store for global keyboard shortcuts
  useEffect(() => {
    setMatchCount(matches.length);
  }, [matches.length, setMatchCount]);

  const handleClose = useCallback(() => {
    setShowFindReplace(false);
  }, [setShowFindReplace]);

  useEffect(() => {
    if (matchNavigationVersion === 0) {
      return;
    }
    const match = matches[currentMatchIndex];
    if (match) {
      setScrollToEntryId(match.entryId);
    }
  }, [matches, currentMatchIndex, matchNavigationVersion, setScrollToEntryId]);

  const handleNext = useCallback(() => {
    nextMatch(matches.length);
  }, [nextMatch, matches.length]);

  const handlePrev = useCallback(() => {
    prevMatch(matches.length);
  }, [prevMatch, matches.length]);

  const handleReplaceOne = useCallback(async () => {
    if (!regex || matches.length === 0) return;
    const match = matches[currentMatchIndex];
    if (!match || match.field !== "translation") {
      toast.error(t(match?.field === "original"
        ? "findReplace.noReplaceInOriginal"
        : "findReplace.noReplaceMatches"));
      return;
    }

    const entry = entryByKey.get(`${match.filePath}:${match.entryId}`);
    if (!entry) return;

    const updated = entry.translation.replace(regex, replaceText);
    regex.lastIndex = 0;
    try {
      await updateEntry(
        match.filePath,
        match.entryId,
        updated,
        entry.status === "untranslated" ? "draft" : entry.status,
      );
    } catch {}
  }, [regex, matches, currentMatchIndex, entryByKey, replaceText, updateEntry, t]);

  const handleReplaceAll = useCallback(async () => {
    if (!regex || matches.length === 0) return;
    const translationMatches = matches.filter((m) => m.field === "translation");
    if (translationMatches.length === 0) {
      toast.error(t("findReplace.noReplaceMatches"));
      return;
    }

    const seen = new Set<string>();
    const updates: ProjectEntryUpdate[] = [];
    for (const match of translationMatches) {
      const key = `${match.filePath}:${match.entryId}`;
      if (seen.has(key)) continue;
      seen.add(key);
      const entry = entryByKey.get(key);
      if (!entry) continue;

      const updated = entry.translation.replace(regex, replaceText);
      regex.lastIndex = 0;
      if (updated !== entry.translation) {
        updates.push({
          filePath: match.filePath,
          entryId: match.entryId,
          translation: updated,
          status: entry.status === "untranslated" ? "draft" : entry.status,
          notes: entry.notes,
        });
      }
    }
    if (updates.length === 0) {
      toast.error(t("findReplace.noReplaceMatches"));
      return;
    }
    try {
      await updateEntriesBatch(updates);
      toast.success(t("findReplace.replaced", { count: updates.length }));
    } catch {}
  }, [regex, matches, entryByKey, replaceText, updateEntriesBatch, t]);

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        handleClose();
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [handleClose]);

  const matchInfoLabel = isInvalidRegex
    ? t("findReplace.invalidRegex")
    : findQuery
      ? matches.length > 0
        ? t("findReplace.currentMatch", { current: currentMatchIndex + 1, total: matches.length })
        : t("findReplace.noMatches")
      : "";

  return (
    <div className="flex flex-col gap-1.5 border-b bg-muted/30 px-2 py-1.5">
      <FindControlsRow
        findInputRef={findInputRef}
        findQuery={findQuery}
        isInvalidRegex={isInvalidRegex}
        matchInfoLabel={matchInfoLabel}
        hasMatches={matches.length > 0}
        matchCase={matchCase}
        useRegex={useRegex}
        searchTarget={searchTarget}
        onFindChange={setFindQuery}
        onSubmitSearch={(goPrevious) => (goPrevious ? handlePrev() : handleNext())}
        onPrev={handlePrev}
        onNext={handleNext}
        onToggleMatchCase={() => setMatchCase(!matchCase)}
        onToggleRegex={() => setUseRegex(!useRegex)}
        onSearchTargetChange={(value) => setSearchTarget(value)}
        onClose={handleClose}
      />
      <ReplaceControlsRow
        replaceText={replaceText}
        disabled={matches.length === 0 || !findQuery}
        onReplaceTextChange={setReplaceText}
        onReplaceOne={handleReplaceOne}
        onReplaceAll={handleReplaceAll}
      />
    </div>
  );
}
