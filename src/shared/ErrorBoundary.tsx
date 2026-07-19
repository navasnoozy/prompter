import { Component, type ReactNode } from "react";

type ErrorBoundaryProps = {
  children: ReactNode;
};

type ErrorBoundaryState = {
  error: Error | null;
};

export class ErrorBoundary extends Component<
  ErrorBoundaryProps,
  ErrorBoundaryState
> {
  state: ErrorBoundaryState = { error: null };

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { error };
  }

  render() {
    if (this.state.error === null) return this.props.children;

    return (
      <div className="error-boundary" role="alert">
        <h1>Prompter ran into a problem</h1>
        <p>{this.state.error.message}</p>
        <button onClick={() => window.location.reload()} type="button">
          Reload Prompter
        </button>
      </div>
    );
  }
}
