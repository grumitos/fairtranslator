import { useCallback, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import type { Row } from "@tanstack/react-table";
import type { Virtualizer } from "@tanstack/react-virtual";
import { useProject } from "@/hooks/useProject";
import type { EntryStatus } from "@/types/domain";
import type { EntryWithPath } from "@/components/editor/translation-grid/types";

interface UseInlineEditOptions {
  virtualizer: Virtualizer<HTMLDivElement, Element>;
}

export function useInlineEdit({ virtualizer }: UseInlineEditOptions) {
  const { t } = useTranslation("common");
  const { updateEntry } = useProject();
  const [inlineEditingEntryId, setInlineEditingEntryId] = useState<string | null>(null);
  const [pendingEditRowIndex, setPendingEditRowIndex] = useState<number | null>(null);

  const startEdit = useCallback((entryId: string) => {
    setInlineEditingEntryId(entryId);
  }, []);

  const commitEdit = useCallback(async (
    entryId: string,
    filePath: string,
    value: string,
    currentStatus: EntryStatus,
  ) => {
    const newStatus = currentStatus === "untranslated" && value.trim()
      ? "draft"
      : currentStatus;
    setInlineEditingEntryId(null);
    try {
      await updateEntry(filePath, entryId, value, newStatus);
    } catch (error) {
      toast.error(t("error"), {
        description: error instanceof Error ? error.message : String(error),
      });
    }
  }, [t, updateEntry]);

  const cancelEdit = useCallback(() => {
    setInlineEditingEntryId(null);
  }, []);

  const navigateEdit = useCallback((
    direction: "next" | "prev",
    rows: Row<EntryWithPath>[],
    currentRowIndex: number,
  ) => {
    const nextIndex = direction === "next" ? currentRowIndex + 1 : currentRowIndex - 1;
    if (nextIndex < 0 || nextIndex >= rows.length) {
      setInlineEditingEntryId(null);
      return;
    }

    const targetEntry = rows[nextIndex]?.original;
    if (!targetEntry) {
      setInlineEditingEntryId(null);
      return;
    }

    // Scroll to the target row and set it for editing
    virtualizer.scrollToIndex(nextIndex, { align: "auto" });
    setInlineEditingEntryId(targetEntry.id);
    setPendingEditRowIndex(nextIndex);
  }, [virtualizer]);

  return {
    inlineEditingEntryId,
    pendingEditRowIndex,
    startEdit,
    commitEdit,
    cancelEdit,
    navigateEdit,
    setInlineEditingEntryId,
    setPendingEditRowIndex,
  };
}
