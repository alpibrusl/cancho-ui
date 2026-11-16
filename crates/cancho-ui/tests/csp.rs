//! The baseline Content-Security-Policy of every page (docs/structure.md 12.0r, `crates/cancho-ui/src/csp.rs`): a page of either kind has one, whatever it draws; it is first in
//! the head; it lists the SHA-256 of every inline script and no `unsafe-inline` for scripts; taking a map out of a page leaves it with the baseline; a program adds to the baseline what it
//! declared (its tile hosts, its `Network` hosts, its sealed islands) and to nothing else. What a browser does with it is `scripts/check-csp.js`.
mod common;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn run(args: &[&str]) -> (i32, String, String) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new(env!("CARGO_BIN_EXE_cancho-ui"))
        .args(args)
        .current_dir(root)
        .output()
        .expect("the cancho-ui binary runs");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn p(path: &Path) -> &str {
    path.to_str().expect("UTF-8 temp path")
}

fn scratch() -> PathBuf {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = common::scratch_root().join(format!("cancho-ui-csp-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir
}

fn built(src: &str, mode: &str) -> String {
    let dir = scratch();
    let f = dir.join("p.cui");
    std::fs::write(&f, src).expect("the program");
    let out = dir.join("out");
    let (code, o, e) = run(&["build", p(&f), mode, "-o", p(&out)]);
    assert_eq!(code, 0, "{o}{e}");
    std::fs::read_to_string(out.join("index.html")).expect("index.html")
}

fn built_file(file: &str, mode: &str) -> String {
    let out = scratch().join("out");
    let (code, o, e) = run(&["build", file, mode, "-o", p(&out)]);
    assert_eq!(code, 0, "{o}{e}");
    std::fs::read_to_string(out.join("index.html")).expect("index.html")
}

/// The CSP source of a script body, by the system's tools: `'sha256-<base64 of the digest>'`, not the compiler's own implementation.
fn source_of(body: &str) -> String {
    let mut sum = Command::new("openssl")
        .args(["dgst", "-sha256", "-binary"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("openssl is installed");
    sum.stdin
        .take()
        .expect("stdin")
        .write_all(body.as_bytes())
        .expect("write");
    let digest = sum.wait_with_output().expect("openssl runs").stdout;
    let mut b64 = Command::new("openssl")
        .args(["base64", "-A"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("openssl is installed");
    b64.stdin
        .take()
        .expect("stdin")
        .write_all(&digest)
        .expect("write");
    format!(
        "'sha256-{}'",
        String::from_utf8_lossy(&b64.wait_with_output().expect("openssl runs").stdout)
    )
}

/// The bodies of the `<script>` elements of a page that have no attributes (inline scripts).
fn inline_scripts(html: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = html;
    while let Some(i) = rest.find("<script>") {
        let body = &rest[i + 8..];
        let end = body.find("</script>").expect("a script ends");
        out.push(body[..end].to_string());
        rest = &body[end + 9..];
    }
    out
}

const PLAIN: &str = "component C {\n    state n: Int = 0;\n    on Inc { n = n + 1; }\n    view { Column { Button { label: \"Add\" on: Inc } Text { \"{n}\" } } }\n}\n";
const BASELINE: &str = "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self'; object-src 'none'; base-uri 'self'";

#[test]
fn every_page_has_the_baseline_and_a_page_with_nothing_special_has_nothing_more() {
    let html = built(PLAIN, "--page");
    assert_eq!(common::csp(&html).as_deref(), Some(BASELINE), "{html}");
    // the app has one inline script (the worker's registration), which the policy lists
    let app = built(PLAIN, "--pwa");
    let policy = common::csp(&app).expect("an app has a policy");
    assert_eq!(
        policy.replacen(
            &format!(
                "script-src 'self' {}",
                source_of(&inline_scripts(&app).concat())
            ),
            "script-src 'self'",
            1
        ),
        BASELINE,
        "{policy}"
    );
}

#[test]
fn the_policy_comes_before_anything_in_the_head_that_could_ask_for_something() {
    for mode in ["--page", "--pwa"] {
        let html = built(PLAIN, mode);
        let at = html.find("Content-Security-Policy").expect("a policy");
        for later in [
            "<title>",
            "<style>",
            "<link",
            "<script",
            "<meta name=\"viewport\"",
        ] {
            if let Some(i) = html.find(later) {
                assert!(at < i, "{mode}: the policy is after {later}");
            }
        }
    }
}

#[test]
fn every_inline_script_is_listed_by_its_hash_and_scripts_have_no_unsafe_inline() {
    for (file, mode) in [
        ("examples/page/page.cui", "--page"),
        ("examples/counter/counter.cui", "--pwa"),
        ("examples/motion/motion.cui", "--page"),
    ] {
        let html = built_file(file, mode);
        let script = common::directive(&html, "script-src").expect("script-src");
        let inline = inline_scripts(&html);
        assert!(!inline.is_empty(), "{file} has an inline script");
        for body in inline {
            assert!(
                script.contains(&source_of(&body)),
                "{file}: a script of {} bytes is not listed in `{script}`",
                body.len()
            );
        }
        assert!(
            script.starts_with("'self'")
                && !script.contains("unsafe-inline")
                && !script.contains("unsafe-eval"),
            "{file}: {script}"
        );
        // a script an attacker adds is not among them: nothing is allowed by a host, a scheme or a wildcard
        assert!(
            !script.contains("https:") && !script.contains('*'),
            "{file}: {script}"
        );
    }
}

const MAP: &str = "component M {\n    requires Network(\"tiles.example\");\n    state pos: String = \"40.42,-3.70\";\n    state places: List[String] = [\"[\\\"Sol\\\",40.4168,-3.7038]\"];\n    view { Column { {map} Text { \"hello\" } } }\n}\n";
const A_MAP: &str = "Map { label: \"Places\" center: pos places: places zoom: 13 tiles: \"https://tiles.example/{z}/{x}/{y}.png\" attribution: \"c\" }";

#[test]
fn taking_the_map_out_of_a_page_leaves_it_with_its_policy() {
    let with = built(&MAP.replace("{map}", A_MAP), "--page");
    assert_eq!(
        common::directive(&with, "img-src").as_deref(),
        Some("'self' data: https://tiles.example")
    );
    // the same page, the map taken out (and the host with it, as the compiler would refuse it unused): the policy is still there
    let without = built(
        &MAP.replace("{map}", "")
            .replace("    requires Network(\"tiles.example\");\n", ""),
        "--page",
    );
    assert_eq!(common::csp(&without).as_deref(), Some(BASELINE));
    // and the two differ in the tile host and in that alone
    let a = common::csp(&with)
        .expect("policy")
        .replace(" https://tiles.example", "");
    let b = common::csp(&without).expect("policy");
    let strip = |s: &str| {
        s.split("; ")
            .filter(|d| !d.starts_with("connect-src") && !d.starts_with("script-src"))
            .collect::<Vec<_>>()
            .join("; ")
    };
    assert_eq!(strip(&a), strip(&b));
}

#[test]
fn the_hosts_a_program_declares_are_the_only_other_origins_it_may_connect_to() {
    let src = "component F {\n    requires Network(\"API.example\", \"data.example:8443\", \"api.example\");\n    state r: Result[String, String] = Ok(\"\");\n    on Go { r = await(net.get(\"https://api.example/x\")); }\n    view { Button { label: \"Go\" on: Go } }\n}\n";
    let html = built(src, "--page");
    assert_eq!(
        common::directive(&html, "connect-src").as_deref(),
        Some("'self' api.example data.example:8443"),
        "lower case, once each, the port kept, and no scheme: the page's own"
    );
    // a program that declares a bare `Network` (its own origin) adds no host
    let own = built(
        "component F {\n    requires Network;\n    state r: Result[String, String] = Ok(\"\");\n    on Go { r = await(net.get(\"data.txt\")); }\n    view { Button { label: \"Go\" on: Go } }\n}\n",
        "--page",
    );
    assert_eq!(
        common::directive(&own, "connect-src").as_deref(),
        Some("'self'")
    );
}

#[test]
fn a_sealed_island_adds_its_frame_rule_and_the_hashes_of_the_scripts_of_its_document() {
    let html = built_file("examples/islands/islands.cui", "--page");
    assert_eq!(
        common::directive(&html, "frame-src").as_deref(),
        Some("about:")
    );
    // the frame's document is in an attribute, escaped, so the page's own scan does not find its scripts: the policy has them from the island
    let script = common::directive(&html, "script-src").expect("script-src");
    let page_only = inline_scripts(&html).len();
    assert!(
        script.matches("'sha256-").count() >= page_only + 3,
        "the shim, the island's script(s) and the ready line: {script}"
    );
    // a page with no island has no frame rule, and the frame rule is not `default-src`'s: the baseline lets in no frame
    assert_eq!(
        common::directive(&built(PLAIN, "--page"), "frame-src"),
        None
    );
}

#[test]
fn a_directive_of_a_program_adds_to_the_baseline_one_and_never_takes_from_it() {
    // the unit test of csp::policy says the same for text; here, on a built page, nothing of the baseline is missing from a page with every addition
    let html = built_file("examples/map-island/map.cui", "--page");
    let policy = common::csp(&html).expect("a policy");
    for must in [
        "default-src 'self'",
        "style-src 'self' 'unsafe-inline'",
        "object-src 'none'",
        "base-uri 'self'",
    ] {
        assert!(policy.contains(must), "{must} in {policy}");
    }
    assert!(
        common::directive(&html, "img-src")
            .expect("img-src")
            .starts_with("'self' data:")
    );
}
