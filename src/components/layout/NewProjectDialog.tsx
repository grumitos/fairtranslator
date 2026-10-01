import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { open as dialogOpen } from "@tauri-apps/plugin-dialog";
import { ArrowRight, Lock } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Dialog,
  DialogContent,
  DialogDescription,
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
import { confirmAction } from "@/components/common/ConfirmDialog";
import { engineLabel, engineName } from "@/components/common/engineLabel";
import * as ipc from "@/lib/ipc";
import type { GameInspection } from "@/types/domain";

const LANGUAGES = ["ja", "en", "es", "zh", "ko", "fr", "de", "pt", "ru", "it"];

interface NewProjectDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

// Nuevo proyecto: carpeta del juego, nombre e idiomas. Solo etiquetas y datos.
export function NewProjectDialog({ open, onOpenChange }: NewProjectDialogProps) {
  const { t } = useTranslation("common");
  const { create, isDirty, isLoading } = useProject();
  const [createError, setCreateError] = useState<string | null>(null);
  // Lo que se importaría de la carpeta elegida; null mientras no hay carpeta o se está leyendo.
  const [inspection, setInspection] = useState<GameInspection | null>(null);
  const [isInspecting, setIsInspecting] = useState(false);
  const [inspectError, setInspectError] = useState<string | null>(null);
  const inspectRun = useRef(0);

  useEffect(() => {
    if (open) setCreateError(null);
  }, [open]);

  const [formName, setFormName] = useState("");
  const [formGameDir, setFormGameDir] = useState("");
  const [formSourceLang, setFormSourceLang] = useState("ja");
  const [formTargetLang, setFormTargetLang] = useState("es");
  const hasSameLanguage = formSourceLang === formTargetLang;

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
      const run = ++inspectRun.current;
      setInspection(null);
      setInspectError(null);
      setIsInspecting(true);
      try {
        const result = await ipc.gameInspect(selected);
        if (run === inspectRun.current) setInspection(result);
      } catch (error) {
        if (run === inspectRun.current) setInspectError(error instanceof Error ? error.message : String(error));
      } finally {
        if (run === inspectRun.current) setIsInspecting(false);
      }
    }
  }, [t, formName]);

  const isBlocked = Boolean(inspection?.blockReason) || Boolean(inspectError);

  const handleCreate = useCallback(async () => {
    if (!formGameDir || !formName || hasSameLanguage || isInspecting || isBlocked) return;
    if (
      isDirty &&
      !(await confirmAction({ message: t("unsavedChanges.createProject"), confirmLabel: t("discard"), destructive: true }))
    ) {
      return;
    }
    setCreateError(null);
    try {
      await create(formGameDir, formName, formSourceLang, formTargetLang);
      onOpenChange(false);
      setFormName("");
      setFormGameDir("");
      setInspection(null);
      setFormSourceLang("ja");
      setFormTargetLang("es");
    } catch (e) {
      setCreateError(e instanceof Error ? e.message : String(e));
    }
  }, [create, formGameDir, formName, formSourceLang, formTargetLang, hasSameLanguage, isBlocked, isDirty, isInspecting, onOpenChange, t]);

  const languageSelect = (id: string, value: string, onChange: (value: string) => void) => (
    <Select value={value} onValueChange={onChange}>
      <SelectTrigger className="w-full" aria-labelledby={`${id}-label`}>
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        {LANGUAGES.map((lang) => (
          <SelectItem key={lang} value={lang}>
            {t(`languages.${lang}`)} <span className="font-mono text-ink-4">{lang}</span>
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="gap-0 p-0">
        <DialogHeader className="px-5 pt-[18px] pb-3.5">
          <DialogTitle>{t("newProjectDialog.title")}</DialogTitle>
          <DialogDescription className="sr-only">{t("newProjectDialog.description")}</DialogDescription>
        </DialogHeader>

        <dl className="mx-5 flex flex-col border-t border-line">
          <div className="flex flex-col gap-2 border-b border-line py-3">
            <dt id="npd-game-dir-label" className="text-xs text-ink-3">
              {t("newProjectDialog.gameDir")}
            </dt>
            <dd className="m-0 flex items-center gap-2">
              <span
                className={
                  formGameDir
                    ? "min-w-0 flex-1 truncate font-mono text-[12.5px]"
                    : "min-w-0 flex-1 text-[12.5px] text-ink-4"
                }
                title={formGameDir || undefined}
              >
                {formGameDir || t("export.noDestination")}
              </span>
              <Button
                variant="outline"
                size="sm"
                onClick={handleBrowseGameDir}
                aria-describedby="npd-game-dir-label"
              >
                {formGameDir ? t("export.change") : t("export.choose")}
              </Button>
            </dd>
          </div>
          {formGameDir && (
            <GameInspectionRows inspection={inspection} isInspecting={isInspecting} error={inspectError} />
          )}
        </dl>

        <div className="flex flex-col gap-4 px-5 pt-[18px] pb-1">
          <div className="flex flex-col gap-1.5">
            <label htmlFor="npd-name" className="text-xs text-ink-3">
              {t("newProjectDialog.name")}
            </label>
            <Input
              id="npd-name"
              value={formName}
              onChange={(e) => setFormName(e.target.value)}
              placeholder={t("newProjectDialog.namePlaceholder")}
              name="project-name"
              autoComplete="off"
              spellCheck={false}
            />
          </div>

          <div className="grid grid-cols-[minmax(0,1fr)_24px_minmax(0,1fr)] items-end gap-2">
            <div className="flex flex-col gap-1.5">
              <span id="npd-source-lang-label" className="text-xs text-ink-3">
                {t("newProjectDialog.sourceLang")}
              </span>
              {languageSelect("npd-source-lang", formSourceLang, setFormSourceLang)}
            </div>
            <ArrowRight aria-hidden="true" className="mx-auto mb-2 size-4 text-ink-3" strokeWidth={1.7} />
            <div className="flex flex-col gap-1.5">
              <span id="npd-target-lang-label" className="text-xs text-ink-3">
                {t("newProjectDialog.targetLang")}
              </span>
              {languageSelect("npd-target-lang", formTargetLang, setFormTargetLang)}
            </div>
          </div>
          {hasSameLanguage && (
            <p className="m-0 text-xs text-danger" role="alert">
              {t("newProjectDialog.languagesMustDiffer")}
            </p>
          )}
          <p className="m-0 inline-flex items-center gap-2 text-xs text-ink-3">
            <Lock aria-hidden="true" className="size-3.5 shrink-0" strokeWidth={1.6} />
            {t("export.untouched")}
          </p>
          {createError && (
            <p className="m-0 text-xs text-danger" role="alert">
              {createError}
            </p>
          )}
        </div>

        <DialogFooter className="px-5 py-[18px]">
          <Button variant="outline" size="lg" onClick={() => onOpenChange(false)}>
            {t("cancel")}
          </Button>
          <Button
            variant="brand"
            size="lg"
            onClick={handleCreate}
            disabled={!formGameDir || !formName || hasSameLanguage || isLoading || isInspecting || isBlocked}
          >
            {isLoading ? t("newProjectDialog.creating") : t("newProjectDialog.create")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

// Motor y contenido detectados en la carpeta; el espacio queda reservado mientras se lee.
function GameInspectionRows({
  inspection,
  isInspecting,
  error,
}: {
  inspection: GameInspection | null;
  isInspecting: boolean;
  error: string | null;
}) {
  const { t } = useTranslation("common");

  if (error || inspection?.blockReason) {
    const reason = inspection?.blockReason;
    return (
      <div className="border-b border-line py-3">
        <p className="m-0 text-xs text-danger" role="alert">
          {reason ? t(`newProjectDialog.blocked.${reason}`, { archive: inspection?.packedArchive ?? "" }) : error}
        </p>
      </div>
    );
  }

  const pending = isInspecting || !inspection;
  const content =
    inspection && inspection.textFiles !== null && inspection.entries !== null
      ? `${t("newProjectDialog.files", { count: inspection.textFiles })} · ${t("newProjectDialog.entries", { count: inspection.entries })}`
      : "";

  return (
    <>
      <div className="flex items-center justify-between gap-4 border-b border-line py-2.5">
        <dt className="text-xs text-ink-3">{t("newProjectDialog.engine")}</dt>
        <dd className="m-0 flex items-center gap-2 text-[12.5px]">
          {pending || !inspection?.engine ? (
            <span className="text-ink-4">…</span>
          ) : (
            <>
              <span className="rounded-sm border border-line-chip px-1.5 font-mono text-[11px] leading-[18px]">
                {engineLabel(inspection.engine)}
              </span>
              {engineName(inspection.engine)}
            </>
          )}
        </dd>
      </div>
      <div className="flex items-center justify-between gap-4 border-b border-line py-2.5">
        <dt className="text-xs text-ink-3">{t("newProjectDialog.content")}</dt>
        <dd className="m-0 font-mono text-[12.5px]">{pending ? <span className="text-ink-4">…</span> : content}</dd>
      </div>
      {!pending && inspection?.packedArchive && (
        <div className="flex items-center justify-between gap-4 border-b border-line py-2.5">
          <dt className="text-xs text-ink-3">{t("newProjectDialog.packedArchive")}</dt>
          <dd className="m-0 font-mono text-[12.5px] text-warning">{inspection.packedArchive}</dd>
        </div>
      )}
    </>
  );
}
