import { Button, EmptyState } from "../ui";
import { useLocale } from "../shared/locale";

// Recovery UI shown when the content area's ErrorBoundary catches a render
// error. Keeps the shell usable and offers escape hatches instead of a blank
// window.
export function AppContentErrorFallback({ reset }: { error: Error; reset: () => void }) {
  const { text } = useLocale();
  return (
    <div className="app-error-recovery" role="alert">
      <EmptyState>{text("Something went wrong displaying this view.")}</EmptyState>
      <div className="app-error-recovery-actions">
        <Button variant="primary" onClick={reset}>
          {text("Try again")}
        </Button>
        <Button variant="secondary" onClick={() => window.location.reload()}>
          {text("Reload app")}
        </Button>
      </div>
    </div>
  );
}
