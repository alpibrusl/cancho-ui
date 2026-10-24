//! `theme { .. }` (docs/theme.md): the colours of a program in two schemes, checked for contrast, and its fonts. The corpus
//! (`tests/theme_corpus.json`) is the palette of the cancho-www home page at commit 0579a80, vendored: nothing here reads the network or
//! that repository. The contrast numbers in it are written by `scripts/theme-oracle.js`, which does not share code with the compiler; the
//! tests compare the compiler with them. `scripts/check-theme.js` runs the same programs in Chromium.
mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

const CORPUS: &str = include_str!("../../../tests/theme_corpus.json");
const FONT: &[u8] =
    include_bytes!("../../../examples/theme/fonts/jetbrains-mono-latin-wght-normal.woff2");
const FONT_FILE: &str = "fonts/jetbrains-mono-latin-wght-normal.woff2";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn run_in(dir: Option<&Path>, args: &[&str]) -> (i32, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_cancho-ui"))
        .args(args)
        .current_dir(dir.map_or_else(root, Path::to_path_buf))
        .output()
        .expect("the cancho-ui binary runs");
    (
        out.status.code().expect("exited, not killed by a signal"),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn run(args: &[&str]) -> (i32, String, String) {
    run_in(None, args)
}

/// A scratch directory holding the font, so a theme can name `fonts/..` as a program's directory would hold it.
fn scratch(tag: &str) -> PathBuf {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir =
        common::scratch_root().join(format!("cancho-ui-theme-{tag}-{}-{n}", std::process::id()));
    std::fs::create_dir_all(dir.join("fonts")).expect("a scratch directory");
    std::fs::write(dir.join(FONT_FILE), FONT).expect("the font");
    dir
}

fn corpus() -> Value {
    serde_json::from_str(CORPUS).expect("the corpus is JSON")
}

fn tokens(c: &Value) -> Vec<String> {
    c["tokens"]
        .as_array()
        .expect("tokens")
        .iter()
        .map(|t| t.as_str().expect("token").to_string())
        .collect()
}

/// One scheme block: `(token, value)` lines in order, with a name.
type Scheme = (String, Vec<(String, String)>);

fn schemes_of(c: &Value) -> Vec<Scheme> {
    ["light", "dark"]
        .iter()
        .map(|s| {
            let lines = tokens(c)
                .into_iter()
                .map(|t| {
                    let v = c["palette"][*s][t.as_str()]
                        .as_str()
                        .expect("colour")
                        .to_string();
                    (t, v)
                })
                .collect();
            (s.to_string(), lines)
        })
        .collect()
}

fn scheme_text(name: &str, lines: &[(String, String)]) -> String {
    let body: String = lines
        .iter()
        .map(|(k, v)| format!("        {k}: \"{v}\";\n"))
        .collect();
    format!("    {name} {{\n{body}    }}\n")
}

fn font_text(extra: &str) -> String {
    let sha = sha256_of(FONT);
    format!(
        "    font body {{\n        family: \"JetBrains Mono\";\n        file: \"{FONT_FILE}\";\n        sha256: \"{sha}\";\n        licence: \"OFL-1.1\";\n{extra}    }}\n"
    )
}

/// SHA-256 by the system's `sha256sum`: not the compiler's own implementation.
fn sha256_of(bytes: &[u8]) -> String {
    use std::io::Write;
    let mut child = Command::new("sha256sum")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("sha256sum is installed");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(bytes)
        .expect("write");
    let out = child.wait_with_output().expect("sha256sum runs");
    String::from_utf8_lossy(&out.stdout)[..64].to_string()
}

const APP: &str = "component Hello {\n    state n: Int = 0;\n    on Up { n = n + 1; }\n    view { Column { Text { \"n = {n}\" } Button { label: \"Up\" on: Up } TextField { label: \"Name\" value: \"\" on: Up } } }\n}\n";

fn program(theme_body: &str) -> String {
    format!("theme {{\n{theme_body}}}\n\n{APP}")
}

fn palette_program(c: &Value, fonts: &str) -> String {
    let body: String = schemes_of(c)
        .iter()
        .map(|(n, l)| scheme_text(n, l))
        .collect();
    program(&format!("{body}{fonts}"))
}

fn write(dir: &Path, name: &str, text: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, text).expect("a program");
    path
}

fn check(dir: &Path, text: &str) -> (i32, Vec<Value>) {
    let file = write(dir, "p.cui", text);
    let (code, out, err) = run_in(
        Some(dir),
        &["check", file.to_str().expect("utf8"), "--output", "json"],
    );
    let diags: Vec<Value> = if out.trim().is_empty() {
        Vec::new()
    } else {
        serde_json::from_str(&out)
            .unwrap_or_else(|e| panic!("diagnostics are JSON ({e}): {out} {err}"))
    };
    (code, diags)
}

fn only_code(diags: &[Value]) -> String {
    assert_eq!(diags.len(), 1, "{diags:?}");
    diags[0]["code"].as_str().expect("code").to_string()
}

fn message(diags: &[Value]) -> String {
    diags[0]["message"].as_str().expect("message").to_string()
}

/// A corpus case applied to the base schemes and written as a program.
fn case_program(c: &Value, case: &Value) -> String {
    let mut schemes = schemes_of(c);
    if let Some(set) = case["set"].as_object() {
        for (k, v) in set {
            let (s, t) = k.split_once('.').expect("scheme.token");
            let lines = &mut schemes.iter_mut().find(|x| x.0 == s).expect("scheme").1;
            lines.iter_mut().find(|l| l.0 == t).expect("token").1 =
                v.as_str().expect("value").to_string();
        }
    }
    if let Some(add) = case["add"].as_object() {
        for (k, v) in add {
            let (s, t) = k.split_once('.').expect("scheme.token");
            schemes
                .iter_mut()
                .find(|x| x.0 == s)
                .expect("scheme")
                .1
                .push((t.to_string(), v.as_str().expect("value").to_string()));
        }
    }
    if let Some(dup) = case["dup"].as_object() {
        for (k, v) in dup {
            let (s, t) = k.split_once('.').expect("scheme.token");
            schemes
                .iter_mut()
                .find(|x| x.0 == s)
                .expect("scheme")
                .1
                .push((t.to_string(), v.as_str().expect("value").to_string()));
        }
    }
    if let Some(drop) = case["drop"].as_array() {
        for k in drop {
            let (s, t) = k
                .as_str()
                .expect("key")
                .split_once('.')
                .expect("scheme.token");
            schemes
                .iter_mut()
                .find(|x| x.0 == s)
                .expect("scheme")
                .1
                .retain(|l| l.0 != t);
        }
    }
    if let Some(rename) = case["rename"].as_object() {
        for (from, to) in rename {
            schemes.iter_mut().find(|x| x.0 == *from).expect("scheme").0 =
                to.as_str().expect("name").to_string();
        }
    }
    if let Some(gone) = case["drop_scheme"].as_str() {
        schemes.retain(|x| x.0 != gone);
    }
    let body: String = schemes.iter().map(|(n, l)| scheme_text(n, l)).collect();
    program(&format!("{body}{}", font_text("")))
}

#[test]
fn the_palette_of_the_home_page_passes_and_its_ratios_are_the_oracles() {
    // the compiler's arithmetic against the oracle's numbers, pair by pair, to 3 decimals
    let c = corpus();
    for scheme in ["light", "dark"] {
        for pair in c["pairs"].as_array().expect("pairs") {
            let (fg, bg) = (pair[0].as_str().expect("fg"), pair[1].as_str().expect("bg"));
            let a =
                cancho_ui_ir::theme::parse_hex(c["palette"][scheme][fg].as_str().expect("colour"))
                    .expect("hex");
            let b =
                cancho_ui_ir::theme::parse_hex(c["palette"][scheme][bg].as_str().expect("colour"))
                    .expect("hex");
            let got = cancho_ui_ir::theme::contrast_ratio(a, b);
            let want = c["ratios"][scheme][format!("{fg}/{bg}")]
                .as_f64()
                .expect("the oracle's ratio");
            assert!(
                (got - want).abs() < 1e-3,
                "{scheme} {fg}/{bg}: {got} against the oracle's {want}"
            );
        }
    }
    // the compiler's own list of pairs is the corpus's
    let ours: Vec<(String, String)> = cancho_ui_ir::theme::PAIRS
        .iter()
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect();
    let theirs: Vec<(String, String)> = c["pairs"]
        .as_array()
        .expect("pairs")
        .iter()
        .map(|p| {
            (
                p[0].as_str().expect("fg").to_string(),
                p[1].as_str().expect("bg").to_string(),
            )
        })
        .collect();
    assert_eq!(ours, theirs);
    assert_eq!(
        cancho_ui_ir::theme::TOKENS
            .iter()
            .map(|t| t.to_string())
            .collect::<Vec<_>>(),
        tokens(&c)
    );
    // and the program that holds it checks
    let dir = scratch("palette");
    let (code, diags) = check(&dir, &palette_program(&c, &font_text("")));
    assert_eq!((code, diags.len()), (0, 0), "{diags:?}");
}

#[test]
fn every_case_of_the_corpus_is_refused_by_its_tag_or_accepted() {
    let c = corpus();
    let dir = scratch("cases");
    for case in c["cases"].as_array().expect("cases") {
        let name = case["name"].as_str().expect("name");
        let (exit, diags) = check(&dir, &case_program(&c, case));
        match case["code"].as_str() {
            None => assert_eq!((exit, diags.len()), (0, 0), "{name}: {diags:?}"),
            Some(code) => {
                assert_eq!(exit, 1, "{name}");
                assert_eq!(only_code(&diags), code, "{name}: {diags:?}");
                if code == "theme-contrast" {
                    // the message names the pair, the scheme and the ratio the oracle computed, and no other pair
                    let want: Vec<&str> = case["expect_fails"]
                        .as_array()
                        .expect("the oracle's failing pairs")
                        .iter()
                        .map(|v| v.as_str().expect("text"))
                        .collect();
                    let got = message(&diags);
                    let got: Vec<&str> = got
                        .split("; ")
                        .map(|s| s.split(" (needs").next().expect("part"))
                        .collect();
                    assert_eq!(got, want, "{name}");
                }
            }
        }
    }
}

#[test]
fn moving_muted_to_9a9a9a_is_a_contrast_refusal_naming_the_pair() {
    let c = corpus();
    let case = c["cases"]
        .as_array()
        .expect("cases")
        .iter()
        .find(|x| x["name"].as_str().is_some_and(|n| n.contains("#9a9a9a")))
        .expect("the case");
    let dir = scratch("muted");
    let (code, diags) = check(&dir, &case_program(&c, case));
    assert_eq!(code, 1);
    assert_eq!(only_code(&diags), "theme-contrast");
    let m = message(&diags);
    assert!(
        m.contains("light: muted on bg is 2.55:1") && m.contains("needs 4.5:1"),
        "{m}"
    );
}

#[test]
fn a_ratio_is_cut_so_that_a_failure_never_reads_as_the_limit() {
    // #777777 on white is 4.4776:1: refused, and shown as 4.47, never 4.48 or 4.50
    let c = corpus();
    let case = c["cases"]
        .as_array()
        .expect("cases")
        .iter()
        .find(|x| x["name"].as_str().is_some_and(|n| n.contains("#777777")))
        .expect("the case");
    let dir = scratch("cut");
    let (_, diags) = check(&dir, &case_program(&c, case));
    assert!(
        message(&diags).contains("fg on bg is 4.47:1"),
        "{}",
        message(&diags)
    );
}

#[test]
fn a_theme_is_one_per_file_and_one_per_program() {
    let c = corpus();
    let dir = scratch("dup");
    let one = palette_program(&c, &font_text(""));
    let body: String = schemes_of(&c)
        .iter()
        .map(|(n, l)| scheme_text(n, l))
        .collect();
    let two = format!("theme {{\n{body}}}\n{one}");
    let (code, diags) = check(&dir, &two);
    assert_eq!((code, only_code(&diags).as_str()), (1, "theme-duplicate"));
    // the entry file and an imported module each with one
    write(
        &dir,
        "lib.cui",
        &format!(
            "module lib;\n\ntheme {{\n{body}}}\n\ncomponent Inner {{\n    view {{ Text {{ \"inner\" }} }}\n}}\n"
        ),
    );
    let entry = format!("import lib.Inner;\n\n{one}");
    let (code, diags) = check(&dir, &entry);
    assert_eq!((code, only_code(&diags).as_str()), (1, "theme-duplicate"));
}

#[test]
fn fonts_are_files_of_the_program_with_a_licence_and_a_matching_hash() {
    let c = corpus();
    let dir = scratch("fonts");
    let schemes: String = schemes_of(&c)
        .iter()
        .map(|(n, l)| scheme_text(n, l))
        .collect();
    let with = |font: &str| program(&format!("{schemes}{font}"));
    let sha = sha256_of(FONT);
    let font = |family: &str, file: &str, hash: &str, licence: &str, role: &str| {
        let licence_line = if licence.is_empty() {
            String::new()
        } else {
            format!("        licence: \"{licence}\";\n")
        };
        let family_line = if family.is_empty() {
            String::new()
        } else {
            format!("        family: \"{family}\";\n")
        };
        format!(
            "    font {role} {{\n{family_line}        file: \"{file}\";\n        sha256: \"{hash}\";\n{licence_line}    }}\n"
        )
    };
    let cases: Vec<(&str, String, &str)> = vec![
        (
            "a font that is complete",
            font("JetBrains Mono", FONT_FILE, &sha, "OFL-1.1", "body"),
            "",
        ),
        (
            "an https address",
            font(
                "X",
                "https://fonts.example/x.woff2",
                &sha,
                "OFL-1.1",
                "body",
            ),
            "font-remote",
        ),
        (
            "a protocol-relative address",
            font("X", "//fonts.example/x.woff2", &sha, "OFL-1.1", "body"),
            "font-remote",
        ),
        (
            "a data address",
            font("X", "data:font/woff2;base64,AAAA", &sha, "OFL-1.1", "body"),
            "font-remote",
        ),
        (
            "an http address in capitals",
            font("X", "HTTP://fonts.example/x.woff2", &sha, "OFL-1.1", "body"),
            "font-remote",
        ),
        (
            "no licence",
            font("X", FONT_FILE, &sha, "", "body"),
            "font-licence-missing",
        ),
        (
            "a licence that is not an identifier",
            font("X", FONT_FILE, &sha, "free as in beer!", "body"),
            "font-licence-missing",
        ),
        (
            "a hash that is not the file's",
            font("X", FONT_FILE, &"0".repeat(64), "OFL-1.1", "body"),
            "font-hash-mismatch",
        ),
        (
            "a hash that is not a hash",
            font("X", FONT_FILE, "abc", "OFL-1.1", "body"),
            "font-hash-mismatch",
        ),
        (
            "a file that does not exist",
            font("X", "fonts/none.woff2", &sha, "OFL-1.1", "body"),
            "font-file-missing",
        ),
        (
            "no family",
            font("", FONT_FILE, &sha, "OFL-1.1", "body"),
            "font-incomplete",
        ),
        (
            "a family that is only a quote in the middle",
            font("A\\\"B", FONT_FILE, &sha, "OFL-1.1", "body"),
            "font-incomplete",
        ),
        (
            "a family with a quote",
            font(
                "X\\\"; } body { display:none",
                FONT_FILE,
                &sha,
                "OFL-1.1",
                "body",
            ),
            "font-incomplete",
        ),
        (
            "a file that climbs out of the directory",
            font("X", "../x.woff2", &sha, "OFL-1.1", "body"),
            "font-incomplete",
        ),
        (
            "an absolute file",
            font("X", "/etc/x.woff2", &sha, "OFL-1.1", "body"),
            "font-incomplete",
        ),
        (
            "a file that is not woff2",
            font("X", "fonts/x.ttf", &sha, "OFL-1.1", "body"),
            "font-incomplete",
        ),
        (
            "a role that does not exist",
            font("X", FONT_FILE, &sha, "OFL-1.1", "title"),
            "theme-token-unknown",
        ),
    ];
    for (name, text, want) in cases {
        let (code, diags) = check(&dir, &with(&text));
        if want.is_empty() {
            assert_eq!((code, diags.len()), (0, 0), "{name}: {diags:?}");
        } else {
            assert_eq!(code, 1, "{name}");
            assert_eq!(only_code(&diags), want, "{name}: {diags:?}");
        }
    }
    // a font key that does not exist, and a weight that is not one
    let extra = |line: &str| {
        format!(
            "    font body {{\n        family: \"X\";\n        file: \"{FONT_FILE}\";\n        sha256: \"{sha}\";\n        licence: \"OFL-1.1\";\n{line}    }}\n"
        )
    };
    let (_, d) = check(&dir, &with(&extra("        src: \"x\";\n")));
    assert_eq!(only_code(&d), "theme-token-unknown");
    let (_, d) = check(&dir, &with(&extra("        weight: \"bold\";\n")));
    assert_eq!(only_code(&d), "font-incomplete");
    let (c0, d) = check(&dir, &with(&extra("        weight: \"200 800\";\n")));
    assert_eq!((c0, d.len()), (0, 0), "{d:?}");
}

#[test]
fn a_font_that_is_the_wrong_file_is_refused_by_the_hash_the_message_names() {
    let c = corpus();
    let dir = scratch("tamper");
    let text = palette_program(&c, &font_text(""));
    let (code, _) = check(&dir, &text);
    assert_eq!(code, 0);
    // one byte of the file changes: the program is not the program any more
    let mut bytes = FONT.to_vec();
    bytes[100] ^= 1;
    std::fs::write(dir.join(FONT_FILE), &bytes).expect("the changed font");
    let (code, diags) = check(&dir, &text);
    assert_eq!(code, 1);
    assert_eq!(only_code(&diags), "font-hash-mismatch");
    assert!(
        message(&diags).contains(&sha256_of(&bytes)),
        "{}",
        message(&diags)
    );
}

#[test]
fn a_themed_page_has_its_stylesheet_its_font_and_makes_no_other_address() {
    let c = corpus();
    let dir = scratch("page");
    let file = write(&dir, "p.cui", &palette_program(&c, &font_text("")));
    let out = dir.join("out");
    let (code, _, err) = run_in(
        Some(&dir),
        &[
            "build",
            file.to_str().expect("utf8"),
            "--page",
            "-o",
            out.to_str().expect("utf8"),
        ],
    );
    assert_eq!(code, 0, "{err}");
    let html = std::fs::read_to_string(out.join("index.html")).expect("the page");
    assert!(html.contains("<style id=\"cui-theme\">"), "{html}");
    assert!(
        html.contains("@media (prefers-color-scheme: dark) { :root { --cui-bg: #0b1016;"),
        "the dark block is written from `dark`"
    );
    assert!(html.contains("--cui-bg: #f7f4eb;"));
    assert!(
        html.contains(&format!("@font-face {{ font-family: \"JetBrains Mono\"; font-style: normal; font-display: swap; src: url(\"{FONT_FILE}\") format(\"woff2\"); }}")),
        "{html}"
    );
    // the font was copied, byte for byte
    assert_eq!(
        std::fs::read(out.join(FONT_FILE)).expect("the copied font"),
        FONT
    );
    // no address to another host anywhere in the page or its script
    for f in ["index.html", "app.js"] {
        let text = std::fs::read_to_string(out.join(f)).expect("file");
        for bad in ["http://", "https://", "//fonts", "@import"] {
            assert!(
                !text.replace("http://www.w3.org/2000/svg", "").contains(bad),
                "{f} has {bad}"
            );
        }
    }
    // `--pwa` precaches the font too
    let pwa = dir.join("pwa");
    let (code, _, err) = run_in(
        Some(&dir),
        &[
            "build",
            file.to_str().expect("utf8"),
            "--pwa",
            "-o",
            pwa.to_str().expect("utf8"),
        ],
    );
    assert_eq!(code, 0, "{err}");
    let sw = std::fs::read_to_string(pwa.join("sw.js")).expect("the worker");
    assert!(sw.contains(&format!("./{FONT_FILE}")), "{sw}");
}

#[test]
fn a_program_without_a_theme_is_emitted_as_it_was() {
    // the same components with and without a theme: the script is the same bytes, the page has no theme block
    let c = corpus();
    let dir = scratch("same");
    let themed = write(&dir, "themed.cui", &palette_program(&c, &font_text("")));
    let plain = write(&dir, "plain.cui", APP);
    let (a, ja, _) = run_in(Some(&dir), &["build", themed.to_str().expect("utf8")]);
    let (b, jb, _) = run_in(Some(&dir), &["build", plain.to_str().expect("utf8")]);
    assert_eq!((a, b), (0, 0));
    // the emitted module names its source file in a comment; compare without that line
    let strip = |s: &str| {
        s.lines()
            .filter(|l| !l.contains("themed.cui") && !l.contains("plain.cui"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert_eq!(
        strip(&ja),
        strip(&jb),
        "replay and the page read the components, never the theme"
    );
    let out = dir.join("plainout");
    let (code, _, _) = run_in(
        Some(&dir),
        &[
            "build",
            plain.to_str().expect("utf8"),
            "--page",
            "-o",
            out.to_str().expect("utf8"),
        ],
    );
    assert_eq!(code, 0);
    let html = std::fs::read_to_string(out.join("index.html")).expect("page");
    assert!(
        !html.contains("cui-theme") && !html.contains("@font-face") && !html.contains("--cui-bg"),
        "{html}"
    );
}

#[test]
fn a_theme_has_no_authority_and_does_not_touch_replay() {
    let c = corpus();
    let dir = scratch("authority");
    let themed = write(&dir, "themed.cui", &palette_program(&c, &font_text("")));
    let plain = write(&dir, "plain.cui", APP);
    let effects = |f: &Path| {
        run_in(
            Some(&dir),
            &["inspect", "effects", f.to_str().expect("utf8"), "Hello"],
        )
    };
    let (a, ea, _) = effects(&themed);
    let (b, eb, _) = effects(&plain);
    assert_eq!((a, b), (0, 0));
    assert_eq!(
        ea, eb,
        "`inspect effects` is the same with and without a theme"
    );
    // the IR of the components is the same; only the theme is added to the file
    let ir = |f: &Path| -> Value {
        let (_, out, _) = run_in(Some(&dir), &["inspect", "ir", f.to_str().expect("utf8")]);
        serde_json::from_str(&out).expect("IR JSON")
    };
    let (ti, pi) = (ir(&themed), ir(&plain));
    // the components are the same; only where they sit in the file differs (the theme is above them), so spans are left out
    fn unspanned(v: &Value) -> Value {
        match v {
            Value::Object(m) => Value::Object(
                m.iter()
                    .filter(|(k, _)| *k != "span")
                    .map(|(k, v)| (k.clone(), unspanned(v)))
                    .collect(),
            ),
            Value::Array(a) => Value::Array(a.iter().map(unspanned).collect()),
            other => other.clone(),
        }
    }
    assert_eq!(unspanned(&ti["components"]), unspanned(&pi["components"]));
    assert!(
        pi.get("theme").is_none()
            && ti["theme"]["schemes"]
                .as_array()
                .is_some_and(|s| s.len() == 2)
    );
    // the example with a theme replays its trace
    let (code, out, _) = run(&[
        "test",
        "examples/theme/theme.cui",
        "examples/theme/theme.trace.json",
    ]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("PASS"), "{out}");
}

#[test]
fn diff_reports_a_theme_change_as_its_own_line_and_authority_stays() {
    let c = corpus();
    let dir = scratch("diff");
    let base = write(&dir, "base.cui", &palette_program(&c, &font_text("")));
    // light.accent darker, still passing; the font's licence changed
    let mut schemes = schemes_of(&c);
    schemes[0]
        .1
        .iter_mut()
        .find(|l| l.0 == "accent")
        .expect("accent")
        .1 = "#0f3d6e".to_string();
    let body: String = schemes.iter().map(|(n, l)| scheme_text(n, l)).collect();
    let cand = write(
        &dir,
        "cand.cui",
        &program(&format!(
            "{body}{}",
            font_text("").replace("OFL-1.1", "OFL-1.0")
        )),
    );
    let (code, human, err) = run_in(
        Some(&dir),
        &[
            "diff",
            base.to_str().expect("utf8"),
            cand.to_str().expect("utf8"),
        ],
    );
    assert_eq!(
        code, 5,
        "{human}{err}: a theme change is a change (exit 5, as for any change)"
    );
    assert!(
        human.contains("~ theme light.accent: #185fa5 -> #0f3d6e"),
        "{human}"
    );
    assert!(
        human.contains("~ theme font.body.licence: OFL-1.1 -> OFL-1.0"),
        "{human}"
    );
    assert!(human.contains("authority: unchanged"), "{human}");
    let (_, js, _) = run_in(
        Some(&dir),
        &[
            "diff",
            base.to_str().expect("utf8"),
            cand.to_str().expect("utf8"),
            "--output",
            "json",
        ],
    );
    let v: Value = serde_json::from_str(&js).expect("JSON");
    assert_eq!(v["authority"], "unchanged");
    assert_eq!(v["change_count"], 2);
    assert_eq!(v["theme"].as_array().map(Vec::len), Some(2));
    assert_eq!(v["theme"][0]["setting"], "font.body.licence");
    // the same theme twice: no change and no `theme` key at all
    let (code, js, _) = run_in(
        Some(&dir),
        &[
            "diff",
            base.to_str().expect("utf8"),
            base.to_str().expect("utf8"),
            "--output",
            "json",
        ],
    );
    assert_eq!(code, 0);
    let v: Value = serde_json::from_str(&js).expect("JSON");
    assert!(v.get("theme").is_none(), "{v}");
    // adding a theme to a program with none is reported, as additions
    let plain = write(&dir, "plain.cui", APP);
    let (_, human, _) = run_in(
        Some(&dir),
        &[
            "diff",
            plain.to_str().expect("utf8"),
            base.to_str().expect("utf8"),
        ],
    );
    assert!(
        human.contains("+ theme light.bg = #f7f4eb") && human.contains("+ theme dark.bg = #0b1016"),
        "{human}"
    );
    assert!(human.contains("authority: unchanged"), "{human}");
}

/// The properties a theme's stylesheet may set: colours, a font, and the colour of the focus ring. Nothing about size, position, display,
/// visibility or the outline's width and style.
const ALLOWED: &[&str] = &[
    "background",
    "color",
    "border-color",
    "border-bottom-color",
    "font-family",
    "font-style",
    "font-display",
    "font-weight",
    "src",
    "outline",
    "outline-offset",
];

fn properties(css: &str) -> Vec<String> {
    let mut out = Vec::new();
    for rule in css.split('}') {
        let Some((_, body)) = rule.rsplit_once('{') else {
            continue;
        };
        for decl in body.split(';') {
            if let Some((k, _)) = decl.split_once(':') {
                let k = k.trim();
                if !k.is_empty() && !k.starts_with("--") {
                    out.push(k.to_string());
                }
            }
        }
    }
    out
}

#[test]
fn no_token_can_remove_the_44_px_targets_or_the_focus_ring() {
    let c = corpus();
    let dir = scratch("tokens");
    let mut built = 0;
    let mut refused = 0;
    let mut per_token_built = std::collections::BTreeMap::new();
    for token in tokens(&c) {
        for scheme in ["light", "dark"] {
            for value in ["#000000", "#ffffff", "#ff00ff", "#808080"] {
                let mut schemes = schemes_of(&c);
                let lines = &mut schemes
                    .iter_mut()
                    .find(|x| x.0 == scheme)
                    .expect("scheme")
                    .1;
                lines.iter_mut().find(|l| l.0 == token).expect("token").1 = value.to_string();
                let body: String = schemes.iter().map(|(n, l)| scheme_text(n, l)).collect();
                let file = write(&dir, "t.cui", &program(&format!("{body}{}", font_text(""))));
                let out = dir.join("out");
                let _ = std::fs::remove_dir_all(&out);
                let (code, _, err) = run_in(
                    Some(&dir),
                    &[
                        "build",
                        file.to_str().expect("utf8"),
                        "--page",
                        "-o",
                        out.to_str().expect("utf8"),
                    ],
                );
                if code != 0 {
                    // the only reason a token value is refused is its contrast
                    assert!(
                        err.contains("theme-contrast"),
                        "{token} {scheme} {value}: {err}"
                    );
                    refused += 1;
                    continue;
                }
                built += 1;
                *per_token_built.entry(token.clone()).or_insert(0) += 1;
                let html = std::fs::read_to_string(out.join("index.html")).expect("page");
                // the baseline's rule for targets is still there, once, and the theme sits after it
                let rule =
                    "button, input, textarea { min-height: 44px; min-width: 44px; font: inherit; }";
                assert_eq!(html.matches(rule).count(), 1, "{token} {scheme} {value}");
                let theme_at = html.find("<style id=\"cui-theme\">").expect("theme");
                assert!(html.find(rule).expect("rule") < theme_at);
                let theme_css = &html[theme_at
                    ..html[theme_at..]
                        .find("</style>")
                        .map(|e| theme_at + e)
                        .expect("end")];
                for prop in properties(theme_css) {
                    assert!(
                        ALLOWED.contains(&prop.as_str()),
                        "{token} {scheme} {value}: the theme sets `{prop}`"
                    );
                }
                // the focus ring's width and style are the compiler's, whatever the colour
                assert!(theme_css.contains(":focus-visible { outline: 3px solid var(--cui-accent); outline-offset: 2px; }"), "{theme_css}");
                for forbidden in [
                    "outline: none",
                    "outline: 0",
                    "outline-width",
                    "outline-style",
                    "min-height",
                    "min-width",
                    "visibility",
                    "opacity",
                    "pointer-events",
                    "font-size",
                    "width:",
                    "height:",
                ] {
                    assert!(!theme_css.contains(forbidden), "{token}: {forbidden}");
                }
            }
        }
    }
    assert!(
        built > 40 && refused > 20,
        "built {built}, refused {refused}"
    );
    // every one of the 18 tokens built at least once: each was really tried
    for t in tokens(&c) {
        assert!(
            per_token_built.get(&t).copied().unwrap_or(0) > 0,
            "{t} never built"
        );
    }
}

#[test]
fn a_theme_that_is_not_written_properly_is_a_parse_refusal_not_a_panic() {
    let dir = scratch("parse");
    for (name, text) in [
        ("no closing brace", "theme {\n    light {\n"),
        (
            "a value that is not a string",
            "theme { light { bg: 3; } }\n",
        ),
        ("no colon", "theme { light { bg \"#ffffff\"; } }\n"),
        ("no semicolon", "theme { light { bg: \"#ffffff\" } }\n"),
        ("a font with no role", "theme { font { family: \"x\"; } }\n"),
        ("empty", "theme { }\n"),
    ] {
        let (code, diags) = check(&dir, &format!("{text}{APP}"));
        assert_eq!(code, 1, "{name}");
        assert!(!diags.is_empty(), "{name}");
        let c = diags[0]["code"].as_str().expect("code");
        assert!(
            c.starts_with("parse-") || c.starts_with("theme-"),
            "{name}: {c}"
        );
    }
}
