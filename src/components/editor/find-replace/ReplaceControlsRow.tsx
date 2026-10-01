import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";

interface ReplaceControlsRowProps {
  replaceText: string;
  disabled: boolean;
  onReplaceTextChange: (value: string) => void;
  onReplaceOne: () => void;
  onReplaceAll: () => void;
}

export function ReplaceControlsRow({
  replaceText,
  disabled,
  onReplaceTextChange,
  onReplaceOne,
  onReplaceAll,
}: ReplaceControlsRowProps) {
  const { t } = useTranslation("editor");

  return (
    <>
      <input
        name="replace-text"
        value={replaceText}
        onChange={(e) => onReplaceTextChange(e.target.value)}
        placeholder={t("findReplace.replacePlaceholder")}
        autoComplete="off"
        aria-label={t("findReplace.replace")}
        className="h-[30px] w-[220px] min-w-[140px] shrink rounded-md border border-line-field bg-surface-2 px-2.5 text-sm text-ink outline-none placeholder:text-ink-4 hover:border-line-strong focus-visible:border-line-chip"
        onKeyDown={(e) => {
          if (e.key === "Enter" && !e.nativeEvent.isComposing) {
            onReplaceOne();
          }
        }}
      />
      <Button
        variant="outline"
        size="sm"
        onClick={onReplaceOne}
        disabled={disabled}
        aria-label={t("findReplace.replaceOne")}
        title={t("findReplace.replaceOne")}
      >
        {t("findReplace.replaceShort")}
      </Button>
      <Button
        variant="outline"
        size="sm"
        onClick={onReplaceAll}
        disabled={disabled}
        aria-label={t("findReplace.replaceAll")}
        title={t("findReplace.replaceAll")}
      >
        {t("findReplace.replaceAllShort")}
      </Button>
    </>
  );
}
