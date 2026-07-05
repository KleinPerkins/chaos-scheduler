import { Component, type ErrorInfo, type ReactNode } from "react";
import "./ErrorBoundary.css";

interface Props {
  children: ReactNode;
  /** Short label for the view that crashed (e.g. "Dashboard"). */
  viewName?: string;
}

interface State {
  error: Error | null;
}

export default class ErrorBoundary extends Component<Props, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo): void {
    console.error("UI render error:", error, info.componentStack);
  }

  private handleRetry = (): void => {
    this.setState({ error: null });
  };

  render(): ReactNode {
    if (this.state.error) {
      const label = this.props.viewName ?? "This view";
      return (
        <div className="error-boundary" role="alert">
          <h1 className="error-boundary__title">{label} crashed</h1>
          <p className="error-boundary__message">
            {this.state.error.message ||
              "An unexpected rendering error occurred."}
          </p>
          <button
            type="button"
            className="btn btn-primary"
            onClick={this.handleRetry}
          >
            Try again
          </button>
        </div>
      );
    }

    return this.props.children;
  }
}
