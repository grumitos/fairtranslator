import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { ChevronRight } from "lucide-react";
import { Input } from "@/components/ui/input";
import {
  Sheet,
  SheetBody,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from "@/components/ui/sheet";
import { useSettingsStore } from "@/stores/settingsStore";
import * as ipc from "@/lib/ipc";
import type { ProviderType } from "@/types/domain";
import { ApiKeySection } from "@/components/layout/settings/ApiKeySection";
import { CliUpdatesSection } from "@/components/layout/settings/CliUpdatesSection";
import { cn } from "@/lib/utils";

interface SettingsPanelProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  initialProvider?: ProviderType;
}

const PROVIDERS: { id: ProviderType; labelKey: string; connection: "api" | "cli" }[] = [
  { id: "codex", labelKey: "settings:apiKeys.codex.label", connection: "cli" },
  { id: "openai", labelKey: "settings:apiKeys.openai.label", connection: "api" },
  { id: "opencode", labelKey: "settings:apiKeys.opencode.label", connection: "api" },
  { id: "deepl", labelKey: "settings:apiKeys.deepl.label", connection: "api" },
  { id: "google", labelKey: "settings:apiKeys.google.label", connection: "api" },
];

const LANGUAGES: { value: "es" | "en"; lang: string }[] = [
  { value: "es", lang: "es" },
  { value: "en", lang: "en" },
];

/*
 * Ajustes en el panel lateral del kit: proveedores como filas desplegables (una abierta a la vez),
 * actualización de los CLI, idioma de la interfaz y ajuste de línea. Solo etiquetas y datos.
 */
export function SettingsPanel({ open, onOpenChange, initialProvider }: SettingsPanelProps) {
  const { t } = useTranslation(["settings", "common"]);
  const language = useSettingsStore((s) => s.language);
  const wordWrapColumn = useSettingsStore((s) => s.wordWrapColumn);
  const setLanguage = useSettingsStore((s) => s.setLanguage);
  const setWordWrapColumn = useSettingsStore((s) => s.setWordWrapColumn);
  const [expanded, setExpanded] = useState<ProviderType | null>(initialProvider ?? null);
  const [configured, setConfigured] = useState<Partial<Record<ProviderType, boolean>>>({});

  useEffect(() => {
    if (open) {
      setExpanded(initialProvider ?? null);
    }
  }, [initialProvider, open]);

  // Estado de cada proveedor para su fila; se vuelve a leer al cerrar una fila por si se guardó.
  useEffect(() => {
    if (!open) return;
    let isActive = true;
    void Promise.allSettled(
      PROVIDERS.filter((provider) => provider.id !== "codex").map((provider) =>
        ipc.apikeyLoad(provider.id).then((info) => [provider.id, info.isConfigured] as const),
      ),
    ).then((results) => {
      if (!isActive) return;
      const next: Partial<Record<ProviderType, boolean>> = {};
      for (const result of results) {
        if (result.status === "fulfilled") next[result.value[0]] = result.value[1];
      }
      setConfigured(next);
    });
    return () => {
      isActive = false;
    };
  }, [open, expanded]);

  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent>
        <SheetHeader>
          <SheetTitle>{t("settings:title")}</SheetTitle>
          <SheetDescription className="sr-only">{t("settings:description")}</SheetDescription>
        </SheetHeader>

        <SheetBody className="gap-[26px]">
          <section aria-labelledby="settings-providers-title" className="flex flex-col">
            <h3 id="settings-providers-title" className="mb-1.5 text-xs font-medium text-ink-3">
              {t("settings:apiKeys.title")}
            </h3>
            <nav aria-label={t("settings:apiKeys.providerNavigation")} className="flex flex-col">
              {PROVIDERS.map((provider) => {
                const isExpanded = expanded === provider.id;
                const state = provider.id === "codex" ? undefined : configured[provider.id];
                return (
                  <div key={provider.id} className="flex flex-col border-b border-line">
                    <button
                      type="button"
                      aria-expanded={isExpanded}
                      aria-controls={`settings-provider-${provider.id}`}
                      onClick={() => setExpanded(isExpanded ? null : provider.id)}
                      className="flex h-[42px] items-center gap-2.5 px-0.5 text-left text-sm outline-none focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-ink"
                    >
                      <ChevronRight
                        aria-hidden="true"
                        className={cn(
                          "size-3.5 shrink-0 text-ink-3 transition-transform duration-(--motion-fast)",
                          isExpanded && "rotate-90",
                        )}
                        strokeWidth={1.8}
                      />
                      <span className="font-medium">{t(provider.labelKey)}</span>
                      <span className="font-mono text-[11px] text-ink-4 uppercase">
                        {t(`settings:apiKeys.connection.${provider.connection}`)}
                      </span>
                      {state !== undefined && (
                        <span
                          className={cn(
                            "ml-auto inline-flex items-center gap-1.5 text-xs",
                            state ? "text-ink-2" : "text-ink-4",
                          )}
                        >
                          <span
                            aria-hidden="true"
                            className={cn("size-1.5 rounded-full", state ? "bg-success" : "bg-line-track")}
                          />
                          {t(state ? "settings:apiKeys.configured" : "settings:apiKeys.notConfigured")}
                        </span>
                      )}
                    </button>
                    {isExpanded && (
                      <div id={`settings-provider-${provider.id}`} className="pt-1 pr-0.5 pb-[18px] pl-6">
                        <ApiKeySection key={provider.id} provider={provider.id} />
                      </div>
                    )}
                  </div>
                );
              })}
            </nav>
          </section>

          <CliUpdatesSection open={open} />

          <section aria-labelledby="settings-language-title" className="flex flex-col gap-2">
            <h3 id="settings-language-title" className="text-xs font-medium text-ink-3">
              {t("settings:language.title")}
            </h3>
            <div
              role="radiogroup"
              aria-labelledby="settings-language-title"
              className="flex w-56 gap-0.5 rounded-[8px] border border-line-field bg-surface-0 p-0.5"
            >
              {LANGUAGES.map((option) => (
                <button
                  key={option.value}
                  type="button"
                  role="radio"
                  lang={option.lang}
                  aria-checked={language === option.value}
                  onClick={() => setLanguage(option.value)}
                  className={cn(
                    "h-[26px] flex-1 rounded-md text-[12.5px] outline-none transition-colors duration-(--motion-fast) focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ink",
                    language === option.value ? "bg-surface-3 text-ink" : "text-ink-3 hover:text-ink",
                  )}
                >
                  {t(`settings:language.native.${option.value}`)}
                </button>
              ))}
            </div>
          </section>

          <section aria-labelledby="settings-wrap-title" className="flex flex-col gap-2">
            <h3 id="settings-wrap-title" className="text-xs font-medium text-ink-3">
              {t("settings:wordWrap.title")}
            </h3>
            <div className="flex items-center gap-2.5">
              <Input
                id="settings-word-wrap-column"
                name="word-wrap-column"
                aria-labelledby="settings-wrap-title"
                type="number"
                inputMode="numeric"
                autoComplete="off"
                min={0}
                max={500}
                value={wordWrapColumn}
                onChange={(e) => setWordWrapColumn(Number(e.target.value))}
                placeholder="0"
                className="w-20 font-mono"
              />
              <span className="text-[12.5px] text-ink-3">
                {wordWrapColumn > 0
                  ? t("settings:wordWrap.chars", { count: wordWrapColumn })
                  : t("settings:wordWrap.disabled")}
              </span>
            </div>
          </section>
        </SheetBody>
      </SheetContent>
    </Sheet>
  );
}
