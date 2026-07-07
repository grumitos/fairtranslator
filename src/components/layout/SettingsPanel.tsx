import { type CSSProperties } from "react";
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

interface SettingsPanelProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

const PROVIDERS: { id: ProviderType; labelKey: string }[] = [
  { id: "openai", labelKey: "settings:apiKeys.openai.label" },
  { id: "deepl", labelKey: "settings:apiKeys.deepl.label" },
  { id: "google", labelKey: "settings:apiKeys.google.label" },
  { id: "codex", labelKey: "settings:apiKeys.codex.label" },
  { id: "opencode", labelKey: "settings:apiKeys.opencode.label" },
];

export function SettingsPanel({ open, onOpenChange }: SettingsPanelProps) {
  const { t } = useTranslation(["settings", "common"]);
  const theme = useSettingsStore((s) => s.theme);
  const language = useSettingsStore((s) => s.language);
  const wordWrapColumn = useSettingsStore((s) => s.wordWrapColumn);
  const setTheme = useSettingsStore((s) => s.setTheme);
  const setLanguage = useSettingsStore((s) => s.setLanguage);
  const setWordWrapColumn = useSettingsStore((s) => s.setWordWrapColumn);

  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent style={{ "--sheet-width": "420px" } as CSSProperties}>
        <SheetHeader>
          <SheetTitle>{t("settings:title")}</SheetTitle>
          <SheetDescription>{t("settings:description")}</SheetDescription>
        </SheetHeader>

        <Tabs defaultValue="apikeys" className="px-4 pb-4">
          <TabsList className="w-full">
            <TabsTrigger value="apikeys" className="flex-1">
              {t("settings:apiKeys.title")}
            </TabsTrigger>
            <TabsTrigger value="appearance" className="flex-1">
              {t("settings:appearance.title")}
            </TabsTrigger>
          </TabsList>

          <TabsContent value="apikeys" className="mt-4 space-y-6">
            {PROVIDERS.map((provider) => (
              <ApiKeySection key={provider.id} provider={provider.id} />
            ))}
          </TabsContent>

          <TabsContent value="appearance" className="mt-4 space-y-6">
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
