import { useCallback, useState } from "react";
import { useTranslation } from "react-i18next";
import { open as dialogOpen } from "@tauri-apps/plugin-dialog";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { useProject } from "@/hooks/useProject";
import { NewProjectDialog } from "@/components/layout/NewProjectDialog";
import { FilePlus, FolderOpen } from "lucide-react";

export function WelcomeScreen() {
  const { t } = useTranslation("common");
  const { open } = useProject();
  const [showNewDialog, setShowNewDialog] = useState(false);

  const handleOpen = useCallback(async () => {
    const selected = await dialogOpen({
      directory: true,
      title: t("openProject"),
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

  return (
    <div className="flex h-screen flex-col items-center justify-center bg-background text-foreground">
      <div className="flex flex-col items-center gap-6 max-w-md text-center">
        <div>
          <h1 className="text-3xl font-bold">{t("welcome.title")}</h1>
          <p className="mt-2 text-muted-foreground">{t("welcome.subtitle")}</p>
        </div>

        <div className="flex flex-col gap-3 w-full max-w-xs">
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
    </div>
  );
}
