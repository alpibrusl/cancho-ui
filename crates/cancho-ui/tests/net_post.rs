//! `net.post(url, body)` (`docs/adapters.md` section 17): a write, with its own capability. What is emitted, what is
//! refused, how a trace scripts it. The browser side is `scripts/check-post.js`.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

const FETCH: &str = include_str!("../../../examples/fetch/fetch.cui");

const POSTER: &str = "component Poster {\n    requires NetworkWrite;\n    state note: String = \"hello\";\n    state r: Result[String, String] = Ok(\"\");\n    state code: String = \"-\";\n    on Save {\n        r = await(net.post(\"/api/notes\", note));\n        code = match r { Ok(reply) => { json.get(reply, \"status\", \"-\") } Err(e) => { e } Cancelled => { \"cancelled\" } };\n    }\n    view { Column { Text { \"{code}\" } Button { label: \"Save\" on: Save } } }\n}\n";

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
    let dir = std::env::temp_dir().join(format!("cancho-ui-net-post-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let path = dir.join(name);
    std::fs::write(&path, text).expect("a scratch file");
    path
}

fn p(path: &Path) -> &str {
    path.to_str().expect("UTF-8 temp path")
}

fn codes(src: &str, name: &str) -> Vec<String> {
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

#[test]
fn the_post_adapter_is_emitted_only_for_a_program_that_writes() {
    let file = scratch("poster.cui", POSTER);
    let out = scratch("poster.js", "");
    let (code, _, stderr) = run(&["build", p(&file), "-o", p(&out)]);
    assert_eq!(code, 0, "{stderr}");
    let js = std::fs::read_to_string(out).expect("the built program");
    assert!(
        js.contains("__cui_net_post(\"/api/notes\", __state_note, __cui_cred)"),
        "{js}"
    );
    assert!(js.contains("method: \"POST\""));
    assert!(js.contains("credentials: \"same-origin\", redirect: \"manual\""));
    assert!(js.contains("target.origin !== window.location.origin"));
    // a write is not aborted by a newer one or by disposal: no owner.abort in the post adapter
    let post = js
        .split("function __cui_net_post")
        .nth(1)
        .expect("the adapter");
    let post = post.split("\n}\n").next().expect("its body");
    assert!(!post.contains("owner"), "{post}");
    // and a read-only program carries none of it
    let file = scratch("reader.cui", FETCH);
    let out = scratch("reader.js", "");
    let (code, _, stderr) = run(&["build", p(&file), "-o", p(&out)]);
    assert_eq!(code, 0, "{stderr}");
    let js = std::fs::read_to_string(out).expect("the built program");
    assert!(!js.contains("__cui_net_post"), "{js}");
    assert!(!js.contains("method: \"POST\""));
}

#[test]
fn a_write_needs_its_own_capability_and_a_read_does_not_grant_it() {
    let none = POSTER.replace("    requires NetworkWrite;\n", "");
    assert_eq!(codes(&none, "none.cui"), ["capability-missing"]);
    let read_only = POSTER.replace("NetworkWrite", "Network");
    assert_eq!(codes(&read_only, "read.cui"), ["capability-missing"]);
    let unused = POSTER.replace(
        "    requires NetworkWrite;\n",
        "    requires NetworkWrite;\n    requires Network;\n",
    );
    assert_eq!(codes(&unused, "unused.cui"), ["capability-unused"]);
}

#[test]
fn post_takes_two_arguments() {
    for call in [
        "net.post(\"/a\")",
        "net.post(\"/a\", note, note)",
        "net.post()",
    ] {
        let src = POSTER.replace("net.post(\"/api/notes\", note)", call);
        assert_eq!(
            codes(&src, "arity.cui"),
            ["builtin-shape-mismatch"],
            "{call}"
        );
    }
}

#[test]
fn a_write_takes_no_host_bound_and_a_literal_on_another_host_is_refused() {
    let bound = POSTER.replace("NetworkWrite;", "NetworkWrite(\"other.example\");");
    assert_eq!(codes(&bound, "bound.cui"), ["capability-narrowing-illegal"]);
    for url in [
        "https://other.example/x",
        "//other.example/x",
        "http://localhost:9/x",
    ] {
        let src = POSTER.replace("\"/api/notes\"", &format!("\"{url}\""));
        assert_eq!(
            codes(&src, "host.cui"),
            ["capability-bound-violated"],
            "{url}"
        );
    }
    let same = POSTER.replace("\"/api/notes\"", "\"api/notes?x=1\"");
    let file = scratch("same.cui", &same);
    let (code, stdout, stderr) = run(&["check", p(&file)]);
    assert_eq!(code, 0, "{stdout}{stderr}");
}

#[test]
fn a_trace_scripts_the_envelope_and_a_newer_post_drops_the_older_answer() {
    let file = scratch("trace.cui", POSTER);
    let trace = scratch(
        "trace.trace.json",
        r#"{"schema":"cancho-ui-trace","version":"0.1.0","component":"Poster","steps":[
            {"event":"Save","expect":{"pending":[1]}},
            {"event":"Save","expect":{"pending":[1,2]}},
            {"resolve":1,"with":{"Ok":"{\"status\":201,\"body\":\"\"}"},"state":{"code":"-"}},
            {"resolve":2,"with":{"Ok":"{\"status\":409,\"body\":\"exists\"}"},"state":{"code":"409"}}]}"#,
    );
    let (code, stdout, stderr) = run(&["test", p(&file), p(&trace)]);
    assert_eq!(code, 0, "{stdout}{stderr}");
    assert!(
        stdout.contains("issued #2 net.post(\"/api/notes\", \"hello\")"),
        "{stdout}"
    );
}

#[test]
fn a_non_string_url_or_body_is_a_replay_type_error() {
    let src = POSTER.replace("note));", "1));");
    let file = scratch("badbody.cui", &src);
    let trace = scratch(
        "badbody.trace.json",
        r#"{"schema":"cancho-ui-trace","version":"0.1.0","component":"Poster","steps":[{"event":"Save","expect":{"pending":[1]}}]}"#,
    );
    let (code, stdout, _) = run(&["test", p(&file), p(&trace), "--output", "json"]);
    assert_ne!(code, 0);
    assert!(stdout.contains("replay-type-error"), "{stdout}");
}
