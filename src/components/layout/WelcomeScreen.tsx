import { useCallback, useState } from "react";
import { useTranslation } from "react-i18next";
import { open as dialogOpen } from "@tauri-apps/plugin-dialog";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { useProject } from "@/hooks/useProject";
import { NewProjectDialog } from "@/components/layout/NewProjectDialog";
import { SettingsPanel } from "@/components/layout/SettingsPanel";
import { useSettingsStore } from "@/stores/settingsStore";
import { FilePlus, FolderOpen, Moon, Settings, Sun } from "lucide-react";

export function WelcomeScreen() {
  const { t } = useTranslation("common");
  const { open } = useProject();
  const [showNewDialog, setShowNewDialog] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const theme = useSettingsStore((state) => state.theme);
  const setTheme = useSettingsStore((state) => state.setTheme);

  const handleOpen = useCallback(async () => {
    const selected = await dialogOpen({
      directory: true,
      title: t("openProjectFolder"),
      filters: [],
    });
    if (selected && typeof selected === "string") {
      try {
        await open(selected);
      } catch (error) {
        toast.error(t("error"), {
          description: error instanceof Error ? error.message : String(error),
        });
      }
    }
  }, [open, t]);

  const handleThemeToggle = useCallback(() => {
    setTheme(theme === "dark" ? "light" : "dark");
  }, [setTheme, theme]);

  return (
    <div className="relative flex min-h-dvh w-full min-w-0 flex-col items-center justify-center overflow-hidden bg-background p-4 text-foreground">
      <div className="absolute top-3 right-3 flex items-center gap-1">
        <Button
          type="button"
          size="icon"
          variant="ghost"
          onClick={handleThemeToggle}
          aria-label={t("welcome.toggleTheme")}
          title={t("welcome.toggleTheme")}
        >
          {theme === "dark" ? <Sun className="size-4" /> : <Moon className="size-4" />}
        </Button>
        <Button
          type="button"
          size="icon"
          variant="ghost"
          onClick={() => setShowSettings(true)}
          aria-label={t("welcome.settings")}
          title={t("welcome.settings")}
        >
          <Settings className="size-4" />
        </Button>
      </div>
      <div className="w-full max-w-sm min-w-0 rounded-lg border bg-card p-5 text-center shadow-sm">
        <div className="border-b pb-5">
          <h1 className="text-2xl font-semibold tracking-normal break-words">{t("welcome.title")}</h1>
          <p className="mt-2 text-sm leading-6 text-muted-foreground">{t("welcome.subtitle")}</p>
        </div>

        <div className="mx-auto mt-5 flex w-full max-w-xs min-w-0 flex-col gap-3">
          <Button
            size="lg"
            onClick={() => setShowNewDialog(true)}
            className="w-full gap-2"
          >
            <FilePlus className="size-5" />
            {t("welcome.newProject")}
          </Button>
          <p className="text-xs text-muted-foreground">
            {t("welcome.newProjectDesc")}
          </p>

          <Button
            variant="outline"
            size="lg"
            onClick={handleOpen}
            className="w-full gap-2"
          >
            <FolderOpen className="size-5" />
            {t("welcome.openProject")}
          </Button>
          <p className="text-xs text-muted-foreground">
            {t("welcome.openProjectDesc")}
          </p>
        </div>
      </div>

      <NewProjectDialog open={showNewDialog} onOpenChange={setShowNewDialog} />
      <SettingsPanel open={showSettings} onOpenChange={setShowSettings} />
    </div>
  );
}
