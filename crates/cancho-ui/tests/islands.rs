//! Sealed islands (docs/islands.md): every refusal of `tests/islands_corpus.json` is refused by its rule tag, the valid programs check, the page of a
//! program with an island carries `frame-src about:` and its frame as static HTML, the frame's document is the files byte for byte under a policy
//! that names their hashes, and `inspect effects`, `report`, `diff` and `test` each say the island. What the frame does in a browser is checked by
//! `scripts/check-islands.js`.
mod common;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

const CORPUS: &str = include_str!("../../../tests/islands_corpus.json");
const README: &str = include_str!("../../../spec/diagnostics/README.md");

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

fn corpus() -> Value {
    serde_json::from_str(CORPUS).expect("the corpus is JSON")
}

fn p(path: &Path) -> &str {
    path.to_str().expect("UTF-8 temp path")
}

/// SHA-256 by the system's `sha256sum`: not the compiler's own implementation.
fn sha256_of(bytes: &[u8]) -> String {
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

/// The bytes of a corpus file: a string, `{"fill": "x", "bytes": n}` or `{"base64": ".."}` (only the two bytes the corpus needs).
fn bytes_of(v: &Value) -> Vec<u8> {
    match v {
        Value::String(s) => s.as_bytes().to_vec(),
        Value::Object(o) if o.contains_key("fill") => {
            let n = o["bytes"].as_u64().expect("bytes") as usize;
            o["fill"].as_str().expect("fill").repeat(n).into_bytes()
        }
        Value::Object(o) if o.get("base64").and_then(Value::as_str) == Some("gP8=") => {
            vec![0x80, 0xff]
        }
        other => panic!("a corpus file is {other}"),
    }
}

/// `{sha:path}` and `{SHA:path}` replaced by the hash of the corpus file.
fn fill(c: &Value, text: &str) -> String {
    let mut out = text.to_string();
    for (name, v) in c["files"].as_object().expect("files") {
        let h = sha256_of(&bytes_of(v));
        out = out.replace(&format!("{{sha:{name}}}"), &h);
        out = out.replace(&format!("{{SHA:{name}}}"), &h.to_uppercase());
    }
    out
}

fn scratch_dir() -> PathBuf {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = common::scratch_root().join(format!("cancho-ui-islands-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir
}

/// A directory with the corpus files and a program in it.
fn workspace(c: &Value, program: &str) -> PathBuf {
    let dir = scratch_dir();
    for (name, v) in c["files"].as_object().expect("files") {
        std::fs::write(dir.join(name), bytes_of(v)).expect("a corpus file");
    }
    std::fs::write(dir.join("p.cui"), fill(c, program)).expect("the program");
    dir.join("p.cui")
}

fn source_of(c: &Value, case: &Value) -> String {
    if let Some(whole) = case["program"].as_str() {
        return whole.to_string();
    }
    let decl = case["decl"]
        .as_str()
        .unwrap_or(c["decl"].as_str().expect("decl"));
    let view = case["view"].as_str().expect("view");
    let extra = case["extra_state"].as_str().unwrap_or("");
    let comp = c["component"]
        .as_str()
        .expect("component")
        .replace("{view}", view)
        .replace("state n: Int = 0;", &format!("state n: Int = 0; {extra}"));
    format!("{decl}\n{comp}")
}

fn codes(file: &Path) -> (i32, Vec<String>) {
    let (code, out, _) = run(&["check", p(file), "--output", "json"]);
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

fn case_file(c: &Value, case: &Value) -> PathBuf {
    let file = workspace(c, &source_of(c, case));
    if let Some(dirs) = case["mkdir"].as_array() {
        for d in dirs {
            let path = file.parent().expect("dir").join(d.as_str().expect("dir"));
            let _ = std::fs::remove_file(&path);
            std::fs::create_dir_all(path).expect("a directory");
        }
    }
    if let Some(links) = case["symlinks"].as_object() {
        for (name, target) in links {
            std::os::unix::fs::symlink(
                target.as_str().expect("target"),
                file.parent().expect("dir").join(name),
            )
            .expect("a link");
        }
    }
    if let Some(module) = case["module"].as_object() {
        for (name, text) in module {
            std::fs::write(
                file.parent().expect("dir").join(name),
                fill(c, text.as_str().expect("module text")),
            )
            .expect("a module");
        }
    }
    file
}

#[test]
fn every_valid_program_of_the_corpus_checks() {
    let c = corpus();
    for case in c["valid"].as_array().expect("valid") {
        let file = case_file(&c, case);
        assert_eq!(codes(&file), (0, vec![]), "{}", case["name"]);
    }
}

#[test]
fn every_refusal_of_the_corpus_is_refused_by_its_tag() {
    let c = corpus();
    let mut tags = std::collections::BTreeSet::new();
    for case in c["refusals"].as_array().expect("refusals") {
        let file = case_file(&c, case);
        let want = case["code"].as_str().expect("code");
        assert_eq!(
            codes(&file),
            (1, vec![want.to_string()]),
            "{}",
            case["name"]
        );
        tags.insert(want.to_string());
    }
    // every tag the island module owns is reached
    for tag in TAGS {
        assert!(tags.contains(*tag), "no corpus case reaches `{tag}`");
    }
}

/// The tags of docs/islands.md section 8 that are refusals at build time. (`island-event-dropped` is a rule of the runtime, reached by
/// `scripts/check-islands.js`.)
const TAGS: &[&str] = &[
    "island-name-invalid",
    "island-name-conflict",
    "island-duplicate",
    "island-entry-duplicate",
    "island-file-duplicate",
    "island-path-invalid",
    "island-hash-invalid",
    "island-script-missing",
    "island-too-many",
    "island-size-missing",
    "island-size-invalid",
    "island-label-missing",
    "island-label-conflict",
    "island-prop-name",
    "island-prop-duplicate",
    "island-prop-type",
    "island-pointer-twice",
    "island-event-name",
    "island-event-duplicate",
    "island-event-type",
    "island-source-missing",
    "island-source-too-large",
    "island-too-large",
    "island-hash-mismatch",
    "island-source-not-text",
    "island-source-unsafe",
    "island-instance-children",
    "island-instance-label",
    "island-prop-unknown",
    "island-prop-pointer-bound",
    "island-prop-bound-twice",
    "island-prop-dynamic",
    "island-prop-missing",
    "island-event-undeclared",
    "island-event-args",
    "island-event-handler-missing",
    "island-event-arity",
    "island-event-bound-twice",
    "island-open-needs-dom",
    "island-open-pointer",
    "island-text-alternative-missing",
    "island-text-alternative-invalid",
    "island-prop-source-unknown",
    "island-open-instance-twice",
    "capability-unused",
];

#[test]
fn every_tag_is_documented() {
    for tag in TAGS.iter().chain(&["island-event-dropped"]) {
        assert!(
            README.contains(&format!("`{tag}`")),
            "spec/diagnostics/README.md does not document `{tag}`"
        );
    }
}

#[test]
fn a_program_cut_anywhere_is_refused_and_never_a_panic() {
    let c = corpus();
    let case = &c["valid"][0];
    let full = fill(&c, &source_of(&c, case));
    let dir = scratch_dir();
    for (name, v) in c["files"].as_object().expect("files") {
        std::fs::write(dir.join(name), bytes_of(v)).expect("a corpus file");
    }
    let cuts: Vec<usize> = full.char_indices().map(|(i, _)| i).step_by(7).collect();
    for cut in cuts {
        let file = dir.join("cut.cui");
        std::fs::write(&file, &full[..cut]).expect("a cut program");
        let (code, _, err) = run(&["check", p(&file)]);
        assert!(
            code == 0 || code == 1,
            "cut at {cut}: exit {code}\n{err}\n{}",
            &full[..cut]
        );
        assert!(!err.contains("panicked"), "cut at {cut}: {err}");
    }
}

const PROGRAM: &str = r#"island Stones {
    script: "a.js" sha256: "{sha:a.js}";
    script: "b.js" sha256: "{sha:b.js}";
    style: "s.css" sha256: "{sha:s.css}";
    label: "Stones";
    width: 100;
    height: 50;
    props { seed: Int; pointer: Pointer; }
    events { Picked: Int; Closed; }
}
component C {
    state n: Int = 0;
    on Picked(v) { n = v; }
    on Closed { n = 0; }
    view { Column { Heading { level: 1 "Hi" } Stones { seed: 1 on: Picked } Text { "n = {n}" } } }
}
"#;

fn built(program: &str, flags: &[&str]) -> (PathBuf, String, String) {
    let c = corpus();
    let file = workspace(&c, program);
    let out = file.parent().expect("dir").join("out");
    let mut args = vec!["build", p(&file)];
    args.extend_from_slice(flags);
    args.extend_from_slice(&["-o", p(&out)]);
    let (code, _, err) = run(&args);
    assert_eq!(code, 0, "{err}");
    let html = std::fs::read_to_string(out.join("index.html")).expect("index.html");
    let js = std::fs::read_to_string(out.join("app.js")).expect("app.js");
    (file, html, js)
}

#[test]
fn the_page_of_a_program_with_an_island_carries_frame_src_about() {
    let (_, html, _) = built(PROGRAM, &["--page"]);
    assert!(
        html.contains("<meta http-equiv=\"Content-Security-Policy\" content=\"frame-src about:\">"),
        "{}",
        &html[..400]
    );
    // the same for an app
    let (_, html, _) = built(PROGRAM, &["--pwa"]);
    assert!(html.contains("content=\"frame-src about:\""));
    // a program with no island has no policy, and an island that is declared and not used does not make one
    let (_, html, _) = built(
        "component C { view { Column { Text { \"x\" } } } }\n",
        &["--page"],
    );
    assert!(!html.contains("Content-Security-Policy"));
    let unused = PROGRAM.replace("Stones { seed: 1 on: Picked }", "Text { \"-\" }");
    let (_, html, _) = built(&unused, &["--page"]);
    assert!(
        !html.contains("Content-Security-Policy"),
        "an unused island"
    );
}

#[test]
fn the_frame_is_static_html_and_its_document_is_the_files_under_their_hashes() {
    let c = corpus();
    let (_, html, js) = built(PROGRAM, &["--page"]);
    // the frame is in the page as written, with the sandbox, the label and the data the runtime reads
    assert!(
        html.contains("<iframe sandbox=\"allow-scripts\" title=\"Stones\""),
        "{html}"
    );
    assert!(html.contains("data-cui-island=\"{&quot;events&quot;:{&quot;Picked&quot;:{&quot;type&quot;:&quot;Int&quot;}}"));
    assert!(
        html.contains("srcdoc=\"&lt;!doctype html&gt;") || html.contains("srcdoc=\"&lt;!doctype")
    );
    // the script builds no frame: the page already has it (the text of the island's document is not in app.js)
    assert!(
        !js.contains("allow-scripts"),
        "a hydrated page builds its static frame in script"
    );
    assert!(
        !js.contains("cancho.ready"),
        "the document of the frame is in the script"
    );
    // the document: both scripts, byte for byte, in order, between the shim and the line that asks for the props, and a policy naming each by hash
    let doc = html
        .split("srcdoc=\"")
        .nth(1)
        .expect("srcdoc")
        .split('"')
        .next()
        .expect("end");
    let doc = doc
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&amp;", "&");
    let (a, b) = (
        String::from_utf8(bytes_of(&c["files"]["a.js"])).expect("a"),
        String::from_utf8(bytes_of(&c["files"]["b.js"])).expect("b"),
    );
    let at = |s: &str| {
        doc.find(s)
            .unwrap_or_else(|| panic!("`{s}` is not in the document\n{doc}"))
    };
    assert!(at("RTCPeerConnection") < at(&format!("<script>{a}</script>")));
    assert!(at(&format!("<script>{a}</script>")) < at(&format!("<script>{b}</script>")));
    assert!(at(&format!("<script>{b}</script>")) < at("<script>cancho.ready();</script>"));
    assert!(doc.contains("<style>body { margin: 0; }\n</style>"));
    let policy = doc
        .split("content=\"")
        .nth(1)
        .expect("policy")
        .split('"')
        .next()
        .expect("end");
    assert!(!policy.contains("unsafe-inline'") || policy.contains("style-src 'unsafe-inline'"));
    assert!(!policy.contains("script-src 'unsafe-inline'"), "{policy}");
    assert_eq!(policy.matches("'sha256-").count(), 4, "{policy}");
    for part in [
        "default-src 'none'",
        "form-action 'none'",
        "base-uri 'none'",
        "frame-src 'none'",
    ] {
        assert!(policy.contains(part), "{policy}");
    }
}

#[test]
fn an_island_is_a_static_element_of_the_skeleton_and_not_a_hole() {
    let c = corpus();
    let file = workspace(&c, PROGRAM);
    let (code, out, _) = run(&["inspect", "prerender", p(&file), "C"]);
    assert_eq!(code, 0);
    let v: Value = serde_json::from_str(&out).expect("json");
    let holes: Vec<String> = v["holes"]
        .as_array()
        .expect("holes")
        .iter()
        .map(|h| h["what"].as_str().expect("what").to_string())
        .collect();
    assert_eq!(
        holes,
        vec!["Text"],
        "only the text that reads state is a hole: {out}"
    );
    assert_eq!(v["hydrates"], true);
}

#[test]
fn the_skeleton_changes_when_a_file_of_the_island_changes() {
    let c = corpus();
    let (_, html_a, _) = built(PROGRAM, &["--page"]);
    // the same program with another second script: another document, so another skeleton, so the script of the other build leaves the page alone
    let other = PROGRAM
        .replace("b.js", "c.js")
        .replace("{sha:b.js}", "{sha:c.js}");
    let (_, html_b, _) = built(&other, &["--page"]);
    let hash = |h: &str| {
        h.split("data-cui-skeleton=\"")
            .nth(1)
            .expect("hash")
            .split('"')
            .next()
            .expect("end")
            .to_string()
    };
    assert_ne!(hash(&html_a), hash(&html_b));
    let _ = c;
}

#[test]
fn inspect_effects_and_report_say_the_island_and_what_the_sandbox_does_not_bound() {
    let c = corpus();
    let file = workspace(&c, PROGRAM);
    let (code, out, _) = run(&["inspect", "effects", p(&file), "C"]);
    assert_eq!(code, 0);
    let v: Value = serde_json::from_str(&out).expect("json");
    let i = &v["islands"][0];
    assert_eq!(i["name"], "Stones");
    assert_eq!(i["sealed"], true);
    assert_eq!(i["files"].as_array().expect("files").len(), 3);
    assert_eq!(
        i["files"][0]["sha256"],
        sha256_of(&bytes_of(&c["files"]["a.js"]))
    );
    assert_eq!(i["props"][1], "pointer: Pointer");
    assert_eq!(i["events"][1], "Closed");
    assert!(i["not_bounded"][0].as_str().expect("t").starts_with("time"));
    assert_eq!(
        v["requires"].as_array().expect("requires").len(),
        0,
        "a sealed island is not a capability"
    );
    let (code, out, _) = run(&["report", p(&file)]);
    assert_eq!(code, 0);
    let r: Value = serde_json::from_str(&out).expect("json");
    assert_eq!(r["trust"]["count"], 1);
    assert_eq!(r["trust"]["boundaries"][0]["kind"], "sealed island");
    assert_eq!(r["trust"]["boundaries"][0]["name"], "Stones");
}

#[test]
fn diff_ranks_an_island_first_and_leaves_authority_unchanged() {
    let c = corpus();
    let a = workspace(&c, PROGRAM);
    // a file that changed
    let changed = b"var x = 2;\n";
    let b = workspace(&c, &PROGRAM.replace("{sha:a.js}", &sha256_of(changed)));
    std::fs::write(b.parent().expect("dir").join("a.js"), changed).expect("a changed file");
    let (code, out, _) = run(&["diff", p(&a), p(&b), "--output", "json"]);
    assert_eq!(code, 5, "{out}");
    let v: Value = serde_json::from_str(&out).expect("json");
    assert_eq!(v["authority"], "unchanged");
    let settings: Vec<&str> = v["islands"]
        .as_array()
        .expect("islands")
        .iter()
        .map(|s| s["setting"].as_str().expect("setting"))
        .collect();
    assert_eq!(
        settings,
        vec!["Stones.script a.js"],
        "only the file that changed"
    );
    let change = &v["islands"][0];
    assert_eq!(change["base"], sha256_of(&bytes_of(&c["files"]["a.js"])));
    assert_eq!(change["candidate"], sha256_of(changed));
    let (_, human, _) = run(&["diff", p(&a), p(&b)]);
    let line = human.lines().position(|l| {
        l.starts_with("~ island") || l.starts_with("+ island") || l.starts_with("- island")
    });
    let comp = human.lines().position(|l| l.contains("component"));
    assert!(line.is_some(), "{human}");
    assert!(
        comp.is_none() || line < comp,
        "an island comes first\n{human}"
    );
    // an island added
    let none =
        "component C {\n    state n: Int = 0;\n    view { Column { Text { \"n = {n}\" } } }\n}\n";
    let base = workspace(&c, none);
    let (code, out, _) = run(&["diff", p(&base), p(&a), "--output", "json"]);
    assert_eq!(code, 5, "{out}");
    let v: Value = serde_json::from_str(&out).expect("json");
    assert_eq!(
        v["authority"], "unchanged",
        "a sealed island reaches nothing: {out}"
    );
    assert!(
        v["islands"]
            .as_array()
            .expect("islands")
            .iter()
            .any(|s| s["setting"] == "Stones" && s["base"].is_null())
    );
}

#[test]
fn test_says_an_island_is_not_replayed_and_the_handler_runs_from_the_trace() {
    let c = corpus();
    let file = workspace(&c, PROGRAM);
    let trace = file.parent().expect("dir").join("t.json");
    std::fs::write(
        &trace,
        r#"{"schema":"cancho-ui-trace","version":"0.1.0","steps":[{"event":"Picked","params":[2],"state":{"n":2}}]}"#,
    )
    .expect("trace");
    let (code, out, _) = run(&["test", p(&file), p(&trace)]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("island Stones: not replayed"), "{out}");
    let (_, out, _) = run(&["test", p(&file), p(&trace), "--output", "json"]);
    let v: Value = serde_json::from_str(&out).expect("json");
    assert_eq!(v["islands_not_replayed"][0], "Stones");
}

#[test]
fn the_examples_island_is_pinned_to_the_files_beside_it() {
    let (code, _, err) = run(&["check", "examples/islands/islands.cui"]);
    assert_eq!(code, 0, "{err}");
}

/// A file a test or an example needs must be in the repository: a clean clone has no ignored file. (`*.js` is ignored in `.gitignore`, which is how
/// `examples/islands/assets/stones.js` and `tests/fixtures/islands/reach.js` once passed locally and failed in CI.)
#[test]
fn no_file_of_the_examples_or_the_fixtures_is_ignored_by_git() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    if Command::new("git").arg("--version").output().is_err() || !root.join(".git").exists() {
        return;
    }
    fn files(dir: &Path, out: &mut Vec<PathBuf>) {
        for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                files(&p, out);
            } else {
                out.push(p);
            }
        }
    }
    let mut all = Vec::new();
    files(&root.join("examples"), &mut all);
    files(&root.join("tests"), &mut all);
    assert!(all.len() > 50);
    let ignored: Vec<String> = all
        .iter()
        .filter(|f| {
            Command::new("git")
                .current_dir(&root)
                .args(["check-ignore", "-q"])
                .arg(f)
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
        })
        .map(|f| f.display().to_string())
        .collect();
    assert!(
        ignored.is_empty(),
        "ignored by git, so absent from a clone: {ignored:?}"
    );
}

const OPEN: &str = r#"island Places {
    sealed: false;
    script: "a.js" sha256: "{sha:a.js}";
    style: "s.css" sha256: "{sha:s.css}";
    label: "Places";
    width: 100;
    height: 50;
    props { seed: Int; }
    events { Picked: Int; }
}
component C {
    requires Dom;
    state n: Int = 0;
    on Picked(v) { n = v; }
    view { Column { Places { seed: n text: "alt" on: Picked } Section { label: "Alt" id: "alt" Text { "alt" } } } }
}
"#;

#[test]
fn an_open_island_is_a_div_of_the_page_and_adds_no_frame_policy() {
    let (_, html, js) = built(OPEN, &["--page"]);
    assert!(!html.contains("<iframe"), "an open island has no frame");
    assert!(
        !html.contains("Content-Security-Policy"),
        "an open island adds no policy"
    );
    assert!(
        html.contains("<div role=\"group\" aria-label=\"Places\" aria-describedby=\"alt\""),
        "{html}"
    );
    assert!(html.contains("data-cui-island=\"{&quot;dynamic&quot;:[&quot;seed&quot;]"));
    // its files are in the script, to be run in the page
    assert!(js.contains("const __cui_open_src = {"));
    assert!(js.contains("var x = 1;"));
}

#[test]
fn inspect_effects_and_report_say_unbounded_for_an_open_island() {
    let c = corpus();
    let file = workspace(&c, OPEN);
    let (code, out, _) = run(&["inspect", "effects", p(&file), "C"]);
    assert_eq!(code, 0, "{out}");
    let v: Value = serde_json::from_str(&out).expect("json");
    let i = &v["islands"][0];
    assert_eq!(i["sealed"], false);
    assert!(
        i["authority"]
            .as_str()
            .expect("authority")
            .starts_with("UNBOUNDED")
    );
    assert!(
        i["reaches"]
            .as_str()
            .expect("reaches")
            .contains("everything the page can")
    );
    assert!(i.get("not_bounded").is_some());
    assert_eq!(v["requires"][0]["name"], "Dom");
    let (_, out, _) = run(&["report", p(&file)]);
    let r: Value = serde_json::from_str(&out).expect("json");
    assert_eq!(r["trust"]["boundaries"][0]["kind"], "open island");
    assert!(
        r["trust"]["boundaries"][0]["authority"]
            .as_str()
            .expect("a")
            .starts_with("UNBOUNDED")
    );
}

#[test]
fn an_open_island_is_a_static_element_of_the_skeleton() {
    let c = corpus();
    let file = workspace(&c, OPEN);
    let (_, out, _) = run(&["inspect", "prerender", p(&file), "C"]);
    let v: Value = serde_json::from_str(&out).expect("json");
    let holes: Vec<&str> = v["holes"]
        .as_array()
        .expect("holes")
        .iter()
        .map(|h| h["what"].as_str().expect("w"))
        .collect();
    assert!(holes.is_empty(), "{out}");
    assert_eq!(v["hydrates"], true);
}

#[test]
fn diff_marks_an_open_island_as_expanded_authority() {
    let c = corpus();
    let none =
        "component C {\n    state n: Int = 0;\n    view { Column { Text { \"n = {n}\" } } }\n}\n";
    let base = workspace(&c, none);
    let open = workspace(&c, OPEN);
    let (code, out, _) = run(&["diff", p(&base), p(&open), "--output", "json"]);
    assert_eq!(code, 5, "{out}");
    let v: Value = serde_json::from_str(&out).expect("json");
    assert_eq!(v["authority"], "expanded");
    assert!(
        v["islands"]
            .as_array()
            .expect("islands")
            .iter()
            .any(|s| s["setting"] == "Places" && s["candidate"] == "open")
    );
    // the same island sealed, opened: authority expands; opened, sealed: it reduces
    let sealed_src = OPEN
        .replace("    sealed: false;\n", "")
        .replace("    requires Dom;\n", "")
        .replace(" text: \"alt\"", "")
        .replace("seed: n", "seed: 1");
    let sealed = workspace(&c, &sealed_src);
    let (code, out, _) = run(&["diff", p(&sealed), p(&open), "--output", "json"]);
    assert_eq!(code, 5, "{out}");
    let v: Value = serde_json::from_str(&out).expect("json");
    assert_eq!(v["authority"], "expanded", "{out}");
    assert!(
        v["islands"]
            .as_array()
            .expect("islands")
            .iter()
            .any(|s| s["setting"] == "Places" && s["base"] == "sealed" && s["candidate"] == "open")
    );
    let (_, out, _) = run(&["diff", p(&open), p(&sealed), "--output", "json"]);
    let v: Value = serde_json::from_str(&out).expect("json");
    assert_eq!(v["authority"], "reduced", "{out}");
    // a sealed island added to a program leaves authority alone (the sealed path is as it was)
    let (_, out, _) = run(&["diff", p(&base), p(&sealed), "--output", "json"]);
    let v: Value = serde_json::from_str(&out).expect("json");
    assert_eq!(v["authority"], "unchanged", "{out}");
}

#[test]
fn the_page_policy_of_a_program_with_both_kinds_is_the_sealed_islands_alone() {
    let both = format!(
        "{}\n{}",
        PROGRAM.split("component C").next().expect("island"),
        "island Places {\n    sealed: false;\n    script: \"a.js\" sha256: \"{sha:a.js}\";\n    decorative: true;\n    width: 10;\n    height: 10;\n}\ncomponent C {\n    requires Dom;\n    state n: Int = 0;\n    on Picked(v) { n = v; }\n    view { Column { Stones { seed: 1 on: Picked } Places { } } }\n}\n"
    );
    let (_, html, _) = built(&both, &["--page"]);
    assert!(html.contains("content=\"frame-src about:\""));
    assert_eq!(
        html.matches("<iframe").count(),
        1,
        "one frame, for the sealed island"
    );
}

#[test]
fn the_example_map_is_pinned_to_the_files_beside_it() {
    let (code, _, err) = run(&["check", "examples/map-island/map.cui"]);
    assert_eq!(code, 0, "{err}");
}

#[test]
fn opening_a_sealed_island_expands_authority_by_itself() {
    // both programs declare `requires Dom` (one island is open in both), so the capability does not move: only the island does
    let c = corpus();
    let src = |other_open: bool| {
        format!(
            "island Places {{\n    sealed: false;\n    script: \"a.js\" sha256: \"{{sha:a.js}}\";\n    decorative: true;\n    width: 10;\n    height: 10;\n}}\nisland Other {{\n    {}\n    script: \"b.js\" sha256: \"{{sha:b.js}}\";\n    decorative: true;\n    width: 10;\n    height: 10;\n}}\ncomponent C {{\n    requires Dom;\n    view {{ Column {{ Places {{ }} Other {{ }} }} }}\n}}\n",
            if other_open { "sealed: false;" } else { "" }
        )
    };
    let sealed = workspace(&c, &src(false));
    let opened = workspace(&c, &src(true));
    let (code, out, _) = run(&["diff", p(&sealed), p(&opened), "--output", "json"]);
    assert_eq!(code, 5, "{out}");
    let v: Value = serde_json::from_str(&out).expect("json");
    assert_eq!(v["authority"], "expanded", "{out}");
    assert_eq!(
        v["capabilities"]["added"].as_array().expect("caps").len(),
        0,
        "{out}"
    );
    let (_, out, _) = run(&["diff", p(&opened), p(&sealed), "--output", "json"]);
    let v: Value = serde_json::from_str(&out).expect("json");
    assert_eq!(v["authority"], "reduced", "{out}");
}
