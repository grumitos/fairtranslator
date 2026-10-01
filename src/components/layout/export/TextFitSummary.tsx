import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { ControlCodeText } from "@/components/editor/ControlCodeText";
import type { Project, TextFitReport, TextOverflow } from "@/types/domain";
import { Fact } from "./Fact";

// Tope de filas en la lista; el resto se resume con un contador.
const MAX_LISTED = 200;

interface TextFitSummaryProps {
  report: TextFitReport;
  project: Project;
  onOpenEntry: (filePath: string, entryId: string) => void;
}

/**
 * Qué hizo el exportador para que el texto quepa en las ventanas del juego y qué entradas siguen sin
 * caber. Cada entrada de la lista lleva a su fila en la tabla.
 */
export function TextFitSummary({ report, project, onOpenEntry }: TextFitSummaryProps) {
  const { t } = useTranslation("common");

  const entries = useMemo(() => {
    const byKey = new Map<string, { context: string; translation: string }>();
    for (const file of project.files) {
      for (const entry of file.entries) {
        byKey.set(entryKey(file.relativePath, entry.id), { context: entry.fileContext, translation: entry.translation });
      }
    }
    return byKey;
  }, [project]);

  const listed = report.overflow.slice(0, MAX_LISTED);
  const hidden = report.overflow.length - listed.length;

  return (
    <section className="flex flex-col gap-2" aria-label={t("export.fit.title")}>
      <h3 className="m-0 text-xs font-normal text-ink-3">{t("export.fit.title")}</h3>
      <dl className="m-0 flex flex-col border-t border-line">
        <Fact label={t("export.fit.wrapped")} value={String(report.wrappedEntries)} />
        <Fact label={t("export.fit.pagesAdded")} value={String(report.pagesAdded)} />
        <Fact
          label={t("export.fit.overflow")}
          value={String(report.overflowCount)}
          tone={report.overflowCount > 0 ? "warning" : undefined}
        />
        {report.wrapPlugin && <Fact label={t("export.fit.plugin")} value={report.wrapPlugin} />}
      </dl>

      {listed.length > 0 && (
        <ul className="m-0 flex list-none flex-col p-0" aria-label={t("export.fit.list")}>
          {listed.map((item) => {
            const entry = entries.get(entryKey(item.filePath, item.entryId));
            return (
              <li key={`${item.filePath}:${item.entryId}:${item.kind}`}>
                <button
                  type="button"
                  onClick={() => onOpenEntry(item.filePath, item.entryId)}
                  className="flex w-full items-center gap-3 rounded-sm border-b border-line-soft px-1 py-2 text-left outline-none transition-colors duration-(--motion-fast) hover:bg-surface-2 focus-visible:outline-2 focus-visible:outline-ink"
                >
                  <span className="flex min-w-0 flex-1 flex-col gap-0.5">
                    <span className="truncate font-mono text-[11.5px] text-ink-3">{entry?.context ?? item.entryId}</span>
                    <ControlCodeText text={firstLine(entry?.translation)} className="block truncate text-[13px] leading-5 text-ink" />
                  </span>
                  <span className="flex shrink-0 flex-col items-end gap-0.5">
                    <span className="text-[11.5px] text-ink-3">{t(`export.fit.kinds.${item.kind}`)}</span>
                    <span className="font-mono text-[11.5px] text-warning">{measure(item, t)}</span>
                  </span>
                </button>
              </li>
            );
          })}
        </ul>
      )}
      {hidden > 0 && <p className="m-0 font-mono text-xs text-ink-3">{t("export.fit.more", { count: hidden })}</p>}
    </section>
  );
}

function entryKey(filePath: string, entryId: string) {
  return `${filePath}\u0000${entryId}`;
}

function firstLine(text: string | undefined) {
  return text?.split("\n").find((line) => line.trim()) ?? "";
}

// Lo que desborda: filas si sobran líneas; si no, el ancho estimado frente al disponible.
function measure(item: TextOverflow, t: (key: string, options?: Record<string, unknown>) => string) {
  if (item.maxLines !== null && item.lines > item.maxLines) {
    return t("export.fit.rows", { lines: item.lines, max: item.maxLines });
  }
  return t("export.fit.width", { width: item.width, max: item.maxWidth });
}
