//! Pure FX conversion domain (ADR 0089 decision 2). No I/O — persistence lives
//! in `storage::fx_rates`, fetching/parsing in `source_adapters::nbp_fx`. This
//! module holds the *rules* the comparison read model applies to turn a native
//! amount into PLN, decimal-exact and deterministically labeled:
//!
//! - **flow** KPIs (`measure_window = flow`) convert at the **period-average**
//!   mid (the mean of the available mids whose date falls inside the period);
//! - **stock** KPIs convert at the **last mid <= period end**;
//! - a **PLN** amount is already in PLN (basis `native_pln`, no conversion);
//! - ratios/percentages are never converted — that is a caller-level rule; this
//!   module only exposes the [`FxBasis`] label and the conversion arithmetic.
//!
//! A missing rate is a typed [`FxError::MissingRate`] — never a silent fallback
//! to PLN or a 1.0 multiplier.

use rust_decimal::Decimal;
use serde::Serialize;

/// The measurement window of a KPI (ADR 0027 three-layer model): a **flow** is
/// accumulated over the period (revenue, net profit) and converts at the
/// period-average mid; a **stock** is a point-in-time balance (equity, debt) and
/// converts at the last mid on or before the period end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeasureWindow {
    Flow,
    Stock,
}

/// The per-cell conversion-basis label (contract field `fxBasis`). Deterministic
/// and inspectable so a converted number always says *how* it was converted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FxBasis {
    /// Flow: the mean of the available mids inside the period.
    PeriodAverage,
    /// Stock: the last available mid whose date is <= the period end.
    LatestOnOrBefore,
    /// The amount was already PLN — no conversion applied.
    NativePln,
}

/// Why a conversion could not be produced. Never a silent fallback — the read
/// model renders this as an explicit per-cell flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FxError {
    /// No mid exists for the currency within the window the basis needs.
    MissingRate,
}

/// A resolved conversion: the PLN value plus the basis that produced it.
#[derive(Debug, Clone, PartialEq)]
pub struct Converted {
    pub value_pln: Decimal,
    pub mid: Decimal,
    pub basis: FxBasis,
}

/// PLN currency code — the app's comparison currency and NBP's own base.
pub const PLN: &str = "PLN";

/// One decimal-exact FX observation: a currency's mid rate on a session date.
/// The shared row type across the adapter (parse output), the store (persisted
/// row), and the conversion helpers.
#[derive(Debug, Clone, PartialEq)]
pub struct FxRate {
    pub currency: String,
    pub date: String,
    pub mid: Decimal,
}

/// The mean of the mids whose `date` falls within `[start, end]` inclusive.
/// `rates` need not be sorted. Returns `None` when no mid falls in the window
/// (the caller turns this into [`FxError::MissingRate`]). Decimal-exact: the sum
/// is exact and the division is `Decimal`'s (banker's-rounding) division — never
/// a float.
pub fn period_average_mid(rates: &[(String, Decimal)], start: &str, end: &str) -> Option<Decimal> {
    let mut sum = Decimal::ZERO;
    let mut count: u64 = 0;
    for (date, mid) in rates {
        if date.as_str() >= start && date.as_str() <= end {
            sum += *mid;
            count += 1;
        }
    }
    if count == 0 {
        return None;
    }
    Some(sum / Decimal::from(count))
}

/// The mid with the greatest `date` that is `<= end`. `rates` need not be
/// sorted. Returns `None` when every mid is after `end` or the slice is empty.
pub fn latest_mid_on_or_before(rates: &[(String, Decimal)], end: &str) -> Option<Decimal> {
    rates
        .iter()
        .filter(|(date, _)| date.as_str() <= end)
        .max_by(|(a, _), (b, _)| a.cmp(b))
        .map(|(_, mid)| *mid)
}

/// Resolve the mid to apply for `window` over `[start, end]` from `rates`,
/// returning the mid and its basis, or [`FxError::MissingRate`]. Pure and total.
pub fn resolve_mid(
    rates: &[(String, Decimal)],
    window: MeasureWindow,
    start: &str,
    end: &str,
) -> Result<(Decimal, FxBasis), FxError> {
    match window {
        MeasureWindow::Flow => period_average_mid(rates, start, end)
            .map(|mid| (mid, FxBasis::PeriodAverage))
            .ok_or(FxError::MissingRate),
        MeasureWindow::Stock => latest_mid_on_or_before(rates, end)
            .map(|mid| (mid, FxBasis::LatestOnOrBefore))
            .ok_or(FxError::MissingRate),
    }
}

/// Convert `amount` in `currency` to PLN over `[start, end]` for `window`.
/// `currency == "PLN"` is a no-op passthrough (basis `native_pln`); any other
/// currency resolves a mid from `rates` and multiplies exactly. A missing rate
/// is [`FxError::MissingRate`] — never a PLN guess.
pub fn convert_to_pln(
    amount: Decimal,
    currency: &str,
    rates: &[(String, Decimal)],
    window: MeasureWindow,
    start: &str,
    end: &str,
) -> Result<Converted, FxError> {
    if currency.eq_ignore_ascii_case(PLN) {
        return Ok(Converted {
            value_pln: amount,
            mid: Decimal::ONE,
            basis: FxBasis::NativePln,
        });
    }
    let (mid, basis) = resolve_mid(rates, window, start, end)?;
    Ok(Converted {
        value_pln: amount * mid,
        mid,
        basis,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::prelude::FromStr;

    fn d(s: &str) -> Decimal {
        Decimal::from_str(s).expect("decimal")
    }

    fn rate(date: &str, mid: &str) -> (String, Decimal) {
        (date.to_owned(), d(mid))
    }

    #[test]
    fn period_average_is_the_mean_of_in_range_mids() {
        let rates = vec![
            rate("2024-01-02", "4.0000"),
            rate("2024-01-03", "4.2000"),
            rate("2024-01-31", "4.4000"),
            rate("2024-02-01", "9.9999"), // out of range, excluded
            rate("2023-12-31", "1.0000"), // out of range, excluded
        ];
        let avg = period_average_mid(&rates, "2024-01-01", "2024-01-31").expect("some");
        // (4.0 + 4.2 + 4.4) / 3 = 4.2 exactly.
        assert_eq!(avg, d("4.2"));
    }

    #[test]
    fn period_average_is_none_when_no_mid_in_range() {
        let rates = vec![rate("2023-12-31", "4.0"), rate("2024-02-01", "4.0")];
        assert_eq!(period_average_mid(&rates, "2024-01-01", "2024-01-31"), None);
        assert_eq!(period_average_mid(&[], "2024-01-01", "2024-01-31"), None);
    }

    #[test]
    fn stock_takes_the_last_mid_on_or_before_period_end() {
        let rates = vec![
            rate("2024-03-28", "4.30"),
            rate("2024-03-29", "4.35"), // last <= end
            rate("2024-04-02", "4.40"), // after end, excluded
        ];
        let mid = latest_mid_on_or_before(&rates, "2024-03-31").expect("some");
        assert_eq!(mid, d("4.35"));
    }

    #[test]
    fn stock_is_none_when_every_mid_is_after_end() {
        let rates = vec![rate("2024-04-02", "4.40")];
        assert_eq!(latest_mid_on_or_before(&rates, "2024-03-31"), None);
        assert_eq!(latest_mid_on_or_before(&[], "2024-03-31"), None);
    }

    #[test]
    fn pln_amount_is_passed_through_unconverted() {
        let converted = convert_to_pln(
            d("123.45"),
            "PLN",
            &[],
            MeasureWindow::Flow,
            "2024-01-01",
            "2024-01-31",
        )
        .expect("pln passthrough never fails");
        assert_eq!(converted.value_pln, d("123.45"));
        assert_eq!(converted.basis, FxBasis::NativePln);
        assert_eq!(converted.mid, Decimal::ONE);
    }

    #[test]
    fn flow_conversion_multiplies_exactly_at_period_average() {
        let rates = vec![rate("2024-01-10", "4.0"), rate("2024-01-20", "4.5")];
        let converted = convert_to_pln(
            d("100"),
            "EUR",
            &rates,
            MeasureWindow::Flow,
            "2024-01-01",
            "2024-01-31",
        )
        .expect("some");
        // avg mid = 4.25, 100 * 4.25 = 425 exactly.
        assert_eq!(converted.value_pln, d("425"));
        assert_eq!(converted.basis, FxBasis::PeriodAverage);
    }

    #[test]
    fn stock_conversion_multiplies_at_latest_mid() {
        let rates = vec![rate("2024-03-28", "4.30"), rate("2024-03-29", "4.35")];
        let converted = convert_to_pln(
            d("10"),
            "USD",
            &rates,
            MeasureWindow::Stock,
            "2024-01-01",
            "2024-03-31",
        )
        .expect("some");
        assert_eq!(converted.value_pln, d("43.50"));
        assert_eq!(converted.basis, FxBasis::LatestOnOrBefore);
    }

    #[test]
    fn a_missing_rate_is_a_typed_error_never_a_pln_guess() {
        let err = convert_to_pln(
            d("100"),
            "EUR",
            &[],
            MeasureWindow::Flow,
            "2024-01-01",
            "2024-01-31",
        )
        .expect_err("missing rate must be an error");
        assert_eq!(err, FxError::MissingRate);
    }
}

#[cfg(test)]
mod proptests {
    //! Conversion invariants (ADR 0049): determinism, no-panic on empty/missing,
    //! flow-average and stock-latest basis correctness, and decimal-exact
    //! round-trips. Rates are generated as (ISO date, small Decimal mid).
    use super::*;
    use proptest::prelude::*;
    use rust_decimal::Decimal;

    /// A mid in a realistic NBP band, decimal-exact from integer thousandths.
    fn mid_strategy() -> impl Strategy<Value = Decimal> {
        (1i64..100_000).prop_map(|thousandths| Decimal::new(thousandths, 3))
    }

    /// An ISO date drawn from a fixed small calendar so ranges overlap.
    fn date_strategy() -> impl Strategy<Value = String> {
        (2024i32..2026, 1u32..13, 1u32..29).prop_map(|(y, m, d)| format!("{y:04}-{m:02}-{d:02}"))
    }

    fn rates_strategy() -> impl Strategy<Value = Vec<(String, Decimal)>> {
        proptest::collection::vec((date_strategy(), mid_strategy()), 0..40)
    }

    proptest! {
        #[test]
        fn period_average_never_panics_and_is_deterministic(
            rates in rates_strategy(),
            a in date_strategy(),
            b in date_strategy(),
        ) {
            let (start, end) = if a <= b { (a, b) } else { (b, a) };
            let first = period_average_mid(&rates, &start, &end);
            let second = period_average_mid(&rates, &start, &end);
            prop_assert_eq!(first, second);
        }

        #[test]
        fn latest_never_panics_and_is_deterministic(
            rates in rates_strategy(),
            end in date_strategy(),
        ) {
            let first = latest_mid_on_or_before(&rates, &end);
            prop_assert_eq!(first, latest_mid_on_or_before(&rates, &end));
        }

        #[test]
        fn stock_result_is_actually_the_max_date_within_bound(
            rates in rates_strategy(),
            end in date_strategy(),
        ) {
            if let Some(mid) = latest_mid_on_or_before(&rates, &end) {
                // The chosen mid must belong to some in-bound date, and no
                // in-bound date may be strictly greater than the chosen one's.
                let chosen_date = rates
                    .iter()
                    .filter(|(dt, m)| dt.as_str() <= end.as_str() && *m == mid)
                    .map(|(dt, _)| dt.clone())
                    .max()
                    .expect("chosen mid comes from an in-bound row");
                let max_in_bound = rates
                    .iter()
                    .filter(|(dt, _)| dt.as_str() <= end.as_str())
                    .map(|(dt, _)| dt.clone())
                    .max()
                    .expect("at least one in-bound row exists");
                prop_assert_eq!(chosen_date, max_in_bound);
            }
        }

        #[test]
        fn flow_average_is_within_the_min_max_of_in_range_mids(
            rates in rates_strategy(),
            a in date_strategy(),
            b in date_strategy(),
        ) {
            let (start, end) = if a <= b { (a, b) } else { (b, a) };
            let in_range: Vec<Decimal> = rates
                .iter()
                .filter(|(dt, _)| dt.as_str() >= start.as_str() && dt.as_str() <= end.as_str())
                .map(|(_, m)| *m)
                .collect();
            if let Some(avg) = period_average_mid(&rates, &start, &end) {
                let lo = *in_range.iter().min().expect("nonempty");
                let hi = *in_range.iter().max().expect("nonempty");
                prop_assert!(avg >= lo && avg <= hi, "avg {avg} not within [{lo}, {hi}]");
            }
        }

        #[test]
        fn conversion_is_exact_multiplication(
            amount in mid_strategy(),
            rates in rates_strategy().prop_filter("nonempty", |r| !r.is_empty()),
            end in date_strategy(),
        ) {
            // Stock path over the full generated calendar so a mid always resolves
            // when any row is <= end.
            let result = convert_to_pln(amount, "EUR", &rates, MeasureWindow::Stock, "2000-01-01", &end);
            if let Ok(converted) = result {
                prop_assert_eq!(converted.value_pln, amount * converted.mid);
            } else {
                // Only acceptable failure is a genuinely missing rate.
                prop_assert!(latest_mid_on_or_before(&rates, &end).is_none());
            }
        }

        #[test]
        fn pln_passthrough_never_fails_and_never_scales(
            amount in mid_strategy(),
            rates in rates_strategy(),
            a in date_strategy(),
            b in date_strategy(),
        ) {
            let (start, end) = if a <= b { (a, b) } else { (b, a) };
            let converted = convert_to_pln(amount, "PLN", &rates, MeasureWindow::Flow, &start, &end)
                .expect("pln never fails");
            prop_assert_eq!(converted.value_pln, amount);
            prop_assert_eq!(converted.basis, FxBasis::NativePln);
        }
    }
}
