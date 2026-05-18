import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useProject } from "@/hooks/useProject";
import type { TranslationEntry, EntryStatus } from "@/types/domain";

interface EditEntryDialogProps {
  entry: TranslationEntry;
  filePath: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function EditEntryDialog({
  entry,
  filePath,
  open,
  onOpenChange,
}: EditEntryDialogProps) {
  const { t } = useTranslation(["editor", "common"]);
  const { updateEntry } = useProject();
  const [translation, setTranslation] = useState(entry.translation);
  const [status, setStatus] = useState<EntryStatus>(entry.status);
  const [notes, setNotes] = useState(entry.notes);
  const [isSaving, setIsSaving] = useState(false);

  useEffect(() => {
    setTranslation(entry.translation);
    setStatus(entry.status);
    setNotes(entry.notes);
  }, [entry]);

  const handleSave = async () => {
    setIsSaving(true);
    try {
      await updateEntry(filePath, entry.id, translation, status, notes);
      onOpenChange(false);
    } catch {
      // Error handled by useProject
    } finally {
      setIsSaving(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-2xl max-h-[80vh] overflow-auto">
        <DialogHeader>
          <DialogTitle className="text-sm font-medium">
            {t("editor:grid.editTranslation")}
          </DialogTitle>
          <DialogDescription className="text-xs text-muted-foreground">
            {entry.fileContext}
          </DialogDescription>
        </DialogHeader>

        <div className="flex flex-col gap-4 py-2">
          {/* Original text */}
          <div className="flex flex-col gap-1.5">
            <label id="edit-entry-original-label" className="text-xs font-medium text-muted-foreground">
              {t("editor:grid.original")}
            </label>
            <div
              className="rounded-md border bg-muted/30 p-3 text-sm whitespace-pre-wrap select-text"
              aria-labelledby="edit-entry-original-label"
            >
              {entry.original}
            </div>
          </div>

          {/* Translation textarea */}
          <div className="flex flex-col gap-1.5">
            <label htmlFor="edit-entry-translation" className="text-xs font-medium text-muted-foreground">
              {t("editor:grid.translation")}
            </label>
            <Textarea
              id="edit-entry-translation"
              name="entry-translation"
              value={translation}
              onChange={(e) => setTranslation(e.target.value)}
              rows={Math.max(3, entry.original.split("\n").length)}
              className="resize-y text-sm select-text"
            />
          </div>

          {/* Status + notes row */}
          <div className="grid grid-cols-2 gap-3">
            <div className="flex flex-col gap-1.5">
              <label id="edit-entry-status-label" className="text-xs font-medium text-muted-foreground">
                {t("editor:grid.status")}
              </label>
              <Select
                value={status}
                onValueChange={(val) => {
                  const valid: EntryStatus[] = ["untranslated", "draft", "reviewed", "final"];
                  if (valid.includes(val as EntryStatus)) {
                    setStatus(val as EntryStatus);
                  }
                }}
              >
                <SelectTrigger className="h-8 text-sm" aria-labelledby="edit-entry-status-label">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="untranslated">
                    {t("editor:status.untranslated")}
                  </SelectItem>
                  <SelectItem value="draft">{t("editor:status.draft")}</SelectItem>
                  <SelectItem value="reviewed">
                    {t("editor:status.reviewed")}
                  </SelectItem>
                  <SelectItem value="final">{t("editor:status.final")}</SelectItem>
                </SelectContent>
              </Select>
            </div>

            <div className="flex flex-col gap-1.5">
              <label htmlFor="edit-entry-notes" className="text-xs font-medium text-muted-foreground">
                {t("editor:grid.notes")}
              </label>
              <Textarea
                id="edit-entry-notes"
                name="entry-notes"
                value={notes}
                onChange={(e) => setNotes(e.target.value)}
                rows={1}
                className="resize-y text-sm"
                placeholder={t("editor:grid.notesPlaceholder")}
              />
            </div>
          </div>
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            {t("common:cancel")}
          </Button>
          <Button onClick={handleSave} disabled={isSaving}>
            {t("common:save")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
