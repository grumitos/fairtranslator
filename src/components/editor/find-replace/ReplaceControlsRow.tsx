import { useTranslation } from "react-i18next";
import { Replace, ReplaceAll } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";

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
    <div className="flex items-center gap-1">
      <Input
        value={replaceText}
        onChange={(e) => onReplaceTextChange(e.target.value)}
        placeholder={t("findReplace.replacePlaceholder")}
        className="h-7 flex-1 text-xs"
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            onReplaceOne();
          }
        }}
      />

      <Tooltip>
        <TooltipTrigger asChild>
          <Button variant="ghost" size="icon-sm" onClick={onReplaceOne} disabled={disabled}>
            <Replace className="size-3.5" />
          </Button>
        </TooltipTrigger>
        <TooltipContent side="bottom">{t("findReplace.replaceOne")}</TooltipContent>
      </Tooltip>

      <Tooltip>
        <TooltipTrigger asChild>
          <Button variant="ghost" size="icon-sm" onClick={onReplaceAll} disabled={disabled}>
            <ReplaceAll className="size-3.5" />
          </Button>
        </TooltipTrigger>
        <TooltipContent side="bottom">{t("findReplace.replaceAll")}</TooltipContent>
      </Tooltip>
    </div>
  );
}
