import { Plus } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";

interface GlossaryAddFormProps {
  source: string;
  target: string;
  caseSensitive: boolean;
  isLoading: boolean;
  onSourceChange: (value: string) => void;
  onTargetChange: (value: string) => void;
  onCaseSensitiveChange: (value: boolean) => void;
  onAdd: () => void;
}

// Alta de término en una sola fila: original, traducción, «Aa» (distinguir mayúsculas) y agregar.
export function GlossaryAddForm({
  source,
  target,
  caseSensitive,
  isLoading,
  onSourceChange,
  onTargetChange,
  onCaseSensitiveChange,
  onAdd,
}: GlossaryAddFormProps) {
  const { t } = useTranslation("glossary");
  const canAdd = Boolean(source.trim() && target.trim()) && !isLoading;

  return (
    <form
      className="grid grid-cols-[minmax(0,1fr)_minmax(0,1fr)_auto_auto] items-center gap-2"
      onSubmit={(event) => {
        event.preventDefault();
        if (canAdd) onAdd();
      }}
    >
      <Input
        name="glossary-source"
        aria-label={t("source")}
        autoComplete="off"
        spellCheck={false}
        value={source}
        onChange={(e) => onSourceChange(e.target.value)}
        placeholder={t("source")}
        className="ft-cjk"
      />
      <Input
        name="glossary-target"
        aria-label={t("target")}
        autoComplete="off"
        spellCheck={false}
        value={target}
        onChange={(e) => onTargetChange(e.target.value)}
        placeholder={t("target")}
      />
      <button
        type="button"
        aria-label={t("caseSensitive")}
        aria-pressed={caseSensitive}
        title={t("caseSensitive")}
        onClick={() => onCaseSensitiveChange(!caseSensitive)}
        className={cn(
          "h-8 min-w-8 rounded-md border px-1.5 font-mono text-xs outline-none transition-colors duration-(--motion-fast) focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ink",
          caseSensitive ? "border-line-chip bg-surface-3 text-ink" : "border-line-field text-ink-3 hover:text-ink",
        )}
      >
        Aa
      </button>
      <Button type="submit" variant="outline" size="icon" className="size-8" disabled={!canAdd} aria-label={t("add")} title={t("add")}>
        <Plus strokeWidth={1.8} />
      </Button>
    </form>
  );
}
