import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import {
  Sheet,
  SheetBody,
  SheetContent,
  SheetDescription,
  SheetFooter,
  SheetHeader,
  SheetTitle,
} from "@/components/ui/sheet";
import { Button } from "@/components/ui/button";
import { Kbd } from "@/components/common/Kbd";
import { StatusGlyph } from "@/components/common/StatusGlyph";
import { Textarea } from "@/components/ui/textarea";
import { ControlCodeText } from "@/components/editor/ControlCodeText";
import { splitSpecialTokens } from "@/components/editor/controlCodes";
import { useProject } from "@/hooks/useProject";
import { statusAfterEdit } from "@/lib/translationState";
import { cn } from "@/lib/utils";
import type { TranslationEntry, EntryStatus } from "@/types/domain";
import { confirmAction } from "@/components/common/ConfirmDialog";

interface EditEntryDialogProps {
  entry: TranslationEntry;
  filePath: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

const STATUSES: EntryStatus[] = ["untranslated", "draft", "reviewed", "final"];

// Cuántos códigos del original siguen en la traducción (como multiconjunto: \C[2] dos veces cuenta dos).
function preservedCodes(original: string, translation: string) {
  const expected = splitSpecialTokens(original).filter((segment) => segment.kind === "code");
  const available = new Map<string, number>();
  for (const segment of splitSpecialTokens(translation)) {
    if (segment.kind === "code") available.set(segment.text, (available.get(segment.text) ?? 0) + 1);
  }
  let kept = 0;
  for (const segment of expected) {
    const count = available.get(segment.text) ?? 0;
    if (count > 0) {
      kept += 1;
      available.set(segment.text, count - 1);
    }
  }
  return { kept, total: expected.length };
}

/*
 * Edición completa de una entrada en el panel lateral del kit: original con sus códigos, traducción,
 * estado y notas. Ctrl+Enter guarda; cerrar con cambios pide confirmación.
 */
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
  const hasUnsavedChanges =
    translation !== entry.translation || status !== entry.status || notes !== entry.notes;
  const codes = useMemo(() => preservedCodes(entry.original, translation), [entry.original, translation]);

  useEffect(() => {
    setTranslation(entry.translation);
    setStatus(entry.status);
    setNotes(entry.notes);
  }, [entry]);

  const handleSave = async () => {
    setIsSaving(true);
    try {
      // La misma regla que la edición en línea y Buscar y reemplazar: la primera traducción promueve una
      // entrada sin traducir a borrador. El exportador omite las entradas sin traducir aunque tengan texto.
      const nextStatus =
        entry.status === "untranslated" ? statusAfterEdit(status, translation) : status;
      await updateEntry(filePath, entry.id, translation, nextStatus, notes);
      onOpenChange(false);
    } catch (error) {
      toast.error(t("common:error"), {
        description: error instanceof Error ? error.message : String(error),
      });
    } finally {
      setIsSaving(false);
    }
  };

  const handleOpenChange = (nextOpen: boolean) => {
    if (!nextOpen && hasUnsavedChanges) {
      void confirmAction({
        message: t("editor:grid.discardChanges"),
        confirmLabel: t("common:discard"),
        destructive: true,
      }).then((confirmed) => {
        if (confirmed) onOpenChange(false);
      });
      return;
    }
    onOpenChange(nextOpen);
  };

  return (
    <Sheet open={open} onOpenChange={handleOpenChange}>
      <SheetContent
        onKeyDown={(event) => {
          if ((event.ctrlKey || event.metaKey) && event.key === "Enter" && !isSaving) {
            event.preventDefault();
            void handleSave();
          }
        }}
      >
        <SheetHeader>
          <SheetTitle>{t("editor:grid.editTranslation")}</SheetTitle>
          <SheetDescription className="truncate font-mono" title={entry.fileContext}>
            {entry.fileContext}
          </SheetDescription>
        </SheetHeader>

        <SheetBody>
          <div className="flex flex-col gap-2">
            <span className="text-xs text-ink-3">
              {t("editor:grid.original")}
            </span>
            <p className="m-0 text-[14px] leading-[1.65] whitespace-pre-wrap select-text">
              <ControlCodeText text={entry.original} className="ft-cjk" />
            </p>
          </div>

          <div className="flex flex-col gap-2">
            <label htmlFor="edit-entry-translation" className="text-xs text-ink-3">
              {t("editor:grid.translation")}
            </label>
            <Textarea
              id="edit-entry-translation"
              name="entry-translation"
              value={translation}
              onChange={(e) => setTranslation(e.target.value)}
              rows={Math.max(4, entry.original.split("\n").length + 1)}
              className="resize-y text-[14px] leading-[1.6] select-text"
            />
            <div className="flex items-center gap-4 text-xs text-ink-3">
              {codes.total > 0 && (
                <span className={cn("inline-flex items-center gap-1.5", codes.kept < codes.total && "text-warning")}>
                  {codes.kept === codes.total && <StatusGlyph status="final" />}
                  {t("editor:entry.codes")}
                  <span className="font-mono">
                    {codes.kept}/{codes.total}
                  </span>
                </span>
              )}
              <span>
                {t("editor:entry.characters")} <span className="font-mono text-ink-2">{Array.from(translation).length}</span>
              </span>
            </div>
          </div>

          <div className="flex flex-col gap-2">
            <span id="edit-entry-status-label" className="text-xs text-ink-3">
              {t("editor:grid.status")}
            </span>
            <div
              role="radiogroup"
              aria-labelledby="edit-entry-status-label"
              className="flex gap-0.5 rounded-[8px] border border-line-field bg-surface-0 p-0.5"
            >
              {STATUSES.map((option) => (
                <button
                  key={option}
                  type="button"
                  role="radio"
                  aria-checked={status === option}
                  onClick={() => setStatus(option)}
                  className={cn(
                    "inline-flex h-7 flex-1 items-center justify-center gap-1.5 rounded-md text-[12.5px] whitespace-nowrap outline-none transition-colors duration-(--motion-fast) focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ink",
                    status === option ? "bg-surface-3 text-ink" : "text-ink-3 hover:text-ink",
                  )}
                >
                  <StatusGlyph status={option} className="size-3" />
                  {t(`editor:status.${option}`)}
                </button>
              ))}
            </div>
          </div>

          <div className="flex flex-col gap-2">
            <label htmlFor="edit-entry-notes" className="text-xs text-ink-3">
              {t("editor:grid.notes")}
            </label>
            <Textarea
              id="edit-entry-notes"
              name="entry-notes"
              value={notes}
              onChange={(e) => setNotes(e.target.value)}
              rows={3}
              className="resize-y"
              placeholder="—"
            />
          </div>
        </SheetBody>

        <SheetFooter>
          <Button variant="outline" size="lg" onClick={() => handleOpenChange(false)}>
            {t("common:cancel")}
          </Button>
          <Button variant="brand" size="lg" onClick={handleSave} disabled={isSaving}>
            {t("common:save")}
            <Kbd className="border-brand-ink/25 text-brand-ink/70">Ctrl ↵</Kbd>
          </Button>
        </SheetFooter>
      </SheetContent>
    </Sheet>
  );
}
