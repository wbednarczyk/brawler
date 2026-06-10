import type { CSSProperties } from "react";

type TickerLabelProps = {
  value: string;
  className?: string;
  exposeText?: boolean;
  title?: string;
};

const exchangeColorByCode: Record<string, string> = {
  GPW: "#57d7ff",
  NC: "#ffd166",
  NEWCONNECT: "#ffd166",
  NASDAQ: "#63c0e9",
  NQ: "#63c0e9",
  NYSE: "#f0b85a",
};

const fallbackExchangePalette = [
  "#2dd4bf",
  "#f97316",
  "#a3e635",
  "#38bdf8",
  "#facc15",
  "#fb7185",
  "#22c55e",
  "#e879f9",
];

function splitQualifiedTicker(value: string) {
  const separatorIndex = value.indexOf(":");

  if (separatorIndex <= 0 || separatorIndex === value.length - 1) {
    return null;
  }

  return {
    exchange: value.slice(0, separatorIndex),
    symbol: value.slice(separatorIndex + 1),
  };
}

export function tickerExchangeColor(value: string) {
  const exchange = splitQualifiedTicker(value)?.exchange.toUpperCase() ?? value.toUpperCase();

  return exchangeColorByCode[exchange] ?? fallbackExchangePalette[exchangeColorIndex(exchange)];
}

function exchangeColorIndex(exchange: string) {
  let hash = 0;

  for (const character of exchange) {
    hash = (hash * 31 + character.charCodeAt(0)) >>> 0;
  }

  return hash % fallbackExchangePalette.length;
}

export function TickerLabel({ value, className, exposeText = true, title }: TickerLabelProps) {
  const parts = splitQualifiedTicker(value);
  const classes = ["ticker-label", className].filter(Boolean).join(" ");
  const style = {
    "--ticker-exchange-color": tickerExchangeColor(value),
  } as CSSProperties;

  if (!parts) {
    return (
      <span aria-label={value} className={classes} style={style} title={title ?? value}>
        <span className="ticker-symbol">{value}</span>
      </span>
    );
  }

  return (
    <span aria-label={value} className={classes} style={style} title={title ?? value}>
      {exposeText ? <span className="visually-hidden">{value}</span> : null}
      <span aria-hidden="true" className="ticker-exchange" data-label={parts.exchange} />
      <span aria-hidden="true" className="ticker-separator" />
      <span aria-hidden="true" className="ticker-symbol" data-label={parts.symbol} />
    </span>
  );
}
