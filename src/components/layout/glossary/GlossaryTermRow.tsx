import { Pencil, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";
import type { GlossaryTerm } from "@/types/domain";

interface GlossaryTermRowProps {
  term: GlossaryTerm;
  isEditing: boolean;
  editTarget: string;
  editCaseSensitive: boolean;
  isLoading: boolean;
  onEditTargetChange: (value: string) => void;
  onEditCaseSensitiveChange: (value: boolean) => void;
  onStartEdit: (term: GlossaryTerm) => void;
  onCancelEdit: () => void;
  onUpdate: () => void;
  onRemove: (source: string) => void;
}

// Fila plana del glosario: original, traducción y acciones que aparecen al pasar o enfocar.
export function GlossaryTermRow({
  term,
  isEditing,
  editTarget,
  editCaseSensitive,
  isLoading,
  onEditTargetChange,
  onEditCaseSensitiveChange,
  onStartEdit,
  onCancelEdit,
  onUpdate,
  onRemove,
}: GlossaryTermRowProps) {
  const { t } = useTranslation(["glossary", "common"]);

  if (isEditing) {
    return (
      <li className="flex flex-col gap-2 border-b border-line-soft py-2.5">
        <div className="grid grid-cols-[minmax(0,1fr)_minmax(0,1fr)_auto] items-center gap-2">
          <span className="ft-cjk truncate px-0.5 text-sm text-ink-2">{term.source}</span>
          <Input
            aria-label={t("glossary:target")}
            value={editTarget}
            onChange={(e) => onEditTargetChange(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") onUpdate();
              if (e.key === "Escape") onCancelEdit();
            }}
            autoFocus
          />
          <button
            type="button"
            aria-label={t("glossary:caseSensitive")}
            aria-pressed={editCaseSensitive}
            title={t("glossary:caseSensitive")}
            onClick={() => onEditCaseSensitiveChange(!editCaseSensitive)}
            className={cn(
              "h-8 min-w-8 rounded-md border px-1.5 font-mono text-xs outline-none focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ink",
              editCaseSensitive ? "border-line-chip bg-surface-3 text-ink" : "border-line-field text-ink-3",
            )}
          >
            Aa
          </button>
        </div>
        <div className="flex justify-end gap-2">
          <Button size="sm" variant="ghost" onClick={onCancelEdit}>
            {t("common:cancel")}
          </Button>
          <Button size="sm" onClick={onUpdate} disabled={isLoading}>
            {t("glossary:update")}
          </Button>
        </div>
      </li>
    );
  }

  return (
    <li className="group grid min-h-10 grid-cols-[minmax(0,0.9fr)_minmax(0,1.1fr)_auto] items-center gap-3 rounded-sm border-b border-line-soft px-1 hover:bg-surface-2">
      <span className="ft-cjk truncate text-sm text-ink-2" title={term.source}>
        {term.source}
      </span>
      <span className="flex min-w-0 items-center gap-2">
        <span className="truncate text-sm" title={term.target}>
          {term.target}
        </span>
        {term.caseSensitive && (
          <span
            className="shrink-0 font-mono text-[10.5px] text-ink-4"
            aria-label={t("glossary:caseSensitive")}
            title={t("glossary:caseSensitive")}
          >
            Aa
          </span>
        )}
      </span>
      <span className="flex opacity-0 transition-opacity duration-(--motion-fast) group-focus-within:opacity-100 group-hover:opacity-100">
        <Button
          variant="ghost"
          size="icon-sm"
          onClick={() => onStartEdit(term)}
          aria-label={`${t("glossary:edit")}: ${term.source}`}
          title={t("glossary:edit")}
        >
          <Pencil strokeWidth={1.6} />
        </Button>
        <Button
          variant="ghost"
          size="icon-sm"
          onClick={() => onRemove(term.source)}
          aria-label={`${t("glossary:remove")}: ${term.source}`}
          title={t("glossary:remove")}
        >
          <Trash2 strokeWidth={1.6} />
        </Button>
      </span>
    </li>
  );
}
