import { useCallback, useState } from "react";
import { useTranslation } from "react-i18next";
import { open as dialogOpen } from "@tauri-apps/plugin-dialog";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from "@/components/ui/dialog";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useProject } from "@/hooks/useProject";
import { FolderSearch } from "lucide-react";

const LANGUAGES = [
  { value: "ja", labelKey: "languages.ja" },
  { value: "en", labelKey: "languages.en" },
  { value: "es", labelKey: "languages.es" },
  { value: "zh", labelKey: "languages.zh" },
  { value: "ko", labelKey: "languages.ko" },
  { value: "fr", labelKey: "languages.fr" },
  { value: "de", labelKey: "languages.de" },
  { value: "pt", labelKey: "languages.pt" },
  { value: "ru", labelKey: "languages.ru" },
  { value: "it", labelKey: "languages.it" },
];

interface NewProjectDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function NewProjectDialog({ open, onOpenChange }: NewProjectDialogProps) {
  const { t } = useTranslation("common");
  const { create, isLoading, error } = useProject();
  const [formName, setFormName] = useState("");
  const [formGameDir, setFormGameDir] = useState("");
  const [formSourceLang, setFormSourceLang] = useState("ja");
  const [formTargetLang, setFormTargetLang] = useState("es");

  const handleBrowseGameDir = useCallback(async () => {
    const selected = await dialogOpen({
      directory: true,
      title: t("newProjectDialog.selectFolder"),
    });
    if (selected && typeof selected === "string") {
      setFormGameDir(selected);
      if (!formName) {
        const folderName = selected.split(/[/\\]/).pop() ?? "";
        setFormName(folderName);
      }
    }
  }, [t, formName]);

  const handleCreate = useCallback(async () => {
    if (!formGameDir || !formName) return;
    try {
      await create(formGameDir, formName, formSourceLang, formTargetLang);
      onOpenChange(false);
      // Reset form
      setFormName("");
      setFormGameDir("");
      setFormSourceLang("ja");
      setFormTargetLang("es");
    } catch (e) {
      toast.error(t("error"), {
        description: e instanceof Error ? e.message : String(e),
      });
    }
  }, [create, formGameDir, formName, formSourceLang, formTargetLang, onOpenChange, t]);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{t("newProjectDialog.title")}</DialogTitle>
        </DialogHeader>

        <div className="flex flex-col gap-4 py-4">
          {/* Game directory */}
          <div className="flex flex-col gap-1.5">
            <label htmlFor="npd-game-dir" className="text-sm font-medium">
              {t("newProjectDialog.gameDir")}
            </label>
            <div className="flex gap-2">
              <Input
                id="npd-game-dir"
                value={formGameDir}
                onChange={(e) => setFormGameDir(e.target.value)}
                placeholder={t("newProjectDialog.gameDirPlaceholder")}
                className="flex-1 text-xs"
                readOnly
              />
              <Button
                variant="outline"
                size="sm"
                onClick={handleBrowseGameDir}
              >
                <FolderSearch className="size-4 mr-1" />
                {t("newProjectDialog.browse")}
              </Button>
            </div>
          </div>

          {/* Project name */}
          <div className="flex flex-col gap-1.5">
            <label htmlFor="npd-name" className="text-sm font-medium">
              {t("newProjectDialog.name")}
            </label>
            <Input
              id="npd-name"
              value={formName}
              onChange={(e) => setFormName(e.target.value)}
              placeholder={t("newProjectDialog.namePlaceholder")}
            />
          </div>

          {/* Languages */}
          <div className="grid grid-cols-2 gap-3">
            <div className="flex flex-col gap-1.5">
              <label className="text-sm font-medium">
                {t("newProjectDialog.sourceLang")}
              </label>
              <Select value={formSourceLang} onValueChange={setFormSourceLang}>
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {LANGUAGES.map((lang) => (
                    <SelectItem key={lang.value} value={lang.value}>
                      {t(lang.labelKey)}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <div className="flex flex-col gap-1.5">
              <label className="text-sm font-medium">
                {t("newProjectDialog.targetLang")}
              </label>
              <Select value={formTargetLang} onValueChange={setFormTargetLang}>
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {LANGUAGES.map((lang) => (
                    <SelectItem key={lang.value} value={lang.value}>
                      {t(lang.labelKey)}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          </div>
        </div>

        <DialogFooter>
          <Button
            variant="outline"
            onClick={() => onOpenChange(false)}
          >
            {t("cancel")}
          </Button>
          <Button
            onClick={handleCreate}
            disabled={!formGameDir || !formName || isLoading}
          >
            {isLoading
              ? t("newProjectDialog.creating")
              : t("newProjectDialog.create")}
          </Button>
        </DialogFooter>

        {error && (
          <p className="text-xs text-destructive">{error}</p>
        )}
      </DialogContent>
    </Dialog>
  );
}
