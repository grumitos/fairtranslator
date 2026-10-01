import { useEffect } from "react";
import { TooltipProvider } from "@/components/ui/tooltip";
import { Toaster } from "@/components/ui/sonner";
import { AppShell } from "@/components/layout/AppShell";
import { useCliUpdates } from "@/hooks/useCliUpdates";
import { ErrorBoundary } from "@/components/ErrorBoundary";
import { ConfirmDialogHost } from "@/components/common/ConfirmDialog";

// La pantalla de inicio vive en index.html para que no haya destello blanco; se retira con un
// fundido cuando la app ya pintó su primera vista.
function useDismissSplash() {
  useEffect(() => {
    const splash = document.getElementById("ft-splash");
    if (!splash) return;
    splash.setAttribute("data-leaving", "");
    const timer = window.setTimeout(() => splash.remove(), 220);
    return () => window.clearTimeout(timer);
  }, []);
}

function App() {
  useDismissSplash();
  useCliUpdates();

  return (
    <ErrorBoundary>
      <TooltipProvider delayDuration={300}>
        <AppShell />
        <Toaster position="bottom-right" />
        <ConfirmDialogHost />
      </TooltipProvider>
    </ErrorBoundary>
  );
}

export default App;
