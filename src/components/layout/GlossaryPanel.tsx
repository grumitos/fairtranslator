import { useCallback, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { Search } from "lucide-react";
import { toast } from "sonner";
import { SheetBody } from "@/components/ui/sheet";
import { useGlossary } from "@/hooks/useGlossary";
import { GlossaryAddForm } from "@/components/layout/glossary/GlossaryAddForm";
import { GlossaryTermRow } from "@/components/layout/glossary/GlossaryTermRow";
import type { GlossaryTerm } from "@/types/domain";
import { confirmAction } from "@/components/common/ConfirmDialog";

function getGlossaryErrorMessage(error: unknown, duplicateMessage: string) {
  const message = error instanceof Error ? error.message : String(error);
  return message.includes("already exists") || message.includes("ya existe")
    ? duplicateMessage
    : message;
}

// Pestaña «Glosario» del panel de contexto: alta en una fila, filtro y lista plana de términos.
export function GlossaryEditor() {
  const { t } = useTranslation("glossary");
  const { glossary, add, remove, update, isLoading } = useGlossary();
  const [newSource, setNewSource] = useState("");
  const [newTarget, setNewTarget] = useState("");
  const [newCaseSensitive, setNewCaseSensitive] = useState(true);
  const [editingSource, setEditingSource] = useState<string | null>(null);
  const [editTarget, setEditTarget] = useState("");
  const [editCaseSensitive, setEditCaseSensitive] = useState(true);
  const [filter, setFilter] = useState("");

  const visibleTerms = useMemo(() => {
    const query = filter.trim().toLowerCase();
    if (!query) return glossary;
    return glossary.filter(
      (term) => term.source.toLowerCase().includes(query) || term.target.toLowerCase().includes(query),
    );
  }, [filter, glossary]);

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
      const confirmed = await confirmAction({
        message: t("confirmDelete", { term: source }),
        confirmLabel: t("remove"),
        destructive: true,
      });
      if (!confirmed) return;
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
    <SheetBody className="gap-3 pt-4">
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

      <label className="flex h-8 items-center gap-2 border-b border-line px-1">
        <Search aria-hidden="true" className="size-3.5 shrink-0 text-ink-3" strokeWidth={1.8} />
        <input
          type="search"
          value={filter}
          onChange={(event) => setFilter(event.target.value)}
          placeholder={t("filter")}
          aria-label={t("filter")}
          className="h-7 min-w-0 flex-1 bg-transparent text-sm text-ink outline-none placeholder:text-ink-4"
        />
        <span className="font-mono text-xs text-ink-4">{t("totalTerms", { count: glossary.length })}</span>
      </label>

      {glossary.length === 0 ? (
        <p className="ft-empty-state text-sm">{t("empty")}</p>
      ) : (
        <ul className="flex flex-col" aria-label={t("title")}>
          {visibleTerms.map((term) => (
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
        </ul>
      )}
    </SheetBody>
  );
}
