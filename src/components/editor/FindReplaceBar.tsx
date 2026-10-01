import { useCallback, useDeferredValue, useEffect, useMemo, useRef } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { useEditorStore } from "@/stores/editorStore";
import { useProjectStore } from "@/stores/projectStore";
import { useProject } from "@/hooks/useProject";
import { FindControlsRow } from "@/components/editor/find-replace/FindControlsRow";
import { ReplaceControlsRow } from "@/components/editor/find-replace/ReplaceControlsRow";
import { buildFindRegex, type FindMatch } from "@/components/editor/findReplaceUtils";
import { isVisibleInGrid, toGridEntry } from "@/components/editor/gridEntries";
import { statusAfterEdit } from "@/lib/translationState";
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
  const statusFilter = useEditorStore((s) => s.statusFilter);
  // Igual que la tabla (texto diferido): se cuenta y se reemplaza solo sobre las filas visibles.
  const gridQuery = useDeferredValue(useEditorStore((s) => s.searchQuery)).trim().toLowerCase();

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
  const matches = useMemo<FindMatch[]>(() => {
    if (!regex) return [];
    const result: FindMatch[] = [];
    let dataIndex = 0;

    for (const file of searchableFiles) {
      for (const rawEntry of file.entries) {
        const entry = toGridEntry(rawEntry, file.relativePath);
        if (!isVisibleInGrid(entry, statusFilter, gridQuery)) continue;
        if (searchTarget === "original" || searchTarget === "both") {
          if (regex.test(entry.original)) {
            result.push({
              filePath: file.relativePath,
              entryId: entry.id,
              entry: rawEntry,
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
              entry: rawEntry,
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
  }, [regex, searchableFiles, searchTarget, statusFilter, gridQuery]);

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

    const entry = match.entry;

    const updated = (useRegex
      ? entry.translation.replace(regex, replaceText)
      // Sin regex el reemplazo es literal: $& o $$ no deben interpretarse como patrones.
      : entry.translation.replace(regex, () => replaceText));
    regex.lastIndex = 0;
    try {
      await updateEntry(
        match.filePath,
        match.entryId,
        updated,
        statusAfterEdit(entry.status, updated),
      );
    } catch (error) {
      toast.error(t("findReplace.replaceFailed"), {
        description: error instanceof Error ? error.message : String(error),
      });
    }
  }, [regex, matches, currentMatchIndex, replaceText, useRegex, updateEntry, t]);

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
      const entry = match.entry;

      const updated = (useRegex
      ? entry.translation.replace(regex, replaceText)
      // Sin regex el reemplazo es literal: $& o $$ no deben interpretarse como patrones.
      : entry.translation.replace(regex, () => replaceText));
      regex.lastIndex = 0;
      if (updated !== entry.translation) {
        updates.push({
          filePath: match.filePath,
          entryId: match.entryId,
          translation: updated,
          status: statusAfterEdit(entry.status, updated),
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
    } catch (error) {
      toast.error(t("findReplace.replaceFailed"), {
        description: error instanceof Error ? error.message : String(error),
      });
    }
  }, [regex, matches, replaceText, useRegex, updateEntriesBatch, t]);

  const matchInfoLabel = isInvalidRegex
    ? t("findReplace.invalidRegex")
    : findQuery
      ? matches.length > 0
        ? t("findReplace.currentMatch", { current: currentMatchIndex + 1, total: matches.length })
        : t("findReplace.noMatches")
      : "";
  const matchInfoShort = isInvalidRegex
    ? t("findReplace.invalidRegexShort")
    : findQuery
      ? t("findReplace.currentMatchShort", {
          current: matches.length > 0 ? currentMatchIndex + 1 : 0,
          total: matches.length,
        })
      : "";

  return (
    <div
      role="search"
      aria-label={t("toolbar.findReplace")}
      onKeyDown={(e) => {
        // Escape cierra la barra solo con el foco dentro de ella; los menús en portal (Select) lo gestionan antes.
        if (e.key !== "Escape" || e.defaultPrevented || !e.currentTarget.contains(e.target as Node)) return;
        handleClose();
      }}
      className="flex h-12 min-w-0 shrink-0 items-center gap-1.5 border-b border-line bg-surface-0 px-6"
    >
      <FindControlsRow
        findInputRef={findInputRef}
        findQuery={findQuery}
        isInvalidRegex={isInvalidRegex}
        matchInfoLabel={matchInfoLabel}
        matchInfoShort={matchInfoShort}
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
      <span aria-hidden="true" className="mx-1.5 h-5 w-px shrink-0 bg-line" />
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
