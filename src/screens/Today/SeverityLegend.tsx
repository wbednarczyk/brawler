import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { Info } from "lucide-react";

import { Button } from "../../ui";
import { useLocale } from "../../shared/locale";
import type { Severity } from "./streamModel";

/**
 * In-app discoverability for the severity taxonomy (owner dogfooding 2026-07-23:
 * "had to ask what PILNE / UWAGA mean"). A quiet info toggle beside the counter
 * tiles opens a compact popover that mirrors the product-spec severity table —
 * the three levels with one-line meanings, plus the 3-day aging rule. Static,
 * typed, translated (en + pl via `text()`); no backend call. Keyboard-reachable
 * (button toggles; Escape closes) and aria-labelled.
 */

type LegendEntry = { severity: Severity; label: string; description: string };

export function SeverityLegend() {
  const { text } = useLocale();
  const [open, setOpen] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);

  // Dismiss on outside click so the popover behaves like a lightweight menu.
  useEffect(() => {
    if (!open) return;
    function onPointerDown(event: PointerEvent) {
      if (!containerRef.current?.contains(event.target as Node)) setOpen(false);
    }
    document.addEventListener("pointerdown", onPointerDown);
    return () => document.removeEventListener("pointerdown", onPointerDown);
  }, [open]);

  const entries: LegendEntry[] = [
    {
      severity: "urgent",
      label: text("Urgent"),
      description: text("Leads the stream — the only level that can raise a lasting notification."),
    },
    {
      severity: "notable",
      label: text("Notable"),
      description: text("Shown in the stream, with at most a passing notification."),
    },
    {
      severity: "routine",
      label: text("Routine"),
      description: text("Shown in the stream only, dimmed."),
    },
  ];

  function onKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (event.key === "Escape" && open) {
      event.stopPropagation();
      setOpen(false);
    }
  }

  return (
    <div className="today-legend" ref={containerRef} onKeyDown={onKeyDown}>
      <Button
        variant="icon"
        type="button"
        className="today-legend-toggle"
        aria-expanded={open}
        aria-controls="today-severity-legend"
        aria-label={text("What do the severity levels mean?")}
        onClick={() => setOpen((prior) => !prior)}
      >
        <Info aria-hidden="true" size={14} />
      </Button>
      {open ? (
        <div
          id="today-severity-legend"
          role="dialog"
          aria-label={text("Severity levels")}
          className="today-legend-popover"
        >
          <ul className="today-legend-list">
            {entries.map((entry) => (
              <li key={entry.severity} className="today-legend-item">
                <span className={`today-legend-badge today-sev-${entry.severity}`}>
                  {entry.label}
                </span>
                <span className="today-legend-desc">{entry.description}</span>
              </li>
            ))}
          </ul>
          <p className="ui-hint today-legend-aging">
            {text("An urgent event left unattended for more than 3 days steps down to Notable.")}
          </p>
        </div>
      ) : null}
    </div>
  );
}
