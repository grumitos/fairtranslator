import { useTranslation } from "react-i18next";
import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";
import type { EntryStatus } from "@/types/domain";

const variants: Record<EntryStatus, string> = {
  untranslated: "ft-status-muted",
  draft: "ft-status-draft",
  reviewed: "ft-status-reviewed",
  final: "ft-status-final",
};

export function StatusBadge({ status }: { status: EntryStatus }) {
  const { t } = useTranslation("editor");

  return (
    <Badge
      variant="outline"
      className={cn("h-5 px-1.5 py-0 text-xs font-semibold", variants[status])}
    >
      {t(`status.${status}`)}
    </Badge>
  );
}
