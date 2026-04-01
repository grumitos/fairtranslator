import { Plus } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";

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

  return (
    <div className="space-y-2 rounded-lg border p-3">
      <Label className="text-sm font-medium">{t("add")}</Label>
      <div className="flex gap-2">
        <Input
          value={source}
          onChange={(e) => onSourceChange(e.target.value)}
          placeholder={t("sourcePlaceholder")}
          className="flex-1 text-xs"
        />
        <Input
          value={target}
          onChange={(e) => onTargetChange(e.target.value)}
          placeholder={t("targetPlaceholder")}
          className="flex-1 text-xs"
        />
      </div>
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <Checkbox
            id="new-case-sensitive"
            checked={caseSensitive}
            onCheckedChange={(v) => onCaseSensitiveChange(v === true)}
          />
          <Label htmlFor="new-case-sensitive" className="text-xs">
            {t("caseSensitive")}
          </Label>
        </div>
        <Button
          size="sm"
          onClick={onAdd}
          disabled={!source.trim() || !target.trim() || isLoading}
          className="text-xs"
        >
          <Plus className="mr-1 size-3" />
          {t("add")}
        </Button>
      </div>
    </div>
  );
}
