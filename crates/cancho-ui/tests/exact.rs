//! `exact(text, default)` (`docs/lists.md` section 11): a text key whose order is the order of the decimals the texts spell. The compiler's
//! side (the call is known, its arity, the helper is emitted only when used), the replay's side (the key, against an oracle that is exact integer
//! arithmetic, and against the corpus the emitted code is also checked on), and the example. The emitted code's own check is `scripts/check-exact.js`.
mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use cancho_ui_replay::exact_key;
use serde_json::Value;

fn run(args: &[&str]) -> (i32, String, String) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new(env!("CARGO_BIN_EXE_cancho-ui"))
        .args(args)
        .current_dir(root)
        .output()
        .expect("the cancho-ui binary runs");
    (
        out.status.code().expect("exited, not killed by a signal"),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn scratch(name: &str, text: &str) -> PathBuf {
    let dir = common::scratch_root().join(format!("cancho-ui-exact-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let path = dir.join(name);
    std::fs::write(&path, text).expect("a scratch file");
    path
}

fn p(path: &Path) -> &str {
    path.to_str().expect("UTF-8 temp path")
}

fn build_js(src: &str, name: &str) -> String {
    let file = scratch(name, src);
    let out = scratch(&format!("{name}.js"), "");
    let (code, _, stderr) = run(&["build", p(&file), "-o", p(&out)]);
    assert_eq!(code, 0, "{stderr}");
    std::fs::read_to_string(out).expect("the built program")
}

fn check_codes(src: &str, name: &str) -> Vec<String> {
    let file = scratch(name, src);
    let (code, stdout, _) = run(&["check", p(&file), "--output", "json"]);
    assert_eq!(code, 1, "{stdout}");
    let diags: Value = serde_json::from_str(&stdout).expect("diagnostics are JSON");
    diags
        .as_array()
        .expect("a list")
        .iter()
        .map(|d| d["code"].as_str().expect("a code").to_string())
        .collect()
}

// ---- the oracle: exact integer arithmetic, no doubles, no shared code with the key ----------------------------------------------------

const SCALE: u32 = 18;

/// The value of a decimal text with at most 18 integer digits and 18 fraction digits, as an integer scaled by 10^18.
fn scaled(text: &str) -> i128 {
    let (neg, rest) = match text.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, text),
    };
    let (int, frac) = rest.split_once('.').unwrap_or((rest, ""));
    let mut v: i128 = int.parse().expect("integer digits");
    v *= 10i128.pow(SCALE);
    let mut f = frac.to_string();
    while (f.len() as u32) < SCALE {
        f.push('0');
    }
    v += f.parse::<i128>().expect("fraction digits");
    if neg { -v } else { v }
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

fn digits(r: &mut Rng, n: usize) -> String {
    (0..n)
        .map(|_| char::from(b'0' + r.below(10) as u8))
        .collect()
}

/// A random decimal, and a second spelling of the same value.
fn decimal(r: &mut Rng) -> (String, String) {
    let ilen = [0usize, 1, 1, 2, 3, 5, 9, 15, 18][r.below(9) as usize];
    let flen = [0usize, 0, 1, 2, 3, 6, 12, 18][r.below(8) as usize];
    let mut int = digits(r, ilen);
    while int.len() > 1 && int.starts_with('0') {
        int.remove(0);
    }
    if int.is_empty() {
        int = "0".to_string();
    }
    let frac = digits(r, flen);
    let neg = r.below(2) == 0;
    let mut a = String::new();
    if neg {
        a.push('-');
    }
    a.push_str(&int);
    if !frac.is_empty() {
        a.push('.');
        a.push_str(&frac);
    }
    // the same value written with more trailing zeros, or `-0` for a negative zero
    let mut b = a.clone();
    if !b.contains('.') {
        b.push('.');
        b.push('0');
    } else {
        b.push('0');
    }
    (a, b)
}

#[test]
fn the_order_of_the_keys_is_the_order_of_the_numbers_and_equal_values_have_equal_keys() {
    let mut r = Rng(0x9E3779B97F4A7C15);
    let mut pairs = 0;
    for _ in 0..30_000 {
        let (a, a2) = decimal(&mut r);
        let (b, _) = decimal(&mut r);
        let (ka, ka2, kb) = (exact_key(&a, "0"), exact_key(&a2, "0"), exact_key(&b, "0"));
        assert_eq!(ka, ka2, "{a} and {a2} are one value");
        let want = scaled(&a).cmp(&scaled(&b));
        assert_eq!(ka.cmp(&kb), want, "{a} against {b}: {ka} against {kb}");
        pairs += 1;
    }
    assert_eq!(pairs, 30_000);
    // the edges by hand
    for (a, b) in [
        ("0", "-0"),
        ("0", "0.000"),
        ("-0.0", "0"),
        ("1.5", "1.50"),
        ("-1.5", "-1.500"),
    ] {
        assert_eq!(exact_key(a, "0"), exact_key(b, "0"), "{a} and {b}");
    }
    for (small, big) in [
        ("-1", "0"),
        ("0", "0.000000000000000001"),
        ("0.25", "0.5"),
        ("0.5", "0.51"),
        ("-0.51", "-0.5"),
        ("9", "10"),
        ("-10", "-9"),
    ] {
        assert!(
            exact_key(small, "0") < exact_key(big, "0"),
            "{small} before {big}"
        );
    }
}

#[test]
fn a_text_that_is_not_a_decimal_is_the_key_of_the_default_and_three_when_the_default_is_not_one() {
    for bad in [
        "", " ", " 1", "1 ", "+1", "1e3", "1E3", ".5", "5.", "007", "-", "--1", "1.2.3", "0x10",
        "NaN", "Infinity", "1,5", "-.5",
    ] {
        assert_eq!(
            exact_key(bad, "7"),
            exact_key("7", "0"),
            "{bad:?} is the default"
        );
        assert_eq!(
            exact_key(bad, "oops"),
            "3",
            "{bad:?} with a default that is not a decimal"
        );
    }
    let too_long = "1".repeat(1_000_000);
    assert_eq!(
        exact_key(&too_long, "0"),
        exact_key("0", "0"),
        "a million digits is not a decimal here"
    );
    assert!(
        exact_key("9", "oops").as_str() < "3",
        "every decimal sorts before 3"
    );
    assert!(
        exact_key("-1", "oops") < exact_key("0", "oops")
            && exact_key("0", "oops") < exact_key("1", "oops")
    );
}

#[test]
fn the_keys_of_the_corpus_are_what_the_replay_makes() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/exact_corpus.json");
    let cases = [
        "0",
        "-0",
        "0.0",
        "1",
        "-1",
        "1.5",
        "1.50",
        "-1.5",
        "-1.50",
        "0.5",
        "0.25",
        "0.51",
        "-0.5",
        "-0.51",
        "9",
        "10",
        "100",
        "-9",
        "-10",
        "-100",
        "9007199254740992",
        "9007199254740993",
        "-9007199254740993",
        "123456789012345678901234567890.123456789",
        "-123456789012345678901234567890.123456789",
        "0.000000000000000000000001",
        "1e3",
        "+1",
        "007",
        ".5",
        "5.",
        "",
        " ",
        "x",
    ];
    let mut fresh = Vec::new();
    for text in cases {
        for default in ["0", "oops"] {
            fresh.push(serde_json::json!({"text": text, "default": default, "key": exact_key(text, default)}));
        }
    }
    if std::env::var("UPDATE_EXACT_CORPUS").is_ok() {
        std::fs::write(
            &path,
            serde_json::to_string_pretty(&fresh).expect("json") + "\n",
        )
        .expect("write the corpus");
    }
    let committed: Value = serde_json::from_str(
        &std::fs::read_to_string(&path).expect("tests/exact_corpus.json is committed"),
    )
    .expect("the corpus is JSON");
    assert_eq!(
        committed,
        Value::Array(fresh),
        "the committed corpus is what the replay makes (UPDATE_EXACT_CORPUS=1 rewrites it)"
    );
}

// ---- the compiler -------------------------------------------------------------------------------------------------------------------------

const EXACT: &str = include_str!("../../../examples/exact/exact.cui");

#[test]
fn the_helper_is_emitted_for_a_program_that_calls_exact_and_for_no_other() {
    let js = build_js(EXACT, "exact.cui");
    assert!(js.contains("function __cui_exact(text, fallback)"), "{js}");
    assert!(js.contains("__cui_exact(x, \"0\")"), "{js}");
    let counter = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/counter/counter.cui"),
    )
    .expect("the counter example");
    assert!(!build_js(&counter, "counter.cui").contains("__cui_exact"));
}

#[test]
fn exact_takes_exactly_two_arguments_and_is_a_known_call() {
    for (name, call) in [
        ("one.cui", "exact(x)"),
        ("three.cui", "exact(x, \"0\", \"1\")"),
    ] {
        let src = format!(
            "component A {{\n    state rows: List[String] = [];\n    on Go {{ rows = sort(rows, x => {call}); }}\n    view {{ Text {{ \"x\" }} }}\n}}\n"
        );
        assert_eq!(
            check_codes(&src, name),
            ["builtin-shape-mismatch"],
            "{call}"
        );
    }
}

#[test]
fn the_example_replays_to_the_exact_order_and_the_same_program_with_num_does_not() {
    let (code, stdout, stderr) = run(&[
        "test",
        "examples/exact/exact.cui",
        "examples/exact/exact.trace.json",
    ]);
    assert_eq!(code, 0, "{stdout}{stderr}");
    assert!(stdout.contains("PASS"), "{stdout}");
    // the negative control: keyed by `num`, the two integers past 2^53 collide and the order is wrong
    let with_num = scratch("num.cui", &EXACT.replace("exact(x, \"0\")", "num(x, 0)"));
    let (code, stdout, _) = run(&[
        "test",
        p(&with_num),
        "examples/exact/exact.trace.json",
        "--output",
        "json",
    ]);
    assert_eq!(code, 4, "{stdout}");
    let report: Value = serde_json::from_str(&stdout).expect("a JSON report");
    assert_eq!(report["failures"][0]["rule"], "test-state-mismatch");
    let actual: Vec<String> = report["failures"][0]["actual"]
        .as_array()
        .expect("the ordered list")
        .iter()
        .map(|v| v.as_str().expect("text").to_string())
        .collect();
    let at = |s: &str| actual.iter().position(|v| v == s).expect("present");
    assert!(
        at("9007199254740993") < at("9007199254740992"),
        "num leaves them in input order: {actual:?}"
    );
}
