import { useTranslation } from "react-i18next";
import { AlertDialog as AlertDialogPrimitive } from "radix-ui";
import { create } from "zustand";
import { buttonVariants } from "@/components/ui/button";
import { cn } from "@/lib/utils";

export interface ConfirmOptions {
  /** La pregunta completa; es el título del diálogo. */
  message: string;
  /** Texto del botón que confirma (por ejemplo "Descartar" o "Eliminar"). */
  confirmLabel: string;
  /** Las acciones que borran o descartan trabajo se pintan en rojo. */
  destructive?: boolean;
}

interface ConfirmRequest extends ConfirmOptions {
  resolve: (confirmed: boolean) => void;
}

const useConfirmStore = create<{ request: ConfirmRequest | null }>(() => ({ request: null }));

/**
 * Pide confirmación con un diálogo propio en lugar de `window.confirm` (WebView2 lo muestra sin estilo
 * y con el origen de la página). Una petición nueva cancela la anterior.
 */
export function confirmAction(options: ConfirmOptions): Promise<boolean> {
  return new Promise((resolve) => {
    useConfirmStore.getState().request?.resolve(false);
    useConfirmStore.setState({ request: { ...options, resolve } });
  });
}

function settle(confirmed: boolean) {
  const { request } = useConfirmStore.getState();
  if (!request) return;
  useConfirmStore.setState({ request: null });
  request.resolve(confirmed);
}

/** Se monta una vez en App. */
export function ConfirmDialogHost() {
  const { t } = useTranslation("common");
  const request = useConfirmStore((s) => s.request);

  return (
    <AlertDialogPrimitive.Root open={request !== null} onOpenChange={(open) => !open && settle(false)}>
      <AlertDialogPrimitive.Portal>
        <AlertDialogPrimitive.Overlay data-slot="dialog-overlay" className="fixed inset-0 top-12 z-50 bg-scrim" />
        <AlertDialogPrimitive.Content
          data-slot="dialog-content"
          className="fixed top-[50%] left-[50%] z-50 flex w-full max-w-[calc(100%-2rem)] translate-x-[-50%] translate-y-[-50%] flex-col gap-5 rounded-xl border border-line-field bg-surface-1 p-5 text-ink shadow-popover outline-none sm:max-w-[420px]"
        >
          <AlertDialogPrimitive.Title className="m-0 text-[15px] leading-snug font-medium">
            {request?.message}
          </AlertDialogPrimitive.Title>
          <AlertDialogPrimitive.Description className="sr-only">{request?.message}</AlertDialogPrimitive.Description>
          <div className="flex justify-end gap-2">
            <AlertDialogPrimitive.Cancel className={cn(buttonVariants({ variant: "outline", size: "lg" }))}>
              {t("cancel")}
            </AlertDialogPrimitive.Cancel>
            <AlertDialogPrimitive.Action
              className={cn(buttonVariants({ variant: request?.destructive ? "destructive" : "default", size: "lg" }))}
              onClick={() => settle(true)}
            >
              {request?.confirmLabel}
            </AlertDialogPrimitive.Action>
          </div>
        </AlertDialogPrimitive.Content>
      </AlertDialogPrimitive.Portal>
    </AlertDialogPrimitive.Root>
  );
}
