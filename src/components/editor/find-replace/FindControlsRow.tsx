import type { RefObject } from "react";
import { useTranslation } from "react-i18next";
import { X, ChevronUp, ChevronDown, CaseSensitive, Regex } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { cn } from "@/lib/utils";

interface FindControlsRowProps {
  findInputRef: RefObject<HTMLInputElement | null>;
  findQuery: string;
  isInvalidRegex: boolean;
  matchInfoLabel: string;
  hasMatches: boolean;
  matchCase: boolean;
  useRegex: boolean;
  searchTarget: "original" | "translation" | "both";
  onFindChange: (query: string) => void;
  onSubmitSearch: (goPrevious: boolean) => void;
  onPrev: () => void;
  onNext: () => void;
  onToggleMatchCase: () => void;
  onToggleRegex: () => void;
  onSearchTargetChange: (target: "original" | "translation" | "both") => void;
  onClose: () => void;
}

export function FindControlsRow({
  findInputRef,
  findQuery,
  isInvalidRegex,
  matchInfoLabel,
  hasMatches,
  matchCase,
  useRegex,
  searchTarget,
  onFindChange,
  onSubmitSearch,
  onPrev,
  onNext,
  onToggleMatchCase,
  onToggleRegex,
  onSearchTargetChange,
  onClose,
}: FindControlsRowProps) {
  const { t } = useTranslation("editor");

  return (
    <div className="flex items-center gap-1">
      <Input
        ref={findInputRef}
        value={findQuery}
        onChange={(e) => onFindChange(e.target.value)}
        placeholder={t("findReplace.findPlaceholder")}
        className={cn(
          "h-7 flex-1 text-xs",
          isInvalidRegex && "border-destructive focus-visible:ring-destructive",
        )}
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            onSubmitSearch(e.shiftKey);
          }
        }}
      />

      <span className="min-w-16 text-center text-xs text-muted-foreground">{matchInfoLabel}</span>

      <Tooltip>
        <TooltipTrigger asChild>
          <Button variant="ghost" size="icon-sm" onClick={onPrev} disabled={!hasMatches}>
            <ChevronUp className="size-3.5" />
          </Button>
        </TooltipTrigger>
        <TooltipContent side="bottom">{t("findReplace.previous")}</TooltipContent>
      </Tooltip>

      <Tooltip>
        <TooltipTrigger asChild>
          <Button variant="ghost" size="icon-sm" onClick={onNext} disabled={!hasMatches}>
            <ChevronDown className="size-3.5" />
          </Button>
        </TooltipTrigger>
        <TooltipContent side="bottom">{t("findReplace.next")}</TooltipContent>
      </Tooltip>

      <Tooltip>
        <TooltipTrigger asChild>
          <Button
            variant={matchCase ? "secondary" : "ghost"}
            size="icon-sm"
            onClick={onToggleMatchCase}
          >
            <CaseSensitive className="size-3.5" />
          </Button>
        </TooltipTrigger>
        <TooltipContent side="bottom">{t("findReplace.matchCase")}</TooltipContent>
      </Tooltip>

      <Tooltip>
        <TooltipTrigger asChild>
          <Button variant={useRegex ? "secondary" : "ghost"} size="icon-sm" onClick={onToggleRegex}>
            <Regex className="size-3.5" />
          </Button>
        </TooltipTrigger>
        <TooltipContent side="bottom">{t("findReplace.useRegex")}</TooltipContent>
      </Tooltip>

      <Select value={searchTarget} onValueChange={onSearchTargetChange}>
        <SelectTrigger className="h-7 w-28 text-xs">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value="translation">{t("findReplace.translationText")}</SelectItem>
          <SelectItem value="original">{t("findReplace.originalText")}</SelectItem>
          <SelectItem value="both">{t("findReplace.bothTexts")}</SelectItem>
        </SelectContent>
      </Select>

      <Tooltip>
        <TooltipTrigger asChild>
          <Button variant="ghost" size="icon-sm" onClick={onClose}>
            <X className="size-3.5" />
          </Button>
        </TooltipTrigger>
        <TooltipContent side="bottom">{t("findReplace.close")}</TooltipContent>
      </Tooltip>
    </div>
  );
}
