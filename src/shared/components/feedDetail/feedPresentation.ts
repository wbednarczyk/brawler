import type { PresentationKind } from "../../../api/generated/PresentationKind";

export type FeedKindChip = { label: string; tone: "media" | "official" | "danger" };

// Host-neutral presentation-kind -> localized chip (dogfooding #8): every
// render site (Inbox row, Company feed row, the shared detail dispatcher)
// shows the SAME chip for a feed item's kind — never the raw backend
// `item.type` (owner finding: "Official report" leaking into the PL UI).
// Exhaustive over `PresentationKind` — a missing arm is a compile error, not
// a silent `null` (the old detail-only map's `redFlag -> null`, F1 #413).
export function feedKindChip(kind: PresentationKind, text: (value: string) => string): FeedKindChip {
  switch (kind) {
    case "media":
      return { label: text("Media"), tone: "media" };
    case "filing":
      return { label: text("ESPI notice"), tone: "official" };
    case "report":
      return { label: text("Periodic report"), tone: "official" };
    case "redFlag":
      return { label: text("Red flag"), tone: "danger" };
  }
}
