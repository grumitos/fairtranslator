import type { TranslationEntry, EntryStatus } from "@/types/domain";

export type EntryWithPath = TranslationEntry & { _filePath: string };

export interface TranslationGridMeta {
  inlineEditingEntryId: string | null;
  selectedRowIndex: number | null;
  onStartInlineEdit: (entryId: string) => void;
  onCommitInlineEdit: (entryId: string, filePath: string, value: string, currentStatus: EntryStatus) => void;
  onCancelInlineEdit: () => void;
  onNavigateInlineEdit: (direction: "next" | "prev", currentRowIndex: number) => void;
  onOpenDialog: (entry: EntryWithPath) => void;
}
