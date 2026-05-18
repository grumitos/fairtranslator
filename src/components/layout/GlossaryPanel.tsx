import { type CSSProperties, useCallback, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from "@/components/ui/sheet";
import { useGlossary } from "@/hooks/useGlossary";
import { GlossaryAddForm } from "@/components/layout/glossary/GlossaryAddForm";
import { GlossaryTermRow } from "@/components/layout/glossary/GlossaryTermRow";
import type { GlossaryTerm } from "@/types/domain";

interface GlossaryPanelProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

function getGlossaryErrorMessage(error: unknown, duplicateMessage: string) {
  const message = error instanceof Error ? error.message : String(error);
  return message.includes("already exists") || message.includes("ya existe")
    ? duplicateMessage
    : message;
}

export function GlossaryPanel({ open, onOpenChange }: GlossaryPanelProps) {
  const { t } = useTranslation("glossary");
  const { glossary, add, remove, update, isLoading } = useGlossary();
  const [newSource, setNewSource] = useState("");
  const [newTarget, setNewTarget] = useState("");
  const [newCaseSensitive, setNewCaseSensitive] = useState(true);
  const [editingSource, setEditingSource] = useState<string | null>(null);
  const [editTarget, setEditTarget] = useState("");
  const [editCaseSensitive, setEditCaseSensitive] = useState(true);

  const handleAdd = useCallback(async () => {
    if (!newSource.trim() || !newTarget.trim()) {
      return;
    }

    try {
      await add(newSource.trim(), newTarget.trim(), newCaseSensitive);
      setNewSource("");
      setNewTarget("");
      setNewCaseSensitive(true);
      toast.success(t("addSuccess"));
    } catch (e) {
      toast.error(t("addFailed"), {
        description: getGlossaryErrorMessage(e, t("duplicate")),
      });
    }
  }, [newSource, newTarget, newCaseSensitive, add, t]);

  const handleRemove = useCallback(
    async (source: string) => {
      try {
        await remove(source);
        toast.success(t("removeSuccess"));
      } catch (e) {
        toast.error(t("removeFailed"), {
          description: e instanceof Error ? e.message : String(e),
        });
      }
    },
    [remove, t],
  );

  const handleStartEdit = useCallback((term: GlossaryTerm) => {
    setEditingSource(term.source);
    setEditTarget(term.target);
    setEditCaseSensitive(term.caseSensitive);
  }, []);

  const handleUpdate = useCallback(async () => {
    if (!editingSource || !editTarget.trim()) {
      return;
    }

    try {
      await update(editingSource, editTarget.trim(), editCaseSensitive);
      setEditingSource(null);
      toast.success(t("updateSuccess"));
    } catch (e) {
      toast.error(t("updateFailed"), {
        description: e instanceof Error ? e.message : String(e),
      });
    }
  }, [editingSource, editTarget, editCaseSensitive, update, t]);

  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent style={{ "--sheet-width": "480px" } as CSSProperties}>
        <SheetHeader>
          <SheetTitle>{t("title")}</SheetTitle>
          <SheetDescription className="sr-only">{t("description")}</SheetDescription>
        </SheetHeader>

        <div className="space-y-4 px-4 pb-4">
          <GlossaryAddForm
            source={newSource}
            target={newTarget}
            caseSensitive={newCaseSensitive}
            isLoading={isLoading}
            onSourceChange={setNewSource}
            onTargetChange={setNewTarget}
            onCaseSensitiveChange={setNewCaseSensitive}
            onAdd={handleAdd}
          />

          <p className="text-xs text-muted-foreground">
            {t("totalTerms", { count: glossary.length })}
          </p>

          <ScrollArea className="h-[calc(100vh-320px)]">
            {glossary.length === 0 ? (
              <p className="py-8 text-center text-sm text-muted-foreground">
                {t("empty")}
              </p>
            ) : (
              <div className="space-y-2">
                {glossary.map((term) => (
                  <GlossaryTermRow
                    key={term.source}
                    term={term}
                    isEditing={editingSource === term.source}
                    editTarget={editTarget}
                    editCaseSensitive={editCaseSensitive}
                    isLoading={isLoading}
                    onEditTargetChange={setEditTarget}
                    onEditCaseSensitiveChange={setEditCaseSensitive}
                    onStartEdit={handleStartEdit}
                    onCancelEdit={() => setEditingSource(null)}
                    onUpdate={handleUpdate}
                    onRemove={handleRemove}
                  />
                ))}
              </div>
            )}
          </ScrollArea>
        </div>
      </SheetContent>
    </Sheet>
  );
}
