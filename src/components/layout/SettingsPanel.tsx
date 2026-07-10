import { type CSSProperties, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Sheet,
  SheetContent,
  SheetHeader,
  SheetDescription,
  SheetTitle,
} from "@/components/ui/sheet";
import {
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
} from "@/components/ui/tabs";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useSettingsStore } from "@/stores/settingsStore";
import type { ProviderType } from "@/types/domain";
import { ApiKeySection } from "@/components/layout/settings/ApiKeySection";
import { cn } from "@/lib/utils";

interface SettingsPanelProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  initialProvider?: ProviderType;
}

const PROVIDERS: {
  id: ProviderType;
  labelKey: string;
  descriptionKey: string;
  connection: "api" | "cli";
}[] = [
  {
    id: "opencode",
    labelKey: "settings:apiKeys.opencode.label",
    descriptionKey: "settings:apiKeys.opencode.description",
    connection: "api",
  },
  {
    id: "openai",
    labelKey: "settings:apiKeys.openai.label",
    descriptionKey: "settings:apiKeys.openai.description",
    connection: "api",
  },
  {
    id: "deepl",
    labelKey: "settings:apiKeys.deepl.label",
    descriptionKey: "settings:apiKeys.deepl.description",
    connection: "api",
  },
  {
    id: "google",
    labelKey: "settings:apiKeys.google.label",
    descriptionKey: "settings:apiKeys.google.description",
    connection: "api",
  },
  {
    id: "codex",
    labelKey: "settings:apiKeys.codex.label",
    descriptionKey: "settings:apiKeys.codex.description",
    connection: "cli",
  },
];

export function SettingsPanel({ open, onOpenChange, initialProvider }: SettingsPanelProps) {
  const { t } = useTranslation(["settings", "common"]);
  const theme = useSettingsStore((s) => s.theme);
  const language = useSettingsStore((s) => s.language);
  const wordWrapColumn = useSettingsStore((s) => s.wordWrapColumn);
  const setTheme = useSettingsStore((s) => s.setTheme);
  const setLanguage = useSettingsStore((s) => s.setLanguage);
  const setWordWrapColumn = useSettingsStore((s) => s.setWordWrapColumn);
  const [selectedProvider, setSelectedProvider] = useState<ProviderType>(initialProvider ?? "opencode");
  const selectedProviderConfig =
    PROVIDERS.find((provider) => provider.id === selectedProvider) ?? PROVIDERS[0];

  useEffect(() => {
    if (open) {
      setSelectedProvider(initialProvider ?? "opencode");
    }
  }, [initialProvider, open]);

  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent
        className="overflow-hidden"
        style={{ "--sheet-width": "680px" } as CSSProperties}
      >
        <SheetHeader className="shrink-0 border-b pr-12">
          <SheetTitle>{t("settings:title")}</SheetTitle>
          <SheetDescription>{t("settings:description")}</SheetDescription>
        </SheetHeader>

        <Tabs defaultValue="apikeys" className="min-h-0 flex-1 px-4 pb-4">
          <TabsList className="w-full shrink-0">
            <TabsTrigger value="apikeys" className="flex-1">
              {t("settings:apiKeys.title")}
            </TabsTrigger>
            <TabsTrigger value="appearance" className="flex-1">
              {t("settings:appearance.title")}
            </TabsTrigger>
          </TabsList>

          <TabsContent value="apikeys" className="mt-2 min-h-0 overflow-hidden">
            <div className="flex h-full min-h-0 flex-col gap-3">
              <div className="space-y-1.5 sm:hidden">
                <Label id="settings-provider-label">{t("settings:apiKeys.provider")}</Label>
                <Select
                  value={selectedProvider}
                  onValueChange={(value) => setSelectedProvider(value as ProviderType)}
                >
                  <SelectTrigger
                    className="w-full"
                    aria-labelledby="settings-provider-label"
                  >
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {PROVIDERS.map((provider) => (
                      <SelectItem key={provider.id} value={provider.id}>
                        {t(provider.labelKey)} · {t(`settings:apiKeys.connection.${provider.connection}`)}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>

              <div className="grid min-h-0 flex-1 sm:grid-cols-[190px_minmax(0,1fr)] sm:gap-4">
                <nav
                  className="hidden min-h-0 flex-col gap-1 overflow-y-auto border-r pr-3 sm:flex"
                  aria-label={t("settings:apiKeys.providerNavigation")}
                >
                  {PROVIDERS.map((provider) => {
                    const isSelected = provider.id === selectedProvider;
                    return (
                      <button
                        key={provider.id}
                        type="button"
                        onClick={() => setSelectedProvider(provider.id)}
                        aria-pressed={isSelected}
                        className={cn(
                          "ft-focus flex w-full items-center justify-between gap-2 rounded-md px-2.5 py-2 text-left text-sm transition-colors",
                          isSelected
                            ? "bg-accent text-foreground"
                            : "text-muted-foreground hover:bg-accent/60 hover:text-foreground",
                        )}
                      >
                        <span className="truncate font-medium">{t(provider.labelKey)}</span>
                        <span className="shrink-0 rounded border bg-background/60 px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide text-foreground/75">
                          {t(`settings:apiKeys.connection.${provider.connection}`)}
                        </span>
                      </button>
                    );
                  })}
                </nav>

                <section className="min-h-0 overflow-y-auto pr-1" aria-labelledby="selected-provider-title">
                  <div className="mb-4 border-b pb-4">
                    <div className="flex flex-wrap items-center gap-2">
                      <h2 id="selected-provider-title" className="text-base font-semibold">
                        {t(selectedProviderConfig.labelKey)}
                      </h2>
                      <span className="rounded border bg-muted px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide text-foreground/75">
                        {t(`settings:apiKeys.connection.${selectedProviderConfig.connection}`)}
                      </span>
                    </div>
                    <p className="mt-1 text-xs leading-5 text-muted-foreground">
                      {t(selectedProviderConfig.descriptionKey)}
                    </p>
                  </div>
                  <ApiKeySection key={selectedProvider} provider={selectedProvider} />
                </section>
              </div>
            </div>
          </TabsContent>

          <TabsContent value="appearance" className="mt-4 min-h-0 space-y-6 overflow-y-auto pr-1">
            <div className="space-y-2">
              <Label id="settings-theme-label">{t("settings:appearance.theme")}</Label>
              <Select value={theme} onValueChange={(v) => setTheme(v as "light" | "dark" | "system")}>
                <SelectTrigger aria-labelledby="settings-theme-label" className="w-full">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="light">{t("settings:appearance.light")}</SelectItem>
                  <SelectItem value="dark">{t("settings:appearance.dark")}</SelectItem>
                  <SelectItem value="system">{t("settings:appearance.system")}</SelectItem>
                </SelectContent>
              </Select>
            </div>

            <div className="space-y-2">
              <Label id="settings-language-label">{t("settings:language.title")}</Label>
              <Select
                value={language}
                onValueChange={(v) => setLanguage(v as "es" | "en")}
              >
                <SelectTrigger aria-labelledby="settings-language-label" className="w-full">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="es">{t("settings:language.es")}</SelectItem>
                  <SelectItem value="en">{t("settings:language.en")}</SelectItem>
                </SelectContent>
              </Select>
            </div>

            <div className="space-y-2">
              <Label htmlFor="settings-word-wrap-column">{t("settings:wordWrap.title")}</Label>
              <Input
                id="settings-word-wrap-column"
                name="word-wrap-column"
                type="number"
                inputMode="numeric"
                autoComplete="off"
                min={0}
                max={500}
                value={wordWrapColumn}
                onChange={(e) => setWordWrapColumn(Number(e.target.value))}
                placeholder="0"
                className="w-full"
              />
              <p className="text-xs text-muted-foreground">
                {t("settings:wordWrap.description")}
              </p>
            </div>
          </TabsContent>
        </Tabs>
      </SheetContent>
    </Sheet>
  );
}
