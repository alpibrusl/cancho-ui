//! `requires Network("host")` (`docs/adapters.md` section 10): the syntax, the two refusals, what is emitted, and the
//! semantic diff. The browser-side check (an allowed host is reached, another is not) is in
//! `scripts/check-network.js`.

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
    let dir = std::env::temp_dir().join(format!("cancho-ui-narrow-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let path = dir.join(name);
    std::fs::write(&path, text).expect("a scratch file");
    path
}

fn p(path: &Path) -> &str {
    path.to_str().expect("UTF-8 temp path")
}

/// A component with the given `requires` line(s) and a handler that fetches `url` (a string literal).
fn program(requires: &str, url: &str) -> String {
    format!(
        "component N {{\n    {requires}\n    state r: Result[String, String] = Ok(\"\");\n    on Go {{ r = await(net.get(\"{url}\")); }}\n    view {{ Text {{ \"x\" }} }}\n}}\n"
    )
}

fn check(src: &str, name: &str) -> (i32, Vec<(String, String)>) {
    let file = scratch(name, src);
    let (code, stdout, _) = run(&["check", p(&file), "--output", "json"]);
    if code == 0 {
        return (0, Vec::new());
    }
    let diags: Value = serde_json::from_str(&stdout).expect("diagnostics are JSON");
    let list = diags
        .as_array()
        .expect("a list")
        .iter()
        .map(|d| {
            (
                d["code"].as_str().expect("a code").to_string(),
                d["remediation"]["description"]
                    .as_str()
                    .unwrap_or("")
                    .to_string(),
            )
        })
        .collect();
    (code, list)
}

fn codes(src: &str, name: &str) -> Vec<String> {
    check(src, name).1.into_iter().map(|(c, _)| c).collect()
}

#[test]
fn a_bound_is_written_after_the_name_and_a_literal_url_on_an_allowed_host_checks() {
    for (requires, url) in [
        ("requires Network;", "data.txt"),
        ("requires Network;", "/api/items?q=1"),
        (
            "requires Network(\"api.example.com\");",
            "https://api.example.com/x",
        ),
        ("requires Network(\"api.example.com\");", "data.txt"),
        (
            "requires Network(\"a.example\", \"B.Example:8443\");",
            "https://b.example:8443/x",
        ),
        (
            "requires Network(\"api.example.com\");",
            "https://API.example.com:443/x",
        ),
        (
            "requires Network(\"x.example:8080\");",
            "http://x.example:8080/y",
        ),
        (
            "requires Network(\"api.example.com\");",
            "https://user@api.example.com/x",
        ),
    ] {
        let (code, list) = check(&program(requires, url), "ok.cui");
        assert_eq!(code, 0, "{requires} {url}: {list:?}");
    }
}

#[test]
fn a_literal_url_on_a_host_the_component_does_not_allow_is_bound_violated() {
    for (requires, url) in [
        ("requires Network;", "https://api.example.com/x"),
        (
            "requires Network(\"api.example.com\");",
            "https://other.example/x",
        ),
        (
            "requires Network(\"api.example.com\");",
            "//other.example/x",
        ),
        (
            "requires Network(\"api.example.com\");",
            "https://api.example.com:8443/x",
        ),
        (
            "requires Network(\"api.example.com\");",
            "https://api.example.com.evil.example/x",
        ),
        (
            "requires Network(\"api.example.com\");",
            "https://api.example.com@evil.example/x",
        ),
        ("requires Network;", "http://"),
    ] {
        assert_eq!(
            codes(&program(requires, url), "violated.cui"),
            ["capability-bound-violated"],
            "{requires} {url}"
        );
    }
    let (_, list) = check(
        &program("requires Network;", "https://api.example.com/x"),
        "msg.cui",
    );
    assert!(
        list[0].1.contains("requires Network(\"api.example.com\")"),
        "{list:?}"
    );
}

#[test]
fn a_bound_on_a_capability_that_takes_none_or_that_is_not_a_host_is_narrowing_illegal() {
    for requires in [
        "requires Clock(\"x\");",
        "requires Storage(\"a:b\");",
        "requires Network(\"https://api.example.com\");",
        "requires Network(\"api.example.com/path\");",
        "requires Network(\"*.example.com\");",
        "requires Network(\"\");",
        "requires Network(\"a b\");",
        "requires Network(\"host:\");",
        "requires Network(\"host:99999x\");",
        "requires Network(\".example\");",
    ] {
        let src = if requires.contains("Clock") {
            "component N {\n    requires Clock(\"x\");\n    state n: Int = 0;\n    on Go { n = clock.now(); }\n    view { Text { \"x\" } }\n}\n".to_string()
        } else if requires.contains("Storage") {
            "component N {\n    requires Storage(\"a:b\");\n    on Go { storage.set(\"a\", \"b\"); }\n    view { Text { \"x\" } }\n}\n".to_string()
        } else {
            program(requires, "data.txt")
        };
        assert_eq!(
            codes(&src, "illegal.cui"),
            ["capability-narrowing-illegal"],
            "{requires}"
        );
    }
}

#[test]
fn a_bound_must_be_a_string_literal() {
    let file = scratch(
        "computed.cui",
        &program("requires Network(host);", "data.txt"),
    );
    let (code, _, stderr) = run(&["check", p(&file)]);
    assert_eq!(code, 1, "{stderr}");
    assert!(
        stderr.contains("parse-unexpected-token") && stderr.contains("string literal bound"),
        "{stderr}"
    );
}

#[test]
fn the_hosts_are_emitted_per_component_lower_cased_and_the_ir_is_unchanged_without_a_bound() {
    let src = program(
        "requires Network(\"API.example.com\", \"b.example:8443\");",
        "https://api.example.com/x",
    );
    let file = scratch("emit.cui", &src);
    let out = scratch("emit.js", "");
    let (code, _, stderr) = run(&["build", p(&file), "-o", p(&out)]);
    assert_eq!(code, 0, "{stderr}");
    let js = std::fs::read_to_string(out).expect("the built program");
    assert!(
        js.contains("const __cui_net_hosts = [\"api.example.com\", \"b.example:8443\"];"),
        "{js}"
    );
    assert!(js.contains("hosts.indexOf(target.host.toLowerCase()) === -1"));
    let bare = scratch("bare.cui", &program("requires Network;", "data.txt"));
    let out = scratch("bare.js", "");
    assert_eq!(run(&["build", p(&bare), "-o", p(&out)]).0, 0);
    assert!(
        std::fs::read_to_string(out)
            .expect("js")
            .contains("const __cui_net_hosts = [];")
    );
    // The IR of a bare capability has no `bounds` key, so IR written before bounds existed is the same bytes.
    let (code, stdout, stderr) = run(&["inspect", "ir", p(&bare)]);
    assert_eq!(code, 0, "{stdout}{stderr}");
    assert!(
        stdout.contains("\"requires\"") && !stdout.contains("\"bounds\""),
        "{stdout}"
    );
    let (code, stdout, stderr) = run(&["inspect", "ir", p(&file)]);
    assert_eq!(code, 0, "{stdout}{stderr}");
    assert!(
        stdout.contains("\"bounds\""),
        "a bound is in the IR: {stdout}"
    );
}

#[test]
fn the_semantic_diff_sees_a_bound_as_a_change_of_declared_authority() {
    let base = scratch("base.cui", &program("requires Network;", "data.txt"));
    let cand = scratch(
        "cand.cui",
        &program("requires Network(\"api.example.com\");", "data.txt"),
    );
    let (code, stdout, stderr) = run(&["diff", p(&base), p(&cand), "--output", "json"]);
    assert!(code == 0 || code == 5, "{stdout}{stderr}");
    assert!(
        stdout.contains("Network(\\\"api.example.com\\\")"),
        "the candidate's declaration is reported with its bound: {stdout}"
    );
}
