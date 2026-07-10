import {
  type CSSProperties,
  useCallback,
  useEffect,
  useMemo,
  useState,
} from "react";
import { toast } from "sonner";
import { Plus, Save, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Sheet, SheetContent, SheetDescription, SheetHeader, SheetTitle } from "@/components/ui/sheet";
import { useStyleGuide } from "@/hooks/useStyleGuide";
import { normalizeStyleGuideRules } from "@/lib/styleGuideRules";

interface StyleGuidePanelProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function StyleGuidePanel({ open, onOpenChange }: StyleGuidePanelProps) {
  const { t } = useTranslation("editor");
  const { styleGuide, replace, refresh, isLoading, error: saveError } = useStyleGuide();
  const [draftRules, setDraftRules] = useState<string[]>([]);
  const [newRule, setNewRule] = useState("");
  const [localError, setLocalError] = useState<string | null>(null);

  useEffect(() => {
    setDraftRules(styleGuide);
    setLocalError(null);
  }, [styleGuide]);

  useEffect(() => {
    if (open) {
      void refresh().catch(() => undefined);
    }
  }, [open, refresh]);

  const normalizedDraft = useMemo(() => normalizeStyleGuideRules(draftRules), [draftRules]);
  const hasChanges = useMemo(
    () => JSON.stringify(normalizedDraft) !== JSON.stringify(styleGuide),
    [normalizedDraft, styleGuide],
  );

  const handleAddRule = useCallback(() => {
    const next = newRule.trim();
    if (!next) {
      return;
    }

    setDraftRules((prev) => normalizeStyleGuideRules([...prev, next]));
    setNewRule("");
  }, [newRule]);

  const handleRuleChange = useCallback((index: number, next: string) => {
    setDraftRules((prev) => {
      const updated = normalizeStyleGuideRules(prev);
      updated[index] = next;
      return updated;
    });
  }, []);

  const handleRemoveRule = useCallback((index: number) => {
    setDraftRules((prev) =>
      normalizeStyleGuideRules(prev).filter((_, currentIndex) => currentIndex !== index),
    );
  }, []);

  const handleSave = useCallback(async () => {
    const normalized = normalizeStyleGuideRules(normalizedDraft);
    setLocalError(null);

    try {
      await replace(normalized);
      setDraftRules(normalized);
      toast.success(t("styleGuide.saved"));
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      setLocalError(message);
      toast.error(t("styleGuide.saveFailed"), {
        description: message,
      });
    }
  }, [replace, t, normalizedDraft]);

  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent style={{ "--sheet-width": "480px" } as CSSProperties}>
        <SheetHeader>
          <SheetTitle>{t("styleGuide.title")}</SheetTitle>
          <SheetDescription>{t("styleGuide.description")}</SheetDescription>
        </SheetHeader>

        <div className="space-y-4 px-4 pb-4">
          <div className="space-y-2">
            <Label htmlFor="style-guide-new-rule">{t("styleGuide.addTitle")}</Label>
            <div className="flex gap-2">
              <Input
                id="style-guide-new-rule"
                name="style-guide-new-rule"
                value={newRule}
                onChange={(event) => setNewRule(event.target.value)}
                placeholder={t("styleGuide.addPlaceholder")}
                autoComplete="off"
                spellCheck={false}
                className="text-xs"
              />
              <Button size="sm" onClick={handleAddRule} disabled={!newRule.trim() || isLoading}>
                <Plus className="mr-2 size-4" />
                {t("styleGuide.add")}
              </Button>
            </div>
          </div>

          <div className="space-y-2">
            <div className="flex items-center justify-between">
              <Label>{t("styleGuide.rulesLabel", { count: normalizedDraft.length })}</Label>
              <Button
                size="sm"
                variant="outline"
                onClick={handleSave}
                disabled={isLoading || !hasChanges}
              >
                <Save className="mr-2 size-4" />
                {t("styleGuide.save")}
              </Button>
            </div>

            <ScrollArea className="h-[260px] rounded-md border p-2">
              {normalizedDraft.length === 0 ? (
                <p className="py-8 text-center text-sm text-muted-foreground">
                  {t("styleGuide.empty")}
                </p>
              ) : (
                <div className="space-y-2">
                  {normalizedDraft.map((rule, index) => (
                    <div key={index} className="flex gap-2">
                      <Input
                        aria-label={t("styleGuide.ruleInputLabel", { index: index + 1 })}
                        name={`style-guide-rule-${index + 1}`}
                        value={rule}
                        onChange={(event) => handleRuleChange(index, event.target.value)}
                        autoComplete="off"
                        spellCheck={false}
                        className="text-xs"
                      />
                      <Button
                        size="icon-sm"
                        variant="ghost"
                        onClick={() => handleRemoveRule(index)}
                        aria-label={t("styleGuide.removeRuleLabel", { index: index + 1 })}
                        title={t("styleGuide.remove")}
                      >
                        <Trash2 aria-hidden="true" className="size-3.5" />
                      </Button>
                    </div>
                  ))}
                </div>
              )}
            </ScrollArea>
          </div>

          {(saveError || localError) && (
            <p className="text-xs text-destructive">{saveError ?? localError}</p>
          )}
        </div>
      </SheetContent>
    </Sheet>
  );
}
