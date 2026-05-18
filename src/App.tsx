import { useEffect } from "react";
import { TooltipProvider } from "@/components/ui/tooltip";
import { Toaster } from "@/components/ui/sonner";
import { useSettingsStore } from "@/stores/settingsStore";
import { AppShell } from "@/components/layout/AppShell";
import { ErrorBoundary } from "@/components/ErrorBoundary";

function App() {
  const theme = useSettingsStore((s) => s.theme);
  const setTheme = useSettingsStore((s) => s.setTheme);

  useEffect(() => {
    setTheme(theme);
  }, [theme, setTheme]);

  return (
    <ErrorBoundary>
      <TooltipProvider delayDuration={300}>
        <AppShell />
        <Toaster position="bottom-right" />
      </TooltipProvider>
    </ErrorBoundary>
  );
}

export default App;
