import type { CSSProperties } from "react"
import {
  CircleCheckIcon,
  InfoIcon,
  Loader2Icon,
  OctagonXIcon,
  TriangleAlertIcon,
} from "lucide-react"
import { Toaster as Sonner, type ToasterProps } from "sonner"

// Avisos del kit: superficie bg-2, borde fuerte y color solo en el ícono de estado.
const Toaster = ({ ...props }: ToasterProps) => {
  return (
    <Sonner
      theme="dark"
      className="toaster group"
      icons={{
        success: <CircleCheckIcon className="size-4 text-success" />,
        info: <InfoIcon className="size-4 text-info" />,
        warning: <TriangleAlertIcon className="size-4 text-warning" />,
        error: <OctagonXIcon className="size-4 text-danger" />,
        loading: <Loader2Icon className="size-4 animate-spin text-ink-3" />,
      }}
      toastOptions={{
        classNames: {
          description: "text-ink-3!",
        },
      }}
      style={
        {
          "--normal-bg": "var(--bg-2)",
          "--normal-text": "var(--ink)",
          "--normal-border": "var(--line-strong)",
          "--border-radius": "8px",
          fontFamily: "var(--font-sans)",
        } as CSSProperties
      }
      {...props}
    />
  )
}

export { Toaster }
