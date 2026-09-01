import { useState } from "react";
import { CalendarDays } from "lucide-react";
import { useLocale } from "../locale";
import type { NotebookDateLikeFieldProps } from "../types/notebook";

function quarterFromDate(date: Date) {
  return `${date.getFullYear()}-Q${Math.floor(date.getMonth() / 3) + 1}`;
}

function shiftQuarter(quarter: string, offset: number) {
  const [yearPart, quarterPart] = quarter.split("-Q");
  const year = Number(yearPart);
  const quarterNumber = Number(quarterPart);
  const zeroBased = year * 4 + quarterNumber - 1 + offset;
  const nextYear = Math.floor(zeroBased / 4);
  const nextQuarter = (zeroBased % 4) + 1;

  return `${nextYear}-Q${nextQuarter}`;
}

function nearbyQuarters() {
  const currentQuarter = quarterFromDate(new Date());

  return Array.from({ length: 7 }, (_, index) => shiftQuarter(currentQuarter, index - 2));
}

export function NotebookQuarterField({ label, ariaLabel, value, onChange }: NotebookDateLikeFieldProps) {
  const { text } = useLocale();
  const [isOpen, setOpen] = useState(false);
  const currentQuarter = quarterFromDate(new Date());
  const quarters = nearbyQuarters();

  return (
    <div className="date-picker-field">
      <span>{label}</span>
      <div className="date-picker-input-row">
        {/* eslint-disable-next-line no-restricted-syntax -- editable field of a custom quarter-picker widget (text entry + calendar popover); no primitive covers this composite control */}
        <input
          aria-label={ariaLabel}
          placeholder="2026-Q4"
          value={value}
          onChange={(event) => onChange(event.target.value)}
        />
        <button
          aria-label={`${label} picker`}
          className="icon-button date-picker-toggle"
          data-action-kind="control"
          onClick={() => setOpen((current) => !current)}
          type="button"
        >
          <CalendarDays size={15} />
        </button>
      </div>
      {isOpen ? (
        <div className="date-picker-popover quarter-picker-popover">
          <button
            className="secondary-button compact-button"
            data-action-kind="control"
            onClick={() => {
              onChange(currentQuarter);
              setOpen(false);
            }}
            type="button"
          >
            <CalendarDays size={15} />
            {text("Today")}
          </button>
          <div className="quarter-picker-options">
            {quarters.map((quarter) => (
              <button
                className={quarter === value ? "quarter-option quarter-option-active" : "quarter-option"}
                data-action-kind="control"
                key={quarter}
                onClick={() => {
                  onChange(quarter);
                  setOpen(false);
                }}
                type="button"
              >
                {quarter}
              </button>
            ))}
          </div>
          <button
            className="minimal-button"
            data-action-kind="control"
            onClick={() => {
              onChange("");
              setOpen(false);
            }}
            type="button"
          >
            {text("Clear")}
          </button>
        </div>
      ) : null}
    </div>
  );
}
