import { Component, type ReactNode, type ErrorInfo } from "react";
import i18n from "@/lib/i18n";

interface ErrorBoundaryProps {
  children: ReactNode;
  fallback?: ReactNode;
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
    console.error("[ErrorBoundary]", error, errorInfo);
  }

  render() {
    if (this.state.hasError) {
      if (this.props.fallback) {
        return this.props.fallback;
      }

      return (
        <div className="flex h-screen flex-col items-center justify-center gap-4 bg-background text-foreground p-8">
          <h1 className="text-xl font-bold text-destructive">
            {i18n.t("common:errorBoundary.title")}
          </h1>
          <p className="text-sm text-muted-foreground max-w-md text-center">
            {this.state.error?.message ?? i18n.t("common:errorBoundary.unexpected")}
          </p>
          <button
            className="px-4 py-2 rounded-md bg-primary text-primary-foreground text-sm hover:bg-primary/90"
            onClick={() => this.setState({ hasError: false, error: null })}
          >
            {i18n.t("common:errorBoundary.retry")}
          </button>
        </div>
      );
    }

    return this.props.children;
  }
}
