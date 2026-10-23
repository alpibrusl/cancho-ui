//! A chart in a trace (docs/charts.md section 4): replay does not draw, so it cannot see a chart's geometry; it reports the data and the
//! text alternative, which the page writes as the chart's accessible name. `tests/chart_corpus.json` is run against both this function
//! and the page's `__cui_chart_summary`.

use crate::eval::math_builtin;
use crate::value::js_number;

/// The most values a chart draws (the last ones),.
pub const MAX_POINTS: usize = 1000;

/// A number as the alternative writes it: rounded to two decimals the way `Math.round` does (halves toward +Infinity), then printed as
/// JavaScript prints it. A number of 1e21 or more is `?` (JavaScript would switch to an exponent; neither side imitates the other).
pub fn number_text(x: f64) -> String {
    let scaled = math_builtin("round", &[x * 100.0]).unwrap_or(f64::NAN) / 100.0;
    js_number(scaled).unwrap_or_else(|| "?".to_string())
}

/// `line chart of 5 values: minimum 1 °C, maximum 5 °C, last 5 °C`, or `bar chart with no values`, with `, the last 1000 of 5000 shown`
/// when the list was longer than the chart draws.
pub fn summary(kind: &str, values: &[f64], unit: &str) -> String {
    let total = values.len();
    if total == 0 {
        return format!("{kind} chart with no values");
    }
    let drawn = &values[total.saturating_sub(MAX_POINTS)..];
    let n = drawn.len();
    let with_unit = |x: f64| {
        if unit.is_empty() {
            number_text(x)
        } else {
            format!("{} {unit}", number_text(x))
        }
    };
    let min = drawn.iter().copied().fold(f64::INFINITY, f64::min);
    let max = drawn.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let last = drawn[n - 1];
    let mut s = format!(
        "{kind} chart of {n} value{}: minimum {}, maximum {}, last {}",
        if n == 1 { "" } else { "s" },
        with_unit(min),
        with_unit(max),
        with_unit(last)
    );
    if total > n {
        s.push_str(&format!(", the last {n} of {total} shown"));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers() {
        assert_eq!(number_text(2.005), "2.01");
        assert_eq!(number_text(1.005), "1");
        assert_eq!(number_text(0.1 + 0.2), "0.3");
        assert_eq!(number_text(-0.004), "0");
        assert_eq!(number_text(-2.5), "-2.5");
        assert_eq!(number_text(2.675), "2.68");
        assert_eq!(number_text(1e22), "?");
        assert_eq!(number_text(1_000_000.0), "1000000");
    }

    #[test]
    fn sentences() {
        assert_eq!(summary("line", &[], ""), "line chart with no values");
        assert_eq!(
            summary("bar", &[5.0], ""),
            "bar chart of 1 value: minimum 5, maximum 5, last 5"
        );
        assert_eq!(
            summary("line", &[1.0, 2.0, 3.0], "°C"),
            "line chart of 3 values: minimum 1 °C, maximum 3 °C, last 3 °C"
        );
        let many: Vec<f64> = (0..1500).map(f64::from).collect();
        assert_eq!(
            summary("line", &many, ""),
            "line chart of 1000 values: minimum 500, maximum 1499, last 1499, the last 1000 of 1500 shown"
        );
    }
}
