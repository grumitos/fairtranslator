import type { RefObject } from "react";
import { useTranslation } from "react-i18next";
import { X, ChevronUp, ChevronDown } from "lucide-react";
import { Button } from "@/components/ui/button";
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
  matchInfoShort: string;
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

const toggleClass = (pressed: boolean) =>
  cn(
    "h-[22px] min-w-[26px] rounded-sm px-1.5 font-mono text-[11.5px] outline-none transition-colors duration-(--motion-fast) focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-ink",
    pressed ? "bg-surface-4 text-ink" : "text-ink-3 hover:text-ink",
  );

export function FindControlsRow({
  findInputRef,
  findQuery,
  isInvalidRegex,
  matchInfoLabel,
  matchInfoShort,
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
    <>
      <div
        className={cn(
          "flex h-[30px] w-[250px] min-w-[160px] shrink items-center gap-1 rounded-md border bg-surface-2 pr-1 pl-2.5 focus-within:border-line-chip",
          isInvalidRegex ? "border-danger" : "border-line-field hover:border-line-strong",
        )}
      >
        <input
          ref={findInputRef}
          name="find-query"
          value={findQuery}
          onChange={(e) => onFindChange(e.target.value)}
          placeholder={t("findReplace.findPlaceholder")}
          autoComplete="off"
          aria-label={t("findReplace.find")}
          aria-invalid={isInvalidRegex}
          className="h-7 min-w-0 flex-1 bg-transparent text-sm text-ink outline-none placeholder:text-ink-4"
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.nativeEvent.isComposing) {
              onSubmitSearch(e.shiftKey);
            }
          }}
        />
        <button
          type="button"
          onClick={onToggleMatchCase}
          aria-label={t("findReplace.matchCase")}
          aria-pressed={matchCase}
          title={t("findReplace.matchCase")}
          className={toggleClass(matchCase)}
        >
          Aa
        </button>
        <button
          type="button"
          onClick={onToggleRegex}
          aria-label={t("findReplace.useRegex")}
          aria-pressed={useRegex}
          title={t("findReplace.useRegex")}
          className={toggleClass(useRegex)}
        >
          .*
        </button>
      </div>

      {/* Visible, el contador corto; el lector de pantalla recibe la frase completa. */}
      <span
        className={cn(
          "min-w-[64px] shrink-0 text-center font-mono text-xs whitespace-nowrap",
          isInvalidRegex ? "text-danger" : hasMatches ? "text-ink-2" : "text-ink-3",
        )}
      >
        <span aria-hidden="true">{matchInfoShort}</span>
        <span className="sr-only" role="status" aria-live="polite">
          {matchInfoLabel}
        </span>
      </span>

      <Button
        variant="ghost"
        size="icon-sm"
        onClick={onPrev}
        disabled={!hasMatches}
        aria-label={t("findReplace.previous")}
        title={`${t("findReplace.previous")} · Shift+Enter`}
      >
        <ChevronUp strokeWidth={1.7} />
      </Button>
      <Button
        variant="ghost"
        size="icon-sm"
        onClick={onNext}
        disabled={!hasMatches}
        aria-label={t("findReplace.next")}
        title={`${t("findReplace.next")} · Enter`}
      >
        <ChevronDown strokeWidth={1.7} />
      </Button>

      <Select value={searchTarget} onValueChange={onSearchTargetChange}>
        <SelectTrigger size="sm" className="w-[132px] shrink-0 text-[12.5px]" aria-label={t("findReplace.searchIn")}>
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value="translation">{t("findReplace.translationText")}</SelectItem>
          <SelectItem value="original">{t("findReplace.originalText")}</SelectItem>
          <SelectItem value="both">{t("findReplace.bothTexts")}</SelectItem>
        </SelectContent>
      </Select>

      <Button
        variant="ghost"
        size="icon-sm"
        onClick={onClose}
        aria-label={t("findReplace.close")}
        title={`${t("findReplace.close")} · Esc`}
        className="order-last ml-auto"
      >
        <X strokeWidth={1.7} />
      </Button>
    </>
  );
}
