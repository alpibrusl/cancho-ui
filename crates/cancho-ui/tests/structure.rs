//! The structure elements (docs/structure.md): every refusal of `tests/structure_corpus.json` is refused by its rule tag, the valid views
//! check, the DOM they become is checked in Chromium (`scripts/check-structure.js`) against the same corpus, authority is unchanged except
//! the hosts a `Link` names, and the nesting bound of 32 applies to the new elements.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

const CORPUS: &str = include_str!("../../../tests/structure_corpus.json");

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
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("cancho-ui-structure-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let path = dir.join(name);
    std::fs::write(&path, text).expect("a scratch file");
    path
}

fn p(path: &Path) -> &str {
    path.to_str().expect("UTF-8 temp path")
}

fn corpus() -> Value {
    serde_json::from_str(CORPUS).expect("the corpus is JSON")
}

fn program(c: &Value, view: &str) -> String {
    format!(
        "component C {{\n    {}\n    view {{ Column {{ {view} }} }}\n}}\n",
        c["state"].as_str().expect("state")
    )
}

fn codes(text: &str) -> (i32, Vec<String>) {
    let file = scratch("p.cui", text);
    let (code, out, _) = run(&["check", p(&file), "--output", "json"]);
    let diags: Vec<Value> = if out.trim().is_empty() {
        Vec::new()
    } else {
        serde_json::from_str(&out).expect("diagnostics are JSON")
    };
    (
        code,
        diags
            .iter()
            .map(|d| d["code"].as_str().expect("code").to_string())
            .collect(),
    )
}

#[test]
fn every_valid_view_of_the_corpus_checks() {
    let c = corpus();
    for case in c["valid"].as_array().expect("valid") {
        let (code, got) = codes(&program(&c, case["view"].as_str().expect("view")));
        assert_eq!((code, got), (0, vec![]), "{}", case["name"]);
    }
}

#[test]
fn every_refusal_of_the_corpus_is_refused_by_its_tag() {
    let c = corpus();
    let mut tags = std::collections::BTreeSet::new();
    for case in c["refusals"].as_array().expect("refusals") {
        let text = match case["program"].as_str() {
            Some(whole) => whole.to_string(),
            None => program(&c, case["view"].as_str().expect("view")),
        };
        let want = case["code"].as_str().expect("code");
        let (code, got) = codes(&text);
        assert_eq!(code, 1, "{}", case["name"]);
        assert_eq!(got, vec![want.to_string()], "{}", case["name"]);
        tags.insert(want.to_string());
    }
    // every tag the structure module owns is reached
    for tag in [
        "heading-level-invalid",
        "heading-level-skipped",
        "heading-empty",
        "id-invalid",
        "anchor-duplicate",
        "anchor-unknown",
        "link-scheme-refused",
        "link-to-invalid",
        "link-label-missing",
        "link-label-generic",
        "link-appearance-invalid",
        "text-style-invalid",
        "details-incomplete",
        "caption-misplaced",
        "landmark-duplicate",
        "nav-label-missing",
        "binding-unknown",
    ] {
        assert!(tags.contains(tag), "no case reaches {tag}");
    }
}

#[test]
fn an_element_the_old_compiler_took_as_a_component_name_is_now_the_primitive() {
    // `Section`, `Link` and the others were names a program could give its own components (an instance of an undeclared name is refused);
    // a program that declares `component Link` and uses it is a program that now has a clash, and is told by the shape check, not run
    let text = "component Link {\n    view { Text { \"x\" } }\n}\n\ncomponent App {\n    view { Link { to: \"https://a.example/\" \"Go on\" } }\n}\n";
    let (code, got) = codes(text);
    assert_eq!(
        code, 0,
        "{got:?}: the built-in Link is what `Link {{ .. }}` means"
    );
}

#[test]
fn a_link_lists_its_host_in_inspect_and_diff_and_authority_stays() {
    let a = "component C {\n    view { Link { to: \"https://github.com/alpibrusl/cancho\" \"The compiler\" } }\n}\n";
    let b = "component C {\n    view { Column { Link { to: \"https://github.com/alpibrusl/cancho\" \"The compiler\" } Link { to: \"https://Evil.Example:8443/x\" \"The other\" } Link { to: \"mailto:Hi@Example.org\" \"Write\" } } }\n}\n";
    let (fa, fb) = (scratch("a.cui", a), scratch("b.cui", b));
    let (code, out, _) = run(&["inspect", "effects", p(&fb), "C"]);
    assert_eq!(code, 0);
    let v: Value = serde_json::from_str(&out).expect("JSON");
    assert_eq!(
        v["links"],
        serde_json::json!(["evil.example", "github.com", "mailto:example.org"])
    );
    assert_eq!(v["requires"], serde_json::json!([]));
    let (_, out, _) = run(&["inspect", "effects", p(&fa), "C"]);
    let v: Value = serde_json::from_str(&out).expect("JSON");
    assert_eq!(v["links"], serde_json::json!(["github.com"]));
    // diff: the new hosts are lines of their own, and the authority is not expanded
    let (code, human, _) = run(&["diff", p(&fa), p(&fb)]);
    assert_eq!(code, 5, "{human}");
    assert!(
        human.contains("+ link evil.example") && human.contains("+ link mailto:example.org"),
        "{human}"
    );
    assert!(!human.contains("+ link github.com"), "{human}");
    assert!(human.contains("authority: unchanged"), "{human}");
    let (_, js, _) = run(&["diff", p(&fa), p(&fb), "--output", "json"]);
    let v: Value = serde_json::from_str(&js).expect("JSON");
    assert_eq!(v["authority"], "unchanged");
    assert_eq!(
        v["components"]["changed"][0]["links"]["added"],
        serde_json::json!(["evil.example", "mailto:example.org"])
    );
    // the same links twice: no `links` key at all
    let (_, js, _) = run(&["diff", p(&fa), p(&fa), "--output", "json"]);
    assert!(!js.contains("\"links\""), "{js}");
}

fn nested(depth: usize, innermost: &str) -> String {
    let mut open = String::new();
    let mut close = String::new();
    for _ in 0..depth {
        open.push_str("Section { ");
        close.push_str(" }");
    }
    format!("component C {{\n    view {{ {open}{innermost}{close} }}\n}}\n")
}

#[test]
fn the_nesting_bound_of_32_applies_to_the_new_elements() {
    // one bound over view nodes, expressions and types together (docs/nesting.md): the text of a heading is an expression of its own, so
    // `Heading` with a text is 2 deep and `Details { Text { .. } }` is 3. With the budget of 32, the deepest program that checks has
    // `32 - d` sections around an innermost of depth `d`, and one more section is `parse-nesting-too-deep`.
    for (inner, d) in [
        ("Heading { level: 1 \"deep\" }", 2usize),
        ("Link { to: \"https://a.example/\" \"Go on\" }", 2),
        ("Details { summary: \"s\" Text { \"x\" } }", 3),
        ("Figure { Caption { \"c\" } }", 3),
        ("Article { Text { \"x\" } }", 3),
    ] {
        assert_eq!(
            codes(&nested(32 - d, inner)),
            (0, vec![]),
            "{inner} at the bound"
        );
        let (code, got) = codes(&nested(33 - d, inner));
        assert_eq!(code, 1, "{inner} one deeper");
        assert_eq!(
            got,
            vec!["parse-nesting-too-deep".to_string()],
            "{inner} one deeper"
        );
    }
}

#[test]
fn garbage_is_a_refusal_and_not_a_panic() {
    let c = corpus();
    for view in [
        "Heading {",
        "Heading { level: }",
        "Heading { level: 2 \"unterminated }",
        "Link { to: \"https://a.example/\" \"x\" Link { to: \"#\" \"y\" } }",
        "Details { summary: \"s\" summary: \"t\" Text { \"x\" } }",
        "Section { id: \"a\" id: \"b\" Text { \"x\" } }",
        "Caption { Caption { \"x\" } }",
        "Figure { Figure { Caption { \"a\" } } }",
        "Nav { Nav { label: \"a\" Text { \"x\" } } Nav { label: \"b\" Text { \"y\" } } }",
        "Link { to: \"\\u{0}\" \"x\" }",
        "Text { style: \"eyebrow\" style: \"note\" \"x\" }",
        "Heading { level: 9223372036854775807 \"x\" }",
        "Heading { level: -1 \"x\" }",
    ] {
        let (code, got) = codes(&program(&c, view));
        assert!(code == 1 || code == 0, "{view}: exit {code}");
        if code == 1 {
            assert!(!got.is_empty(), "{view}");
        }
    }
}
