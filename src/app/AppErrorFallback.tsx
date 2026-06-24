import { Button, EmptyState } from "../ui";
import { useLocale } from "../shared/locale";
import { clearCockpitLayoutStorage } from "../screens/Cockpit/DockLayout";

// Recovery UI shown when the content area's ErrorBoundary catches a render error
// (e.g. a cockpit panel or a stored dock layout that throws). Keeps the shell
// usable and offers escape hatches instead of a blank window. "Reset cockpit
// layout" drops the persisted dockview geometry — the known cause of a layout
// that crashes the dock on every render.
export function AppContentErrorFallback({ reset }: { error: Error; reset: () => void }) {
  const { text } = useLocale();
  return (
    <div className="app-error-recovery" role="alert">
      <EmptyState>{text("Something went wrong displaying this view.")}</EmptyState>
      <div className="app-error-recovery-actions">
        <Button variant="primary" onClick={reset}>
          {text("Try again")}
        </Button>
        <Button
          variant="secondary"
          onClick={() => {
            clearCockpitLayoutStorage();
            reset();
          }}
        >
          {text("Reset cockpit layout")}
        </Button>
        <Button variant="secondary" onClick={() => window.location.reload()}>
          {text("Reload app")}
        </Button>
      </div>
    </div>
  );
}
