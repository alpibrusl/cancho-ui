//! `file-too-long` (docs/file-budget.md section 3): a source file (the entry, an imported module, an included part) of more than 2,000 lines or
//! 160 KiB is refused, naming the file. The limit is moved by `--max-file-lines N` and `CANCHO_UI_MAX_FILE_LINES`; each rule is run at its limit
//! and one past it, through the binary, because the limit is read from the command line and the environment.
mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

const BIN: &str = env!("CARGO_BIN_EXE_cancho-ui");
const ENV: &str = "CANCHO_UI_MAX_FILE_LINES";

fn dir(name: &str) -> PathBuf {
    let d = common::scratch_root().join(format!("budget-{name}"));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("a scratch directory");
    d
}

fn run_env(d: &Path, args: &[&str], env: Option<&str>) -> (i32, String, String) {
    let mut c = Command::new(BIN);
    c.args(args).current_dir(d).env_remove(ENV);
    if let Some(v) = env {
        c.env(ENV, v);
    }
    let out = c.output().expect("the cancho-ui binary runs");
    (
        out.status.code().expect("exited, not killed by a signal"),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn run(d: &Path, args: &[&str]) -> (i32, String, String) {
    run_env(d, args, None)
}

const BASE: &str = "component C {\n    state n: Int = 0;\n    view { Text { \"{n}\" } }\n}\n";

/// A valid program of exactly `lines` physical lines (a comment pads it), ending with a newline.
fn program_of(lines: usize) -> String {
    let base = BASE.lines().count();
    assert!(lines >= base);
    let mut s = String::from(BASE);
    for i in 0..lines - base {
        s.push_str(&format!("// pad {i}\n"));
    }
    s
}

fn refusal(d: &Path, args: &[&str], env: Option<&str>) -> Value {
    let mut a = args.to_vec();
    a.extend(["--output", "json"]);
    let (code, out, err) = run_env(d, &a, env);
    assert_eq!(code, 1, "expected a refusal: {out} {err}");
    serde_json::from_str::<Value>(&out).expect("JSON diagnostics")[0].clone()
}

#[test]
fn exactly_at_the_limit_passes_and_one_over_is_refused() {
    let d = dir("limit");
    std::fs::write(d.join("a.cui"), program_of(2000)).expect("write");
    assert_eq!(run(&d, &["check", "a.cui"]).0, 0);
    std::fs::write(d.join("b.cui"), program_of(2001)).expect("write");
    let r = refusal(&d, &["check", "b.cui"], None);
    assert_eq!(r["code"], "file-too-long");
    assert_eq!(r["severity"], "error");
    assert_eq!(r["tier"], "static");
    assert_eq!(r["subject"]["kind"], "file");
    assert_eq!(r["subject"]["path"], "b.cui");
    assert_eq!(r["span"]["file"], "b.cui");
    assert_eq!(
        r["span"]["start"]["line"], 2001,
        "the first line over the limit"
    );
    let msg = r["message"].as_str().expect("message");
    assert!(
        msg.contains("b.cui") && msg.contains("2001 lines") && msg.contains("2000 lines"),
        "{msg}"
    );
    let rem = r["remediation"]["description"]
        .as_str()
        .expect("remediation");
    assert!(rem.contains("import") && rem.contains("include"), "{rem}");
    // the human form too
    let (code, _, err) = run(&d, &["check", "b.cui"]);
    assert_eq!(code, 1);
    assert!(err.contains("error[file-too-long] at b.cui:2001"), "{err}");
}

#[test]
fn comments_and_blank_lines_count_and_the_last_line_needs_no_newline() {
    let d = dir("count");
    // 2000 lines, the last without a newline
    let mut s = program_of(2000);
    s.pop();
    std::fs::write(d.join("a.cui"), &s).expect("write");
    assert_eq!(run(&d, &["check", "a.cui"]).0, 0);
    // 2001 lines of which the last is a lone, unterminated line
    s.push_str("\n//");
    std::fs::write(d.join("b.cui"), &s).expect("write");
    assert_eq!(
        refusal(&d, &["check", "b.cui"], None)["code"],
        "file-too-long"
    );
    // blank lines count
    let blanks = format!("{BASE}{}", "\n".repeat(1997));
    std::fs::write(d.join("c.cui"), &blanks).expect("write");
    assert_eq!(blanks.lines().count(), 2001);
    assert_eq!(
        refusal(&d, &["check", "c.cui"], None)["code"],
        "file-too-long"
    );
}

#[test]
fn crlf_is_one_line_a_lone_cr_is_not_a_line_end() {
    let d = dir("crlf");
    std::fs::write(d.join("a.cui"), program_of(2000).replace('\n', "\r\n")).expect("write");
    assert_eq!(
        run(&d, &["check", "a.cui"]).0,
        0,
        "2000 lines of CRLF are 2000 lines"
    );
    std::fs::write(d.join("b.cui"), program_of(2001).replace('\n', "\r\n")).expect("write");
    assert_eq!(
        refusal(&d, &["check", "b.cui"], None)["code"],
        "file-too-long"
    );
    // a `\r` alone does not end a line (it is not one to `str::lines()`, and the lexer's line count ignores it)
    let cr = format!("{BASE}{}", "// x\r".repeat(5000));
    std::fs::write(d.join("c.cui"), &cr).expect("write");
    assert_eq!(cr.lines().count(), 5, "str::lines");
    let (code, out, err) = run(&d, &["check", "c.cui"]);
    assert_eq!(code, 0, "{out}{err}");
}

#[test]
fn an_empty_file_and_a_file_without_a_newline_are_not_a_panic() {
    let d = dir("empty");
    std::fs::write(d.join("e.cui"), "").expect("write");
    // an empty file has 0 lines: whatever the compiler makes of it, it is not too long, at any limit
    for args in [
        &["check", "e.cui"][..],
        &["check", "e.cui", "--max-file-lines", "1"],
    ] {
        let (code, out, err) = run(&d, args);
        assert!(code == 0 || code == 1, "{args:?}: {code} {err}");
        assert!(
            !err.contains("file-too-long")
                && !out.contains("file-too-long")
                && !err.contains("panicked"),
            "{err}"
        );
    }
    std::fs::write(d.join("n.cui"), "component C { view { Text { \"x\" } } }").expect("write");
    assert_eq!(run(&d, &["check", "n.cui"]).0, 0);
    assert_eq!(
        run(&d, &["check", "n.cui", "--max-file-lines", "1"]).0,
        0,
        "one line without a newline is one line"
    );
}

#[test]
fn the_flag_and_the_variable_raise_and_lower_the_limit_and_the_flag_wins() {
    let d = dir("knobs");
    std::fs::write(d.join("a.cui"), program_of(2500)).expect("write");
    assert_eq!(
        refusal(&d, &["check", "a.cui"], None)["code"],
        "file-too-long"
    );
    // raised
    assert_eq!(
        run(&d, &["check", "a.cui", "--max-file-lines", "2500"]).0,
        0
    );
    assert_eq!(run(&d, &["check", "a.cui", "--max-file-lines=2500"]).0, 0);
    assert_eq!(run_env(&d, &["check", "a.cui"], Some("2500")).0, 0);
    assert_eq!(
        refusal(&d, &["check", "a.cui", "--max-file-lines", "2499"], None)["code"],
        "file-too-long"
    );
    assert_eq!(
        refusal(&d, &["check", "a.cui"], Some("2499"))["code"],
        "file-too-long"
    );
    // lowered
    std::fs::write(d.join("b.cui"), program_of(100)).expect("write");
    assert_eq!(run(&d, &["check", "b.cui"]).0, 0);
    assert_eq!(run(&d, &["check", "b.cui", "--max-file-lines", "100"]).0, 0);
    let r = refusal(&d, &["check", "b.cui", "--max-file-lines", "99"], None);
    assert!(
        r["message"]
            .as_str()
            .expect("message")
            .contains("100 lines")
            && r["message"].as_str().expect("message").contains("99 lines")
    );
    assert_eq!(
        refusal(&d, &["check", "b.cui"], Some("99"))["code"],
        "file-too-long"
    );
    // the flag beats the variable, both ways, and may come before the file
    assert_eq!(
        run_env(
            &d,
            &["check", "--max-file-lines", "3000", "a.cui"],
            Some("10")
        )
        .0,
        0
    );
    assert_eq!(
        refusal(
            &d,
            &["check", "b.cui", "--max-file-lines", "10"],
            Some("5000")
        )["code"],
        "file-too-long"
    );
}

#[test]
fn a_limit_that_is_not_a_number_of_lines_is_an_error_and_never_a_panic() {
    let d = dir("bad-limits");
    std::fs::write(d.join("a.cui"), BASE).expect("write");
    for bad in ["0", "-1", "+5", "abc", "", "1.5", "1e3", "5 ", "0x10"] {
        let (code, _, err) = run(&d, &["check", "a.cui", "--max-file-lines", bad]);
        assert_eq!(code, 2, "flag {bad:?}: {err}");
        assert!(err.contains("--max-file-lines"), "{err}");
        let (code, _, err) = run_env(&d, &["check", "a.cui"], Some(bad));
        assert_eq!(code, 3, "variable {bad:?}: {err}");
        assert!(err.contains(ENV), "{err}");
        assert!(!err.contains("panicked"), "{err}");
    }
    // the flag without a value
    let (code, _, err) = run(&d, &["check", "a.cui", "--max-file-lines"]);
    assert_eq!(code, 2, "{err}");
    // a bad variable is not read by a command that compiles nothing
    assert_eq!(run_env(&d, &["introspect"], Some("abc")).0, 0);
    // and those commands do not take the flag
    assert_eq!(run(&d, &["introspect", "--max-file-lines", "5"]).0, 2);
}

#[test]
fn a_huge_limit_is_no_limit() {
    let d = dir("huge");
    std::fs::write(d.join("a.cui"), program_of(2500)).expect("write");
    for n in [
        "4294967296",
        "18446744073709551615",
        "18446744073709551616",
        "99999999999999999999999999999999",
    ] {
        assert_eq!(
            run(&d, &["check", "a.cui", "--max-file-lines", n]).0,
            0,
            "{n}"
        );
        assert_eq!(run_env(&d, &["check", "a.cui"], Some(n)).0, 0, "{n}");
    }
}

#[test]
fn the_bytes_are_bounded_too_at_160_kib_and_the_bound_scales_with_the_limit() {
    let d = dir("bytes");
    // 2,000 lines of 100 characters: 200,000 bytes, under the line limit and over the byte limit
    let mut long = String::from(BASE);
    for _ in 0..1996 {
        long.push_str(&format!("//{}\n", "x".repeat(97)));
    }
    assert_eq!(long.lines().count(), 2000);
    assert!(long.len() > 163_840);
    std::fs::write(d.join("a.cui"), &long).expect("write");
    let r = refusal(&d, &["check", "a.cui"], None);
    assert_eq!(r["code"], "file-too-long");
    let msg = r["message"].as_str().expect("message");
    assert!(msg.contains("bytes") && msg.contains("163840"), "{msg}");
    assert!(!msg.contains("2001 lines"), "the unit is bytes: {msg}");
    // the bound scales with the limit: 4000 lines allow 327,680 bytes
    assert_eq!(
        run(&d, &["check", "a.cui", "--max-file-lines", "4000"]).0,
        0
    );
    // exactly 163,840 bytes passes; one more byte is refused
    let mut at = String::from(BASE);
    while at.len() + 100 < 163_840 {
        at.push_str(&format!("//{}\n", "y".repeat(97)));
    }
    let rest = 163_840 - at.len();
    at.push_str(&format!("//{}\n", "z".repeat(rest - 3)));
    assert_eq!(at.len(), 163_840);
    std::fs::write(d.join("b.cui"), &at).expect("write");
    assert_eq!(run(&d, &["check", "b.cui"]).0, 0);
    at.push('\n');
    std::fs::write(d.join("c.cui"), &at).expect("write");
    assert_eq!(
        refusal(&d, &["check", "c.cui"], None)["code"],
        "file-too-long"
    );
}

#[test]
fn an_imported_module_over_the_limit_is_named_not_the_entry() {
    let d = dir("module");
    std::fs::write(
        d.join("cards.cui"),
        format!(
            "module cards;\n\ncomponent Card {{\n    view {{ Text {{ \"x\" }} }}\n}}\n{}",
            "// pad\n".repeat(2000)
        ),
    )
    .expect("write");
    std::fs::write(
        d.join("app.cui"),
        "module app;\nimport cards.Card;\n\ncomponent Page {\n    view { Card { } }\n}\n",
    )
    .expect("write");
    let r = refusal(&d, &["check", "app.cui"], None);
    assert_eq!(r["code"], "file-too-long");
    assert!(
        r["span"]["file"]
            .as_str()
            .expect("file")
            .ends_with("cards.cui")
    );
    assert!(
        r["message"]
            .as_str()
            .expect("message")
            .contains("cards.cui")
    );
    // at the limit the program compiles
    std::fs::write(
        d.join("cards.cui"),
        format!(
            "module cards;\n\ncomponent Card {{\n    view {{ Text {{ \"x\" }} }}\n}}\n{}",
            "// pad\n".repeat(1995)
        ),
    )
    .expect("write");
    assert_eq!(run(&d, &["check", "app.cui"]).0, 0);
}

#[test]
fn a_program_longer_than_the_limit_in_all_but_not_in_any_file_is_fine() {
    // the point of the rule: three files of 1,990 lines are 5,970 lines of program
    let d = dir("split");
    let pad = |n: usize| "// pad\n".repeat(n);
    std::fs::write(
        d.join("app.cui"),
        format!("component C {{\n    include \"a.part\";\n    include \"b.part\";\n    view {{ Text {{ \"{{n}}\" }} }}\n}}\n{}", pad(1990)),
    )
    .expect("write");
    std::fs::write(
        d.join("a.part"),
        format!("state n: Int = 0;\n{}", pad(1990)),
    )
    .expect("write");
    std::fs::write(
        d.join("b.part"),
        format!("on Go {{ n = 1; }}\n{}", pad(1990)),
    )
    .expect("write");
    let (code, text, err) = run(&d, &["inspect", "sources", "app.cui"]);
    assert_eq!(code, 0, "{err}");
    assert!(text.lines().count() > 5000);
    assert_eq!(run(&d, &["check", "app.cui"]).0, 0);
    assert_eq!(run(&d, &["build", "app.cui"]).0, 0);
}

#[test]
fn every_command_that_compiles_takes_the_flag_and_applies_it() {
    let d = dir("commands");
    std::fs::write(d.join("a.cui"), BASE).expect("write");
    std::fs::write(
        d.join("trace.json"),
        "{\"schema\":\"cancho-ui-trace\",\"version\":\"0.1.0\",\"steps\":[]}",
    )
    .expect("write");
    let cmds: &[&[&str]] = &[
        &["check", "a.cui"],
        &["build", "a.cui"],
        &["inspect", "ir", "a.cui"],
        &["inspect", "sources", "a.cui"],
        &["inspect", "component", "a.cui", "C"],
        &["diff", "a.cui", "a.cui"],
        &["test", "a.cui", "trace.json"],
        &["report", "a.cui"],
    ];
    for c in cmds {
        let (code, _, err) = run(&d, c);
        assert_ne!(code, 1, "{c:?} ok without the flag: {err}");
        let mut with: Vec<&str> = c.to_vec();
        with.extend(["--max-file-lines", "2"]);
        let (code, out, err) = run(&d, &with);
        // `inspect` has always answered a refusal with 3 and the diagnostic's debug text on stderr (not changed here)
        let expected = if c[0] == "inspect" && c[1] != "sources" {
            3
        } else {
            1
        };
        assert_eq!(code, expected, "{with:?}: {out} {err}");
        assert!(
            out.contains("file-too-long") || err.contains("file-too-long"),
            "{with:?}: {out} {err}"
        );
        // and the variable
        let (code, out, err) = run_env(&d, c, Some("2"));
        assert_eq!(code, expected, "{c:?} with the variable: {out} {err}");
    }
}

#[test]
fn a_console_sized_program_needs_its_limit_and_the_byte_bound_scales_to_it() {
    let d = dir("console");
    // 7,405 lines of about 65 bytes: the size of the one-file console this rule was written for
    let mut s = String::from(BASE);
    while s.lines().count() < 7405 {
        s.push_str(&format!("// {:>60}\n", s.lines().count()));
    }
    assert_eq!(s.lines().count(), 7405);
    std::fs::write(d.join("trip.cui"), &s).expect("write");
    assert_eq!(
        refusal(&d, &["check", "trip.cui"], None)["code"],
        "file-too-long"
    );
    assert_eq!(run_env(&d, &["check", "trip.cui"], Some("7405")).0, 0);
    assert_eq!(
        refusal(&d, &["check", "trip.cui"], Some("7404"))["code"],
        "file-too-long"
    );
}

#[test]
fn every_cui_file_of_the_repository_is_under_the_default_limit() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    fn walk(d: &Path, out: &mut Vec<PathBuf>) {
        for e in std::fs::read_dir(d).expect("a directory") {
            let p = e.expect("an entry").path();
            let name = p
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();
            if p.is_dir() {
                if name != "target" && name != ".git" && name != "node_modules" {
                    walk(&p, out);
                }
            } else if name.ends_with(".cui") || name.ends_with(".part") {
                out.push(p);
            }
        }
    }
    let mut files = Vec::new();
    walk(&root, &mut files);
    assert!(files.len() > 40);
    for f in files {
        let t = std::fs::read_to_string(&f).expect("UTF-8");
        assert!(
            t.lines().count() <= 2000 && t.len() <= 163_840,
            "{} is over the file budget",
            f.display()
        );
    }
}
