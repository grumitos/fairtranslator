import { useCallback, useRef, type RefObject, type UIEvent } from "react";
import { flexRender, type Column, type Row, type Table } from "@tanstack/react-table";
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

// Original y traducción reparten el ancho disponible (sin bajar de su mínimo); el resto es fijo.
const FLEX_COLUMNS = new Set(["original", "translation"]);

function columnStyle(column: Column<EntryWithPath>) {
  const size = column.getSize();
  if (FLEX_COLUMNS.has(column.id)) {
    return { flex: `1 1 ${size}px`, minWidth: column.columnDef.minSize ?? 150 };
  }
  return { flex: "none", width: size, minWidth: size };
}

// Tabla densa del kit: cabecera baja, filas separadas por líneas suaves, sin cebra ni bordes verticales.
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
  const headerInnerRef = useRef<HTMLDivElement>(null);
  const lastScrollLeftRef = useRef(0);
  const minGridWidth = table
    .getVisibleLeafColumns()
    .reduce((sum, column) => sum + (FLEX_COLUMNS.has(column.id) ? (column.columnDef.minSize ?? 150) : column.getSize()), 0);

  const handleRowsScroll = useCallback((event: UIEvent<HTMLDivElement>) => {
    const scrollLeft = event.currentTarget.scrollLeft;
    if (scrollLeft === lastScrollLeftRef.current) {
      return;
    }

    lastScrollLeftRef.current = scrollLeft;
    if (headerInnerRef.current) {
      headerInnerRef.current.style.transform = `translateX(${-scrollLeft}px)`;
    }
  }, []);

  return (
    <>
      <div className="sticky top-0 z-10 shrink-0 overflow-hidden border-b border-line bg-surface-0" role="rowgroup">
        <div
          ref={headerInnerRef}
          data-ft-grid-row="header"
          className="flex h-[34px] text-xs font-medium text-ink-3 will-change-transform"
          style={{ width: "100%", minWidth: minGridWidth }}
          role="row"
        >
          {table.getHeaderGroups().map((headerGroup) =>
            headerGroup.headers.map((header) => (
              <div
                key={header.id}
                data-ft-grid-column={header.column.id}
                className="relative flex items-center px-3 select-none first:pl-6 last:pr-6"
                style={columnStyle(header.column)}
                role="columnheader"
              >
                {flexRender(header.column.columnDef.header, header.getContext())}

                {header.column.getCanResize() && (
                  <div
                    onMouseDown={header.getResizeHandler()}
                    onTouchStart={header.getResizeHandler()}
                    className={cn(
                      "absolute top-1.5 right-0 bottom-1.5 w-1.5 cursor-col-resize touch-none select-none",
                      "after:absolute after:top-0 after:right-0 after:h-full after:w-px after:bg-line after:transition-colors after:duration-(--motion-fast) hover:after:bg-line-chip",
                      header.column.getIsResizing() && "after:bg-ink",
                    )}
                  />
                )}
              </div>
            )),
          )}
        </div>
      </div>

      <div
        ref={parentRef}
        className="min-h-0 flex-1 overflow-auto bg-surface-0"
        onScroll={handleRowsScroll}
        role="rowgroup"
      >
        {rows.length === 0 ? (
          <div className="flex h-full items-center justify-center p-6">
            <div className="ft-empty-state" role="status">
              <p className="text-sm text-ink-2">{emptyText}</p>
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
                  id={`translation-grid-row-${row.id}`}
                  data-index={virtualRow.index}
                  data-ft-grid-row="body"
                  ref={measureElement}
                  className={cn(
                    "absolute left-0 flex w-full cursor-default items-stretch border-b border-line-soft transition-colors duration-(--motion-fast)",
                    isSelected
                      ? "bg-surface-3"
                      : isMatch
                        ? "bg-[rgb(237_237_239/0.035)] hover:bg-surface-1"
                        : "hover:bg-surface-1",
                    isEditing && "bg-surface-2",
                  )}
                  style={{
                    minHeight: "40px",
                    width: "100%",
                    minWidth: minGridWidth,
                    transform: `translateY(${virtualRow.start}px)`,
                  }}
                  onClick={() => meta?.onSelectRow?.(virtualRow.index)}
                  onDoubleClick={() => onRowDoubleClick(row.original)}
                  role="row"
                  aria-rowindex={virtualRow.index + 2}
                  aria-selected={isSelected}
                >
                  {row.getVisibleCells().map((cell) => {
                    const isTranslationCol = cell.column.id === "translation";

                    return (
                      <div
                        key={cell.id}
                        data-ft-grid-column={cell.column.id}
                        className={cn(
                          "flex items-start px-3 py-[11px] leading-5 first:pl-6 last:pr-6",
                          isTranslationCol ? "cursor-text overflow-visible" : "overflow-hidden",
                          isTranslationCol && isEditing && "py-[7px]",
                        )}
                        style={columnStyle(cell.column)}
                        onClick={isTranslationCol && !isEditing ? (e) => {
                          e.stopPropagation();
                          meta?.onSelectRow?.(virtualRow.index);
                          meta?.onStartInlineEdit?.(row.original.id);
                        } : undefined}
                        onDoubleClick={isTranslationCol ? (e) => {
                          e.stopPropagation();
                          meta?.onOpenDialog?.(row.original);
                        } : undefined}
                        role="gridcell"
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

      <div className="sr-only" aria-live="polite">
        {totalEntriesText}
      </div>
    </>
  );
}
