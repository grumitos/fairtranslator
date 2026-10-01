import { useCallback, useEffect, useMemo, useState } from "react";
import { toast } from "sonner";
import { Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { SheetBody, SheetFooter } from "@/components/ui/sheet";
import { Textarea } from "@/components/ui/textarea";
import { useStyleGuide } from "@/hooks/useStyleGuide";
import { normalizeStyleGuideRules } from "@/lib/styleGuideRules";

// Pestaña «Guía de estilo» del panel de contexto: reglas numeradas, alta en una fila y guardado.
export function StyleGuideEditor({ active }: { active: boolean }) {
  const { t } = useTranslation("editor");
  const { styleGuide, replace, refresh, isLoading, error: saveError } = useStyleGuide();
  const [draftRules, setDraftRules] = useState<string[]>([]);
  const [newRule, setNewRule] = useState("");
  const [localError, setLocalError] = useState<string | null>(null);

  // El borrador se carga normalizado y luego se edita tal cual: normalizar en cada pulsación borraba
  // espacios finales y, al descartar filas vacías, hacía que lo escrito cayera sobre la regla vecina.
  useEffect(() => {
    setDraftRules(normalizeStyleGuideRules(styleGuide));
    setLocalError(null);
  }, [styleGuide]);

  useEffect(() => {
    if (active) {
      void refresh().catch(() => undefined);
    }
  }, [active, refresh]);

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

    setDraftRules((prev) => [...prev, next]);
    setNewRule("");
  }, [newRule]);

  const handleRuleChange = useCallback((index: number, next: string) => {
    setDraftRules((prev) => prev.map((rule, currentIndex) => (currentIndex === index ? next : rule)));
  }, []);

  const handleRemoveRule = useCallback((index: number) => {
    setDraftRules((prev) => prev.filter((_, currentIndex) => currentIndex !== index));
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
    <>
      <SheetBody className="gap-3 pt-4">
        <ol className="flex flex-col" aria-label={t("styleGuide.rulesLabel", { count: normalizedDraft.length })}>
          {draftRules.length === 0 ? (
            <li className="ft-empty-state text-sm">{t("styleGuide.empty")}</li>
          ) : (
            draftRules.map((rule, index) => (
              <li
                key={index}
                className="group grid grid-cols-[24px_minmax(0,1fr)_auto] items-start gap-2 border-b border-line-soft py-1.5"
              >
                <span className="pt-2 font-mono text-xs text-ink-4">{index + 1}</span>
                <Textarea
                  aria-label={t("styleGuide.ruleInputLabel", { index: index + 1 })}
                  name={`style-guide-rule-${index + 1}`}
                  value={rule}
                  onChange={(event) => handleRuleChange(index, event.target.value)}
                  autoComplete="off"
                  spellCheck={false}
                  rows={1}
                  className="min-h-0 resize-none border-transparent bg-transparent px-1.5 py-1.5 hover:border-line-field focus-visible:bg-surface-2"
                />
                <Button
                  size="icon-sm"
                  variant="ghost"
                  onClick={() => handleRemoveRule(index)}
                  aria-label={t("styleGuide.removeRuleLabel", { index: index + 1 })}
                  title={t("styleGuide.remove")}
                  className="mt-0.5 opacity-0 transition-opacity duration-(--motion-fast) group-focus-within:opacity-100 group-hover:opacity-100"
                >
                  <Trash2 aria-hidden="true" strokeWidth={1.6} />
                </Button>
              </li>
            ))
          )}
        </ol>

        <form
          className="flex gap-2"
          onSubmit={(event) => {
            event.preventDefault();
            handleAddRule();
          }}
        >
          <Input
            id="style-guide-new-rule"
            name="style-guide-new-rule"
            aria-label={t("styleGuide.addTitle")}
            value={newRule}
            onChange={(event) => setNewRule(event.target.value)}
            placeholder={t("styleGuide.addTitle")}
            autoComplete="off"
            spellCheck={false}
          />
          <Button type="submit" variant="outline" disabled={!newRule.trim() || isLoading}>
            {t("styleGuide.add")}
          </Button>
        </form>

        {(saveError || localError) && (
          <p className="text-xs text-danger" role="alert">
            {saveError ?? localError}
          </p>
        )}
      </SheetBody>
      <SheetFooter>
        <Button variant="brand" size="lg" onClick={handleSave} disabled={isLoading || !hasChanges}>
          {t("styleGuide.save")}
        </Button>
      </SheetFooter>
    </>
  );
}
