import { Pencil, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
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

  return (
    <div className="flex items-center gap-2 rounded-lg border p-2">
      {isEditing ? (
        <div className="min-w-0 flex-1 space-y-2">
          <div className="flex gap-2">
            <Input
              aria-label={t("glossary:source")}
              value={term.source}
              disabled
              className="min-w-0 flex-1 text-xs opacity-60"
            />
            <Input
              aria-label={t("glossary:target")}
              value={editTarget}
              onChange={(e) => onEditTargetChange(e.target.value)}
              className="min-w-0 flex-1 text-xs"
            />
          </div>
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <Checkbox
                id="glossary-edit-case-sensitive"
                checked={editCaseSensitive}
                onCheckedChange={(v) => onEditCaseSensitiveChange(v === true)}
              />
              <label htmlFor="glossary-edit-case-sensitive" className="text-xs">
                {t("glossary:caseSensitive")}
              </label>
            </div>
            <div className="flex gap-1">
              <Button size="sm" onClick={onUpdate} disabled={isLoading} className="h-6 text-xs">
                {t("glossary:update")}
              </Button>
              <Button
                size="sm"
                variant="ghost"
                onClick={onCancelEdit}
                className="h-6 text-xs"
              >
                {t("common:cancel")}
              </Button>
            </div>
          </div>
        </div>
      ) : (
        <>
          <div className="min-w-0 flex-1">
            <div className="flex gap-2 text-xs">
              <span className="min-w-0 flex-1 truncate font-medium">{term.source}</span>
              <span className="text-muted-foreground">→</span>
              <span className="min-w-0 flex-1 truncate">{term.target}</span>
            </div>
            {term.caseSensitive && (
              <span
                className="text-[10px] text-muted-foreground"
                aria-label={t("glossary:caseSensitive")}
                title={t("glossary:caseSensitive")}
              >
                {t("glossary:caseSensitiveBadge")}
              </span>
            )}
          </div>
          <Button
            variant="ghost"
            size="icon-sm"
            onClick={() => onStartEdit(term)}
            aria-label={`${t("glossary:edit")}: ${term.source}`}
            title={t("glossary:edit")}
          >
            <Pencil className="size-3" />
          </Button>
          <Button
            variant="ghost"
            size="icon-sm"
            onClick={() => onRemove(term.source)}
            aria-label={`${t("glossary:remove")}: ${term.source}`}
            title={t("glossary:remove")}
          >
            <Trash2 className="size-3" />
          </Button>
        </>
      )}
    </div>
  );
}
