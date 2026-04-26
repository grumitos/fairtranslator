import type { RefObject } from "react";
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

  return (
    <>
      {/* Header with resize handles */}
      <div className="sticky top-0 z-10 flex h-9 border-b bg-card/95 text-[11px] font-semibold uppercase tracking-normal text-muted-foreground shadow-xs">
        {table.getHeaderGroups().map((headerGroup) =>
          headerGroup.headers.map((header) => (
            <div
              key={header.id}
              className="relative flex items-center border-r px-3 last:border-r-0 select-none"
              style={{ width: header.getSize(), minWidth: header.getSize() }}
            >
              {flexRender(header.column.columnDef.header, header.getContext())}

              {/* Column resize handle */}
              {header.column.getCanResize() && (
                <div
                  onMouseDown={header.getResizeHandler()}
                  onTouchStart={header.getResizeHandler()}
                  className={cn(
                    "absolute right-0 top-0 h-full w-1.5 cursor-col-resize select-none touch-none transition-colors duration-150",
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
      <div ref={parentRef} className="min-h-0 flex-1 overflow-auto bg-background/40">
        {rows.length === 0 ? (
          <div className="flex h-full items-center justify-center p-6">
            <div className="rounded-md border bg-card px-5 py-4 text-center shadow-sm">
              <p className="text-sm font-medium text-foreground">{emptyText}</p>
              <p className="mt-1 text-xs text-muted-foreground">{totalEntriesText}</p>
            </div>
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
                    "absolute left-0 flex w-full cursor-pointer items-stretch border-b transition-[background-color,box-shadow] duration-150 hover:bg-accent/60",
                    isMatch
                      ? "bg-warning/10"
                      : isSelected
                        ? "bg-accent/55"
                        : virtualRow.index % 2 === 0
                          ? "bg-card"
                          : "bg-muted/20",
                    isSelected && "shadow-[inset_2px_0_0_var(--primary)]",
                    isEditing && "shadow-[inset_2px_0_0_var(--primary),inset_0_0_0_1px_var(--primary)]",
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
                          meta?.onStartInlineEdit?.(row.original.id);
                        } : undefined}
                        onDoubleClick={isTranslationCol ? (e) => {
                          e.stopPropagation();
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
      <div className="flex h-7 items-center border-t bg-card/95 px-3 text-xs text-muted-foreground">
        {totalEntriesText}
      </div>
    </>
  );
}
