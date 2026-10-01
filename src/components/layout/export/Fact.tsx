import { cn } from "@/lib/utils";

/** Fila etiqueta–valor de los resúmenes del panel Exportar. */
export function Fact({ label, value, tone }: { label: string; value: string; tone?: "warning" }) {
  return (
    <div className="flex items-baseline justify-between gap-4 border-b border-line py-2.5">
      <dt className="text-xs text-ink-3">{label}</dt>
      <dd className={cn("m-0 font-mono text-[12.5px]", tone === "warning" && "text-warning")}>{value}</dd>
    </div>
  );
}
