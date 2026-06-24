import { Component, type ErrorInfo, type ReactNode } from "react";

type ErrorBoundaryProps = {
  /** Render the recovery UI. `reset` clears the caught error and re-mounts children. */
  fallback: (error: Error, reset: () => void) => ReactNode;
  /** Bump to force the boundary to clear a stuck error (e.g. after a route change). */
  resetKey?: unknown;
  children: ReactNode;
};

type ErrorBoundaryState = { error: Error | null };

// A render error anywhere in the tree unmounts the WHOLE React root unless a
// boundary catches it — that is how a single panel crash blanked the entire app
// (nav included). This boundary contains a subtree's failure so the rest of the
// shell stays alive and the user gets a recovery action instead of a blank window.
export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  state: ErrorBoundaryState = { error: null };

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { error };
  }

  componentDidUpdate(prevProps: ErrorBoundaryProps) {
    // A changed resetKey (e.g. navigating to another section) clears a stuck error.
    if (this.state.error && prevProps.resetKey !== this.props.resetKey) {
      this.setState({ error: null });
    }
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    // Local-first app: log to the console for diagnostics, never to a network.
    console.error("Brawler render error contained by boundary:", error, info.componentStack);
  }

  private reset = () => this.setState({ error: null });

  render() {
    if (this.state.error) {
      return this.props.fallback(this.state.error, this.reset);
    }
    return this.props.children;
  }
}
