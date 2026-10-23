//! Internationalization (docs/i18n.md): the data and the algorithms that `fmt.number`, `fmt.percent`, `fmt.date`, `fmt.time` and the plural
//! rules of `t(..)` run. The page runs the same data through the same steps written in JavaScript (`codegen-web/src/i18n.rs`); neither
//! calls `Intl`, because two engines disagree with each other (docs/i18n.md section 2.1). The data is `i18n_data.rs`, generated.

pub use crate::i18n_data::LANGS;

/// A piece of a date or time pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    Lit(&'static str),
    /// The day of the month; true: two digits.
    Day(bool),
    /// The month as a number; true: two digits.
    MonthNum(bool),
    /// The year; true: two digits.
    Year(bool),
    /// The month's name, as it is written in this pattern (a genitive in Russian).
    MonthName([&'static str; 12]),
    /// The name of the weekday, Sunday first.
    Weekday([&'static str; 7]),
    /// The hour: (two digits, twelve-hour clock).
    Hour(bool, bool),
    Minute,
    /// (before noon, after noon)
    DayPeriod(&'static str, &'static str),
}

#[derive(Debug)]
pub struct Lang {
    pub code: &'static str,
    pub rtl: bool,
    pub decimal: &'static str,
    pub group: &'static str,
    pub minus: &'static str,
    /// The fewest digits before the first separator for a separator to be written (1: `1,234`; 2: `1234`, `12.345`).
    pub min_group: u8,
    /// Groups of 3 then of 2 (`12,34,567`).
    pub indian: bool,
    pub pct_prefix: &'static str,
    pub pct_suffix: &'static str,
    pub date_short: &'static [Part],
    pub date_medium: &'static [Part],
    pub date_long: &'static [Part],
    pub date_full: &'static [Part],
    pub time: &'static [Part],
}

pub fn lang(code: &str) -> Option<&'static Lang> {
    LANGS.iter().find(|l| l.code == code)
}

pub fn is_known(code: &str) -> bool {
    lang(code).is_some()
}

/// The plural categories a language has, in the order of `PLURAL_ORDER`: a catalog gives exactly these (`other` always).
pub fn categories(code: &str) -> Option<&'static [&'static str]> {
    Some(match code {
        "en" | "de" | "hi" => &["one", "other"],
        "es" | "fr" | "pt" => &["one", "many", "other"],
        "ru" => &["one", "few", "many", "other"],
        "ar" => &["zero", "one", "two", "few", "many", "other"],
        "he" => &["one", "two", "other"],
        "ja" => &["other"],
        _ => return None,
    })
}

/// The plural category of a count, by its size (`Intl.PluralRules` answers `one` for -1). A count that is not a whole number is `other`. Checked against `Intl.PluralRules` for
/// 0 to 1000 and for large counts by scripts/check-i18n.js and `tests/i18n_corpus.json`.
pub fn plural(code: &str, n: f64) -> &'static str {
    let n = n.abs();
    if n.is_nan() || n.fract() != 0.0 || n >= 9_007_199_254_740_992.0 {
        return "other";
    }
    let n = n as u64;
    let (m10, m100) = (n % 10, n % 100);
    match code {
        "en" | "de" => {
            if n == 1 {
                "one"
            } else {
                "other"
            }
        }
        "hi" => {
            if n <= 1 {
                "one"
            } else {
                "other"
            }
        }
        "fr" | "es" | "pt" => {
            if n == 1 || (n == 0 && code != "es") {
                "one"
            } else if n != 0 && n.is_multiple_of(1_000_000) {
                "many"
            } else {
                "other"
            }
        }
        "ru" => {
            if m10 == 1 && m100 != 11 {
                "one"
            } else if (2..=4).contains(&m10) && !(12..=14).contains(&m100) {
                "few"
            } else {
                "many"
            }
        }
        "ar" => match n {
            0 => "zero",
            1 => "one",
            2 => "two",
            _ if (3..=10).contains(&m100) => "few",
            _ if (11..=99).contains(&m100) => "many",
            _ => "other",
        },
        "he" => match n {
            1 => "one",
            2 => "two",
            _ => "other",
        },
        _ => "other",
    }
}

/// The shortest digits of `a` (finite, at least 1e-6 and below 1e21) as an integer part and a fraction part, in decimal, the way a JavaScript
/// `String(a)` writes it. Rust's `Display` of an `f64` is the shortest digits that read back to the same number, and has no exponent.
fn decimal_parts(a: f64) -> (String, String) {
    let s = format!("{a}");
    match s.split_once('.') {
        Some((i, f)) => (i.to_string(), f.to_string()),
        None => (s, String::new()),
    }
}

/// Round the digits `int.frac` to `digits` places, half away from zero (what `Intl.NumberFormat` does with the shortest decimal of a double).
fn round_digits(int: &str, frac: &str, digits: usize) -> (String, String) {
    let mut all: Vec<u8> = int.bytes().chain(frac.bytes()).map(|b| b - b'0').collect();
    let keep = int.len() + digits;
    let up = all.get(keep).is_some_and(|d| *d >= 5);
    all.truncate(keep);
    while all.len() < keep {
        all.push(0);
    }
    if up {
        let mut i = all.len();
        loop {
            if i == 0 {
                all.insert(0, 1);
                break;
            }
            i -= 1;
            if all[i] == 9 {
                all[i] = 0;
            } else {
                all[i] += 1;
                break;
            }
        }
    }
    let int_len = all.len() - digits;
    let text: String = all.iter().map(|d| char::from(b'0' + d)).collect();
    (text[..int_len].to_string(), text[int_len..].to_string())
}

fn group_int(l: &Lang, int: &str) -> String {
    let n = int.len();
    if n < 3 + usize::from(l.min_group) {
        return int.to_string();
    }
    let mut out = String::new();
    let mut first = n - 3;
    out.insert_str(0, &int[first..]);
    while first > 0 {
        let step = if l.indian { 2 } else { 3 };
        let from = first.saturating_sub(step);
        out.insert_str(0, l.group);
        out.insert_str(0, &int[from..first]);
        first = from;
    }
    out
}

/// The digits of `a` as (integer, fraction); a value under 1e-6 is zero.
fn digits_of(a: f64) -> (String, String) {
    if a < 1e-6 {
        ("0".to_string(), String::new())
    } else {
        decimal_parts(a)
    }
}

/// Round to `d` places and write with the language's separators and sign.
fn write_decimal(l: &Lang, negative: bool, int: &str, frac: &str, d: usize) -> String {
    let (int, frac) = round_digits(int, frac, d);
    let mut out = String::new();
    if negative {
        out.push_str(l.minus);
    }
    out.push_str(&group_int(l, &int));
    if d > 0 {
        out.push_str(l.decimal);
        out.push_str(&frac);
    }
    out
}

/// `fmt.number(x, digits)`: exactly `digits` (0 to 6) places, grouped. "" when `x` is not finite, is 1e21 or more, or `digits` is out of range.
/// A value smaller than 1e-6 in size is written as zero (it cannot be told from zero at 6 places, and a JavaScript `String` writes it in
/// exponent form). The sign is kept for a negative value that rounds to zero, as `Intl.NumberFormat` does.
pub fn format_number(l: &Lang, x: f64, digits: f64) -> String {
    if !x.is_finite() || x.abs() >= 1e21 || digits.fract() != 0.0 || !(0.0..=6.0).contains(&digits)
    {
        return String::new();
    }
    let (int, frac) = digits_of(x.abs());
    write_decimal(l, x.is_sign_negative(), &int, &frac, digits as usize)
}

/// `fmt.percent(x)`: `x` times 100, no places. The point moves two places in the decimal digits of `x` (0.145 is 14.5, which rounds to 15;
/// the double product 14.499999999999998 would not).
pub fn format_percent(l: &Lang, x: f64) -> String {
    if !x.is_finite() || x.abs() >= 1e21 {
        return String::new();
    }
    let (int, frac) = digits_of(x.abs());
    let mut all = format!("{int}{frac}");
    let point = int.len() + 2;
    while all.len() < point {
        all.push('0');
    }
    let (i, f) = all.split_at(point);
    let i = i.trim_start_matches('0');
    let i = if i.is_empty() { "0" } else { i };
    let body = write_decimal(l, false, i, f, 0);
    let sign = if x.is_sign_negative() { l.minus } else { "" };
    format!("{sign}{}{body}{}", l.pct_prefix, l.pct_suffix)
}

/// The range a date is formatted in: the years 1000 to 9999.
const MIN_MS: f64 = -30_610_224_000_000.0;
const MAX_MS: f64 = 253_402_300_799_999.0;

struct Civil {
    year: i64,
    month: usize,
    day: i64,
    weekday: usize,
    hour: i64,
    minute: i64,
}

fn civil(ms: f64) -> Option<Civil> {
    if !ms.is_finite() || !(MIN_MS..=MAX_MS).contains(&ms) {
        return None;
    }
    let ms = ms.trunc() as i64;
    let days = ms.div_euclid(86_400_000);
    let rem = ms.rem_euclid(86_400_000);
    // days since 1970-01-01 to a civil date (Howard Hinnant's algorithm)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    Some(Civil {
        year,
        month: (month - 1) as usize,
        day,
        weekday: (days + 4).rem_euclid(7) as usize,
        hour: rem / 3_600_000,
        minute: rem % 3_600_000 / 60_000,
    })
}

fn two(n: i64, pad: bool) -> String {
    if pad {
        format!("{n:02}")
    } else {
        n.to_string()
    }
}

fn apply(parts: &[Part], c: &Civil) -> String {
    let mut out = String::new();
    for p in parts {
        match p {
            Part::Lit(s) => out.push_str(s),
            Part::Day(pad) => out.push_str(&two(c.day, *pad)),
            Part::MonthNum(pad) => out.push_str(&two(c.month as i64 + 1, *pad)),
            Part::Year(short) => {
                if *short {
                    out.push_str(&format!("{:02}", c.year % 100));
                } else {
                    out.push_str(&c.year.to_string());
                }
            }
            Part::MonthName(names) => out.push_str(names[c.month]),
            Part::Weekday(names) => out.push_str(names[c.weekday]),
            Part::Hour(pad, twelve) => {
                let h = if *twelve {
                    match c.hour % 12 {
                        0 => 12,
                        h => h,
                    }
                } else {
                    c.hour
                };
                out.push_str(&two(h, *pad));
            }
            Part::Minute => out.push_str(&format!("{:02}", c.minute)),
            Part::DayPeriod(am, pm) => out.push_str(if c.hour < 12 { am } else { pm }),
        }
    }
    out
}

/// `fmt.date(ms, style)` in UTC, for the years 1000 to 9999 ("" outside them); `style` is `short`, `medium`, `long` or `full`.
pub fn format_date(l: &Lang, ms: f64, style: &str) -> String {
    let parts = match style {
        "short" => l.date_short,
        "medium" => l.date_medium,
        "long" => l.date_long,
        "full" => l.date_full,
        _ => return String::new(),
    };
    civil(ms).map_or_else(String::new, |c| apply(parts, &c))
}

/// `fmt.time(ms)`: hours and minutes in UTC.
pub fn format_time(l: &Lang, ms: f64) -> String {
    civil(ms).map_or_else(String::new, |c| apply(l.time, &c))
}

pub const DATE_STYLES: [&str; 4] = ["short", "medium", "long", "full"];
