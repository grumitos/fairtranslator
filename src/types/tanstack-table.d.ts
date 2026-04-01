import "@tanstack/react-table";
import type { EntryStatus } from "@/types/domain";
import type { EntryWithPath } from "@/components/editor/translation-grid/types";

declare module "@tanstack/react-table" {
  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  interface TableMeta<TData extends RowData> {
    inlineEditingEntryId?: string | null;
    selectedRowIndex?: number | null;
    onStartInlineEdit?: (entryId: string) => void;
    onCommitInlineEdit?: (entryId: string, filePath: string, value: string, currentStatus: EntryStatus) => void;
    onCancelInlineEdit?: () => void;
    onNavigateInlineEdit?: (direction: "next" | "prev", currentRowIndex: number) => void;
    onOpenDialog?: (entry: EntryWithPath) => void;
  }
}
