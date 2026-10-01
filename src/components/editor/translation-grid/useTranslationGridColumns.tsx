import { useMemo, type CSSProperties } from "react";
import { useTranslation } from "react-i18next";
import { createColumnHelper } from "@tanstack/react-table";
import { StatusGlyph } from "@/components/common/StatusGlyph";
import { ControlCodeText } from "@/components/editor/ControlCodeText";
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
        size: 190,
        minSize: 80,
        maxSize: 500,
        cell: (info) => (
          <span className="block truncate pt-px font-mono text-xs text-ink-2" title={info.getValue()}>
            {info.getValue()}
          </span>
        ),
      }),
      columnHelper.accessor("original", {
        header: () => t("grid.original"),
        size: 340,
        minSize: 150,
        maxSize: 900,
        cell: (info) => {
          const shouldHighlight = findRegex && (searchTarget === "original" || searchTarget === "both");
          return (
            <div style={wrapStyle}>
              <ControlCodeText
                text={info.getValue()}
                regex={shouldHighlight ? findRegex : null}
                className="ft-cjk text-sm leading-5 break-words whitespace-pre-wrap select-text"
              />
            </div>
          );
        },
      }),
      columnHelper.accessor("translation", {
        header: () => t("grid.translation"),
        size: 340,
        minSize: 150,
        maxSize: 900,
        cell: (info) => {
          const meta = info.table.options.meta as TranslationGridMeta | undefined;
          const isEditing = meta?.inlineEditingEntryId === info.row.original.id;

          if (isEditing && meta) {
            return (
              <InlineEditCell
                entry={info.row.original}
                initialValue={info.getValue()}
                wrapStyle={wrapStyle}
                ariaLabel={t("grid.translation")}
                onCommit={meta.onCommitInlineEdit}
                onCancel={meta.onCancelInlineEdit}
                onNavigate={(dir) => meta.onNavigateInlineEdit(dir, info.row.index)}
                onOpenDialog={meta.onOpenDialog}
              />
            );
          }

          const shouldHighlight = findRegex && (searchTarget === "translation" || searchTarget === "both");
          return (
            <div style={wrapStyle}>
              <ControlCodeText
                text={info.getValue()}
                regex={shouldHighlight ? findRegex : null}
                className="text-sm leading-5 break-words whitespace-pre-wrap select-text"
              />
            </div>
          );
        },
      }),
      columnHelper.accessor("status", {
        header: () => t("grid.status"),
        size: 130,
        minSize: 90,
        maxSize: 220,
        cell: (info) => {
          const status = info.getValue();
          return (
            <span className="flex h-5 items-center gap-2 text-[12.5px] whitespace-nowrap text-ink-2">
              <StatusGlyph status={status} />
              <span className={status === "untranslated" ? "text-ink-3" : undefined}>{t(`status.${status}`)}</span>
            </span>
          );
        },
      }),
    ],
    [t, findRegex, searchTarget, wrapStyle],
  );
}
