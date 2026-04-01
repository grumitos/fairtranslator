import { useRef, type RefObject } from "react";
import { flexRender, type Row, type Table } from "@tanstack/react-table";
import { cn } from "@/lib/utils";
import type { EntryWithPath, TranslationGridMeta } from "@/components/editor/translation-grid/types";

interface VirtualItem {
  key: string | number | bigint;
  index: number;
  size: number;
  start: number;
}

interface TranslationGridTableViewProps {
  table: Table<EntryWithPath>;
  rows: Row<EntryWithPath>[];
  parentRef: RefObject<HTMLDivElement | null>;
  virtualItems: VirtualItem[];
  totalSize: number;
  matchedEntryIds: Set<string>;
  measureElement: (el: HTMLElement | null) => void;
  onRowDoubleClick: (entry: EntryWithPath) => void;
  emptyText: string;
  totalEntriesText: string;
}

export function TranslationGridTableView({
  table,
  rows,
  parentRef,
  virtualItems,
  totalSize,
  matchedEntryIds,
  measureElement,
  onRowDoubleClick,
  emptyText,
  totalEntriesText,
}: TranslationGridTableViewProps) {
  const meta = table.options.meta as TranslationGridMeta | undefined;
  const clickTimerRef = useRef<ReturnType<typeof setTimeout>>(undefined);

  return (
    <>
      {/* Header with resize handles */}
      <div className="flex border-b bg-muted/30 text-xs font-medium text-muted-foreground">
        {table.getHeaderGroups().map((headerGroup) =>
          headerGroup.headers.map((header) => (
            <div
              key={header.id}
              className="relative flex items-center border-r px-3 py-2 last:border-r-0 select-none"
              style={{ width: header.getSize(), minWidth: header.getSize() }}
            >
              {flexRender(header.column.columnDef.header, header.getContext())}

              {/* Column resize handle */}
              {header.column.getCanResize() && (
                <div
                  onMouseDown={header.getResizeHandler()}
                  onTouchStart={header.getResizeHandler()}
                  className={cn(
                    "absolute right-0 top-0 h-full w-1.5 cursor-col-resize select-none touch-none",
                    "hover:bg-primary/40 active:bg-primary/60",
                    header.column.getIsResizing() && "bg-primary",
                  )}
                />
              )}
            </div>
          )),
        )}
      </div>

      {/* Virtualized rows */}
      <div ref={parentRef} className="flex-1 overflow-auto">
        {rows.length === 0 ? (
          <div className="flex h-full items-center justify-center text-sm text-muted-foreground">
            {emptyText}
          </div>
        ) : (
          <div style={{ height: totalSize, position: "relative" }}>
            {virtualItems.map((virtualRow) => {
              const row = rows[virtualRow.index];
              const isMatch = matchedEntryIds.has(row.original.id);
              const isSelected = meta?.selectedRowIndex === virtualRow.index;
              const isEditing = meta?.inlineEditingEntryId === row.original.id;

              return (
                <div
                  key={virtualRow.key}
                  data-index={virtualRow.index}
                  ref={measureElement}
                  className={cn(
                    "absolute left-0 flex w-full cursor-pointer items-stretch border-b transition-colors hover:bg-accent/50",
                    isMatch
                      ? "bg-yellow-50 dark:bg-yellow-950/30"
                      : isSelected
                        ? "bg-accent/30"
                        : virtualRow.index % 2 === 0
                          ? "bg-background"
                          : "bg-muted/20",
                    isSelected && "ring-2 ring-inset ring-primary/50",
                    isEditing && "ring-2 ring-inset ring-primary",
                  )}
                  style={{
                    minHeight: "36px",
                    transform: `translateY(${virtualRow.start}px)`,
                  }}
                  onDoubleClick={() => onRowDoubleClick(row.original)}
                >
                  {row.getVisibleCells().map((cell) => {
                    const isTranslationCol = cell.column.id === "translation";

                    return (
                      <div
                        key={cell.id}
                        className={cn(
                          "flex items-center border-r px-3 py-1.5 last:border-r-0",
                          isTranslationCol ? "overflow-visible" : "overflow-hidden",
                        )}
                        style={{ width: cell.column.getSize(), minWidth: cell.column.getSize() }}
                        onClick={isTranslationCol && !isEditing ? (e) => {
                          e.stopPropagation();
                          clearTimeout(clickTimerRef.current);
                          clickTimerRef.current = setTimeout(() => {
                            meta?.onStartInlineEdit?.(row.original.id);
                          }, 200);
                        } : undefined}
                        onDoubleClick={isTranslationCol ? (e) => {
                          e.stopPropagation();
                          clearTimeout(clickTimerRef.current);
                          meta?.onOpenDialog?.(row.original);
                        } : undefined}
                      >
                        {flexRender(cell.column.columnDef.cell, cell.getContext())}
                      </div>
                    );
                  })}
                </div>
              );
            })}
          </div>
        )}
      </div>

      {/* Footer */}
      <div className="flex items-center border-t px-3 py-1 text-xs text-muted-foreground">
        {totalEntriesText}
      </div>
    </>
  );
}
