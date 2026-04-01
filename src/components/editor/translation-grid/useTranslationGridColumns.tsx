import { useMemo, type CSSProperties } from "react";
import { useTranslation } from "react-i18next";
import { createColumnHelper } from "@tanstack/react-table";
import { cn } from "@/lib/utils";
import { StatusBadge } from "@/components/editor/StatusBadge";
import { HighlightedText } from "@/components/editor/HighlightedText";
import { InlineEditCell } from "@/components/editor/translation-grid/InlineEditCell";
import type { EntryWithPath, TranslationGridMeta } from "@/components/editor/translation-grid/types";

const columnHelper = createColumnHelper<EntryWithPath>();

export function useTranslationGridColumns(
  findRegex: RegExp | null,
  searchTarget: "original" | "translation" | "both",
  wrapStyle: CSSProperties | undefined,
) {
  const { t } = useTranslation("editor");

  return useMemo(
    () => [
      columnHelper.accessor("fileContext", {
        header: () => t("grid.context"),
        size: 200,
        minSize: 80,
        maxSize: 500,
        cell: (info) => (
          <span className="block truncate text-xs text-muted-foreground" title={info.getValue()}>
            {info.getValue()}
          </span>
        ),
      }),
      columnHelper.accessor("original", {
        header: () => t("grid.original"),
        size: 300,
        minSize: 150,
        maxSize: 800,
        cell: (info) => {
          const shouldHighlight = findRegex && (searchTarget === "original" || searchTarget === "both");
          return (
            <div style={wrapStyle}>
              <HighlightedText
                text={info.getValue()}
                regex={shouldHighlight ? findRegex : null}
                className="select-text whitespace-pre-wrap break-words text-sm"
              />
            </div>
          );
        },
      }),
      columnHelper.accessor("translation", {
        header: () => t("grid.translation"),
        size: 300,
        minSize: 150,
        maxSize: 800,
        cell: (info) => {
          const meta = info.table.options.meta as TranslationGridMeta | undefined;
          const isEditing = meta?.inlineEditingEntryId === info.row.original.id;

          if (isEditing && meta) {
            return (
              <InlineEditCell
                entry={info.row.original}
                initialValue={info.getValue()}
                wrapStyle={wrapStyle}
                onCommit={meta.onCommitInlineEdit}
                onCancel={meta.onCancelInlineEdit}
                onNavigate={(dir) => meta.onNavigateInlineEdit(dir, info.row.index)}
                onOpenDialog={meta.onOpenDialog}
              />
            );
          }

          const value = info.getValue();
          const shouldHighlight =
            findRegex && (searchTarget === "translation" || searchTarget === "both");
          return (
            <div style={wrapStyle}>
              <HighlightedText
                text={value}
                regex={shouldHighlight ? findRegex : null}
                className={cn(
                  "select-text whitespace-pre-wrap break-words text-sm",
                  !value && "italic text-muted-foreground",
                )}
              />
            </div>
          );
        },
      }),
      columnHelper.accessor("status", {
        header: () => t("grid.status"),
        size: 100,
        minSize: 70,
        maxSize: 200,
        cell: (info) => <StatusBadge status={info.getValue()} />,
      }),
    ],
    [t, findRegex, searchTarget, wrapStyle],
  );
}
