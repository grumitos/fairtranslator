import { useId } from "react";
import { cn } from "@/lib/utils";

/*
 * Marca «Dos ventanas»: ventana de origen en contorno y ventana de destino sólida con el cursor
 * ▼ vaciado. Construcción de la familia del kit: rejilla 24, margen 2, formas rellenas,
 * grosor t = 2,6, chaflán del 18 % del lado y separación t/2 entre ventanas. Siempre monocroma.
 */
const BACK_WINDOW =
  "M4.6 2H13.8L16.4 4.6V10.6L13.8 13.2H4.6L2 10.6V4.6Z M5.68 4.6L4.6 5.68V9.52L5.68 10.6H12.72L13.8 9.52V5.68L12.72 4.6Z";
const FRONT_WINDOW = "M10.2 10.8H19.4L22 13.4V19.4L19.4 22H10.2L7.6 19.4V13.4Z";
const FRONT_WITH_CURSOR = `${FRONT_WINDOW} M12.2 14.8H17.4L14.8 18Z`;

export function BrandMark({ className, title }: { className?: string; title?: string }) {
  const maskId = `ft-mark-${useId().replace(/[^a-zA-Z0-9_-]/g, "")}`;

  return (
    <svg
      viewBox="0 0 24 24"
      className={cn("size-5 shrink-0 fill-current", className)}
      role={title ? "img" : undefined}
      aria-label={title}
      aria-hidden={title ? undefined : true}
    >
      <defs>
        <mask id={maskId} maskUnits="userSpaceOnUse" x="0" y="0" width="24" height="24">
          <rect width="24" height="24" fill="#fff" />
          <path d={FRONT_WINDOW} fill="#000" stroke="#000" strokeWidth={2.6} />
        </mask>
      </defs>
      <path fillRule="evenodd" mask={`url(#${maskId})`} d={BACK_WINDOW} />
      <path fillRule="evenodd" d={FRONT_WITH_CURSOR} />
    </svg>
  );
}
