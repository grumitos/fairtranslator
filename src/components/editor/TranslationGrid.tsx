import { useCallback, useDeferredValue, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { getCoreRowModel, getFilteredRowModel, useReactTable, type Row } from "@tanstack/react-table";
import { useVirtualizer } from "@tanstack/react-virtual";
import { useProjectStore } from "@/stores/projectStore";
import { useEditorStore } from "@/stores/editorStore";
import { useSettingsStore } from "@/stores/settingsStore";
import { EditEntryDialog } from "@/components/editor/EditEntryDialog";
import { buildFindRegex } from "@/components/editor/findReplaceUtils";
import { useTranslationGridColumns } from "@/components/editor/translation-grid/useTranslationGridColumns";
import { TranslationGridTableView } from "@/components/editor/translation-grid/TranslationGridTableView";
import { useInlineEdit } from "@/components/editor/translation-grid/useInlineEdit";
import type { EntryWithPath } from "@/components/editor/translation-grid/types";

const ROW_HEIGHT_ESTIMATE = 52;

function buildSearchText(entry: Omit<EntryWithPath, "_filePath" | "_searchText">): string {
  return [entry.original, entry.translation, entry.fileContext, entry.id].join("\n").toLowerCase();
}

function toGridEntry(
  entry: Omit<EntryWithPath, "_filePath" | "_searchText">,
  filePath: string,
): EntryWithPath {
  return { ...entry, _filePath: filePath, _searchText: buildSearchText(entry) };
}

export function TranslationGrid() {
  const { t } = useTranslation("editor");
  const project = useProjectStore((s) => s.project);
  const activeFile = useProjectStore((s) => s.activeFile);
  const searchQuery = useEditorStore((s) => s.searchQuery);
  const statusFilter = useEditorStore((s) => s.statusFilter);
  const wordWrapColumn = useSettingsStore((s) => s.wordWrapColumn);
  const columnSizes = useSettingsStore((s) => s.columnSizes);
  const setColumnSizes = useSettingsStore((s) => s.setColumnSizes);
  const findQuery = useEditorStore((s) => s.findQuery);
  const matchCase = useEditorStore((s) => s.matchCase);
  const useRegex = useEditorStore((s) => s.useRegex);
  const searchTarget = useEditorStore((s) => s.searchTarget);
  const showFindReplace = useEditorStore((s) => s.showFindReplace);
  const scrollToEntryId = useEditorStore((s) => s.scrollToEntryId);
  const setScrollToEntryId = useEditorStore((s) => s.setScrollToEntryId);
  const selectedRowIndex = useEditorStore((s) => s.selectedRowIndex);
  const setSelectedRowIndex = useEditorStore((s) => s.setSelectedRowIndex);
  const deferredSearchQuery = useDeferredValue(searchQuery);

  const [dialogEntry, setDialogEntry] = useState<EntryWithPath | null>(null);
  const rowsRef = useRef<Row<EntryWithPath>[]>([]);
  const filteredDataRef = useRef<EntryWithPath[]>([]);
  const selectedRowKeyRef = useRef<string | null>(null);

  const findRegex = useMemo(
    () => (showFindReplace ? buildFindRegex(findQuery, matchCase, useRegex) : null),
    [showFindReplace, findQuery, matchCase, useRegex],
  );
  const wrapStyle = useMemo(
    () => (wordWrapColumn > 0 ? { maxWidth: `${wordWrapColumn}ch`, wordBreak: "break-word" as const } : undefined),
    [wordWrapColumn],
  );

  const activeFileData = useMemo<EntryWithPath[] | null>(() => {
    if (!activeFile) return null;
    return activeFile.entries.map((entry) => toGridEntry(entry, activeFile.relativePath));
  }, [activeFile]);

  const projectFiles = activeFile ? null : project?.files;
  const data = useMemo<EntryWithPath[]>(() => {
    if (activeFileData) return activeFileData;
    if (!projectFiles) return [];
    return projectFiles.flatMap((file) => file.entries.map((entry) => toGridEntry(entry, file.relativePath)));
  }, [activeFileData, projectFiles]);
  const getEntryKey = useCallback(
    (entry: EntryWithPath | null | undefined) => (entry ? `${entry._filePath}:${entry.id}` : null),
    [],
  );

  const filteredData = useMemo(() => {
    let result = data;
    if (statusFilter) {
      result = result.filter((entry) => entry.status === statusFilter);
    }
    if (deferredSearchQuery.trim()) {
      const q = deferredSearchQuery.toLowerCase();
      result = result.filter((entry) => (entry._searchText ?? buildSearchText(entry)).includes(q));
    }
    return result;
  }, [data, statusFilter, deferredSearchQuery]);

  filteredDataRef.current = filteredData;

  useEffect(() => {
    if (selectedRowIndex === null) {
      selectedRowKeyRef.current = null;
      return;
    }
    selectedRowKeyRef.current = getEntryKey(filteredDataRef.current[selectedRowIndex]);
  }, [selectedRowIndex, getEntryKey]);

  useEffect(() => {
    if (selectedRowIndex === null) {
      return;
    }

    const selectedKey = selectedRowKeyRef.current;
    if (!selectedKey) {
      setSelectedRowIndex(null);
      return;
    }

    const nextIndex = filteredData.findIndex((entry) => getEntryKey(entry) === selectedKey);
    if (nextIndex === -1) {
      setSelectedRowIndex(null);
      return;
    }
    if (nextIndex !== selectedRowIndex) {
      setSelectedRowIndex(nextIndex);
    }
  }, [filteredData, selectedRowIndex, setSelectedRowIndex, getEntryKey]);

  const matchedEntryIds = useMemo(() => {
    if (!findRegex || !showFindReplace) return new Set<string>();
    const ids = new Set<string>();
    for (const entry of filteredData) {
      if (searchTarget === "original" || searchTarget === "both") {
        if (findRegex.test(entry.original)) ids.add(entry.id);
        findRegex.lastIndex = 0;
      }
      if (searchTarget === "translation" || searchTarget === "both") {
        if (findRegex.test(entry.translation)) ids.add(entry.id);
        findRegex.lastIndex = 0;
      }
    }
    return ids;
  }, [findRegex, showFindReplace, filteredData, searchTarget]);

  const columns = useTranslationGridColumns(findRegex, searchTarget, wrapStyle);

  const parentRef = useRef<HTMLDivElement>(null);
  const virtualizer = useVirtualizer({
    count: filteredData.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => ROW_HEIGHT_ESTIMATE,
    overscan: 20,
  });

  const {
    inlineEditingEntryId,
    startEdit,
    commitEdit,
    cancelEdit,
    navigateEdit,
  } = useInlineEdit({ virtualizer });

  // Use a ref-based callback for navigate so it doesn't create a circular dependency with rows
  const handleNavigateInlineEdit = useCallback((dir: "next" | "prev", currentIdx: number) => {
    navigateEdit(dir, rowsRef.current, currentIdx);
  }, [navigateEdit]);

  const handleOpenDialog = useCallback((entry: EntryWithPath) => {
    cancelEdit();
    setDialogEntry(entry);
  }, [cancelEdit]);

  const table = useReactTable({
    data: filteredData,
    columns,
    getCoreRowModel: getCoreRowModel(),
    getFilteredRowModel: getFilteredRowModel(),
    enableColumnResizing: true,
    columnResizeMode: "onEnd",
    state: {
      columnSizing: columnSizes,
    },
    onColumnSizingChange: (updater) => {
      const newSizes = typeof updater === "function"
        ? updater(columnSizes)
        : updater;
      setColumnSizes(newSizes);
    },
    meta: {
      inlineEditingEntryId,
      selectedRowIndex,
      onStartInlineEdit: startEdit,
      onCommitInlineEdit: commitEdit,
      onCancelInlineEdit: cancelEdit,
      onNavigateInlineEdit: handleNavigateInlineEdit,
      onOpenDialog: handleOpenDialog,
    },
  });
  const rows = table.getRowModel().rows;
  rowsRef.current = rows;

  // Re-measure rows when word wrap changes
  useEffect(() => {
    virtualizer.measure();
  }, [wordWrapColumn, virtualizer]);

  // Scroll-to-match: react to scrollToEntryId changes
  useEffect(() => {
    if (!scrollToEntryId) return;
    const rowIndex = filteredData.findIndex((e) => e.id === scrollToEntryId);
    if (rowIndex >= 0) {
      virtualizer.scrollToIndex(rowIndex, { align: "center" });
      setSelectedRowIndex(rowIndex);
    }
    setScrollToEntryId(null);
  }, [scrollToEntryId, filteredData, virtualizer, setSelectedRowIndex, setScrollToEntryId]);

  // Keyboard navigation in grid
  const handleGridKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (inlineEditingEntryId) return;
    if (rows.length === 0) {
      if (e.key === "Escape") {
        setSelectedRowIndex(null);
      }
      return;
    }

    switch (e.key) {
      case "ArrowDown": {
        e.preventDefault();
        const next = Math.min((selectedRowIndex ?? -1) + 1, rows.length - 1);
        setSelectedRowIndex(next);
        virtualizer.scrollToIndex(next, { align: "auto" });
        break;
      }
      case "ArrowUp": {
        e.preventDefault();
        const prev = Math.max((selectedRowIndex ?? 1) - 1, 0);
        setSelectedRowIndex(prev);
        virtualizer.scrollToIndex(prev, { align: "auto" });
        break;
      }
      case "Enter": {
        e.preventDefault();
        if (selectedRowIndex !== null && selectedRowIndex < rows.length) {
          const entry = rows[selectedRowIndex]?.original;
          if (entry) startEdit(entry.id);
        }
        break;
      }
      case "Home": {
        e.preventDefault();
        setSelectedRowIndex(0);
        virtualizer.scrollToIndex(0, { align: "start" });
        break;
      }
      case "End": {
        e.preventDefault();
        const lastIdx = rows.length - 1;
        if (lastIdx >= 0) {
          setSelectedRowIndex(lastIdx);
          virtualizer.scrollToIndex(lastIdx, { align: "end" });
        }
        break;
      }
      case "Escape": {
        setSelectedRowIndex(null);
        break;
      }
    }
  }, [inlineEditingEntryId, selectedRowIndex, rows, setSelectedRowIndex, virtualizer, startEdit]);

  const handleRowDoubleClick = useCallback((entry: EntryWithPath) => {
    cancelEdit();
    setDialogEntry(entry);
  }, [cancelEdit]);

  if (!project) {
    return null;
  }

  return (
    <div
      className="flex h-full flex-col outline-none"
      tabIndex={0}
      onKeyDown={handleGridKeyDown}
    >
      <TranslationGridTableView
        table={table}
        rows={rows}
        parentRef={parentRef}
        virtualItems={virtualizer.getVirtualItems()}
        totalSize={virtualizer.getTotalSize()}
        matchedEntryIds={matchedEntryIds}
        measureElement={virtualizer.measureElement}
        onRowDoubleClick={handleRowDoubleClick}
        emptyText={t("grid.noEntries")}
        totalEntriesText={t("grid.totalEntries", { count: filteredData.length })}
      />

      {dialogEntry && (
        <EditEntryDialog
          entry={dialogEntry}
          filePath={dialogEntry._filePath}
          open={!!dialogEntry}
          onOpenChange={(isOpen) => {
            if (!isOpen) setDialogEntry(null);
          }}
        />
      )}
    </div>
  );
}
