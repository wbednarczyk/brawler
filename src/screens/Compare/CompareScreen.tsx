import { useLocale } from "../../shared/locale";
import { EmptyState, PanelHeader } from "../../ui";

// Compare — the cross-company mode destination (ADR 0054). Task 1 lands the
// routing and a minimal shell; the cross-company KPI comparison table (v0.53)
// and screener/leaderboard (v0.58) plug into this mode as they land.
export function CompareScreen() {
  const { t, text } = useLocale();

  return (
    <section className="compare-screen feed-panel" aria-labelledby="compare-title">
      <PanelHeader
        title={t("compare.title")}
        description={t("compare.description")}
        titleId="compare-title"
      />
      <EmptyState>{text("Cross-company comparison is coming with the valuation arc.")}</EmptyState>
    </section>
  );
}
