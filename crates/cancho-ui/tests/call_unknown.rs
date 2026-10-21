//! `call-unknown` (`spec/diagnostics/README.md`): a call to a function the language does not have is refused when the
//! program is checked. It used to pass `check` and be emitted as `x = /* unsupported call */;`, which is a JavaScript
//! syntax error: the page died when the browser loaded it, with no rule tag (found building `examples/ops-console`).

use std::path::{Path, PathBuf};
use std::process::Command;

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
    let dir = std::env::temp_dir().join(format!("cancho-ui-call-unknown-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let path = dir.join(name);
    std::fs::write(&path, text).expect("a scratch file");
    path
}

fn p(path: &Path) -> &str {
    path.to_str().expect("UTF-8 temp path")
}

fn program(stmt: &str) -> String {
    format!(
        "component T {{\n    state n: Int = 0;\n    state s: String = \"\";\n    state l: List[String] = [\"a\"];\n    on Go {{ {stmt} }}\n    view {{ Text {{ \"x\" }} }}\n}}\n"
    )
}

#[test]
fn a_function_the_language_does_not_have_is_refused_with_its_tag_and_a_span() {
    for (i, stmt) in [
        "n = sqrt(n);",
        "n = pow(n, 2);",
        "n = trunc(n / 2);",
        "n = frobnicate(n);",
        "s = lowercase(s);",
        "s = contians(s, \"a\");",
        "n = math.max(n, 1);",
        "n = net.delete(\"a\");",
        "n = a.b.c(n);",
    ]
    .iter()
    .enumerate()
    {
        let file = scratch(&format!("unknown{i}.cui"), &program(stmt));
        let (code, stdout, _) = run(&["check", p(&file), "--output", "json"]);
        assert_eq!(code, 1, "{stmt}: {stdout}");
        let diags: Value = serde_json::from_str(&stdout).expect("JSON diagnostics");
        let d = &diags.as_array().expect("a list")[0];
        assert_eq!(d["code"], "call-unknown", "{stmt}: {stdout}");
        assert!(d["span"].is_object(), "{stmt}: located");
        assert!(
            d["remediation"]["description"]
                .as_str()
                .is_some_and(|t| t.contains("`max`") && t.contains("`filter`")),
            "{stmt}: the remediation names what exists"
        );
    }
}

#[test]
fn building_it_writes_nothing() {
    let file = scratch("build.cui", &program("n = sqrt(n);"));
    let out = std::env::temp_dir().join(format!(
        "cancho-ui-call-unknown-{}/build.js",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&out);
    let (code, _, stderr) = run(&["build", p(&file), "-o", p(&out)]);
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.contains("call-unknown"), "{stderr}");
    assert!(!out.exists(), "no JavaScript with a syntax error in it");
}

#[test]
fn every_call_the_emitters_know_is_still_accepted() {
    for (i, stmt) in [
        "n = len(l);",
        "l = filter(l, x => len(x) > 0);",
        "l = map(l, x => x);",
        "l = sort(l, x => x);",
        "l = reverse(l);",
        "l = take(l, 1);",
        "l = skip(l, 1);",
        "l = lines(s);",
        "l = split(s, \",\");",
        "s = join(l, \",\");",
        "s = trim(s);",
        "s = upper(s);",
        "s = lower(s);",
        "n = num(s, 0);",
        "n = max(n, 1);",
        "n = min(n, 1);",
        "n = floor(n / 2);",
        "n = ceil(n / 2);",
        "n = round(n / 2);",
        "n = abs(n);",
        "n = len(filter(l, x => contains(x, s)));",
        "n = len(filter(l, x => starts_with(x, s)));",
        "n = len(filter(l, x => ends_with(x, s)));",
        "s = json.get(s, \"a\", \"\");",
        "l = json.items(s, \"a\");",
    ]
    .iter()
    .enumerate()
    {
        let file = scratch(&format!("known{i}.cui"), &program(stmt));
        let (code, stdout, stderr) = run(&["check", p(&file)]);
        assert_eq!(code, 0, "{stmt}: {stdout}{stderr}");
    }
}

#[test]
fn every_example_still_checks() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let mut checked = 0;
    for dir in std::fs::read_dir(&root).expect("examples") {
        let dir = dir.expect("entry").path();
        if !dir.is_dir() {
            continue;
        }
        for f in std::fs::read_dir(&dir).expect("example dir") {
            let f = f.expect("entry").path();
            if f.extension().is_some_and(|e| e == "cui") && !f.to_string_lossy().contains("refused")
            {
                let (code, stdout, stderr) = run(&["check", p(&f)]);
                assert_eq!(code, 0, "{}: {stdout}{stderr}", f.display());
                checked += 1;
            }
        }
    }
    assert!(checked >= 10, "{checked} examples");
}
