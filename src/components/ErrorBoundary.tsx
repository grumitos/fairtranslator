import { Component, type ReactNode, type ErrorInfo } from "react";
import i18n from "@/lib/i18n";
import { Button } from "@/components/ui/button";

interface ErrorBoundaryProps {
  children: ReactNode;
  fallback?: ReactNode;
  reloadApp?: () => void;
}

interface ErrorBoundaryState {
  hasError: boolean;
  error: Error | null;
}

export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  constructor(props: ErrorBoundaryProps) {
    super(props);
    this.state = { hasError: false, error: null };
  }

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    if (import.meta.env.DEV) {
      console.error("[ErrorBoundary]", error, errorInfo);
    }
  }

  render() {
    if (this.state.hasError) {
      if (this.props.fallback) {
        return this.props.fallback;
      }

      return (
        <div className="flex h-screen flex-col items-center justify-center bg-background p-6 text-foreground">
          <div className="ft-panel w-full max-w-md p-5 text-center" role="alert" aria-live="assertive">
            <h1 className="text-base font-semibold text-destructive">
              {i18n.t("common:errorBoundary.title")}
            </h1>
            <p className="mt-2 text-sm text-muted-foreground">
              {i18n.t("common:errorBoundary.unexpected")}
            </p>
            {import.meta.env.DEV && this.state.error?.message && (
              <p className="mt-2 break-words rounded-md bg-muted px-3 py-2 text-xs text-muted-foreground">
                {this.state.error.message}
              </p>
            )}
            <Button
              className="mt-4"
              onClick={this.props.reloadApp ?? (() => window.location.reload())}
            >
              {i18n.t("common:errorBoundary.retry")}
            </Button>
          </div>
        </div>
      );
    }

    return this.props.children;
  }
}
