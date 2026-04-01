import { useTranslation } from "react-i18next";
import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";
import type { EntryStatus } from "@/types/domain";

const variants: Record<EntryStatus, string> = {
  untranslated: "bg-zinc-500/20 text-zinc-400 border-zinc-500/30",
  draft: "bg-amber-500/20 text-amber-400 border-amber-500/30",
  reviewed: "bg-blue-500/20 text-blue-400 border-blue-500/30",
  final: "bg-emerald-500/20 text-emerald-400 border-emerald-500/30",
};

export function StatusBadge({ status }: { status: EntryStatus }) {
  const { t } = useTranslation("editor");

  return (
    <Badge
      variant="outline"
      className={cn("text-[10px] px-1.5 py-0", variants[status])}
    >
      {t(`status.${status}`)}
    </Badge>
  );
}
