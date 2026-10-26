//! `data Name { file sha256 }` (docs/data.md): every refusal of `tests/data_corpus.json` is refused by its rule tag, hostile files are refusals and never a
//! panic, the valid programs check, the page has the elements a `For` over the data stands for before any script, no tool needs a capability or a request
//! for it, and every tool that looks at the program sees the same elements. What a browser does with the page is checked by `scripts/check-data.js`.
mod common;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

const CORPUS: &str = include_str!("../../../tests/data_corpus.json");
const README: &str = include_str!("../../../spec/diagnostics/README.md");

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

fn corpus() -> Value {
    serde_json::from_str(CORPUS).expect("the corpus is JSON")
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

fn unhex(h: &str) -> Vec<u8> {
    (0..h.len() / 2)
        .map(|i| u8::from_str_radix(&h[2 * i..2 * i + 2], 16).expect("hex"))
        .collect()
}

fn bytes_of(v: &Value) -> Vec<u8> {
    match v {
        Value::String(s) => s.as_bytes().to_vec(),
        Value::Object(o) if o.contains_key("hex") => unhex(o["hex"].as_str().expect("hex")),
        Value::Object(o) if o.contains_key("fill") => o["fill"]
            .as_str()
            .expect("fill")
            .repeat(o["bytes"].as_u64().expect("bytes") as usize)
            .into_bytes(),
        Value::Object(o) if o.contains_key("gen") => {
            let n = o["count"].as_u64().expect("count");
            let items: Vec<String> = (0..n).map(|i| format!("{{\"name\":\"n{i}\"}}")).collect();
            format!("{{\"items\":[{}]}}", items.join(",")).into_bytes()
        }
        Value::Object(o) if o.contains_key("nest") => {
            let n = o["nest"].as_u64().expect("nest") as usize;
            format!("{{\"items\":{}1{}}}", "[".repeat(n), "]".repeat(n)).into_bytes()
        }
        Value::Object(o) if o.contains_key("huge") => format!(
            "{{\"items\":[{{\"name\":\"{}\"}}]}}",
            "x".repeat(o["huge"].as_u64().expect("huge") as usize)
        )
        .into_bytes(),
        other => panic!("a corpus file is {other}"),
    }
}

fn scratch_dir() -> PathBuf {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = common::scratch_root().join(format!("cancho-ui-data-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir
}

fn fill(c: &Value, text: &str) -> String {
    let mut out = text.to_string();
    for (name, v) in c["files"].as_object().expect("files") {
        if out.contains(&format!("{{sha:{name}}}")) || out.contains(&format!("{{SHA:{name}}}")) {
            let h = sha256_of(&bytes_of(v));
            out = out.replace(&format!("{{sha:{name}}}"), &h);
            out = out.replace(&format!("{{SHA:{name}}}"), &h.to_uppercase());
        }
    }
    out
}

fn source(c: &Value, case: &Value) -> String {
    let text = c["component"]
        .as_str()
        .expect("component")
        .replace("{decl}", case["decl"].as_str().expect("decl"))
        .replace("{state}", case["state"].as_str().unwrap_or(""))
        .replace("{view}", case["view"].as_str().expect("view"));
    fill(c, &text)
}

fn workspace(c: &Value, program: &str) -> PathBuf {
    let dir = scratch_dir();
    for (name, v) in c["files"].as_object().expect("files") {
        std::fs::write(dir.join(name), bytes_of(v)).expect("a corpus file");
    }
    std::fs::write(dir.join("p.cui"), program).expect("the program");
    dir.join("p.cui")
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

const NEW: &[&str] = &[
    "data-duplicate",
    "data-path-invalid",
    "data-hash-invalid",
    "data-file-missing",
    "data-hash-mismatch",
    "data-malformed",
    "data-too-deep",
    "data-too-large",
    "data-unknown",
    "data-use-invalid",
    "data-shape-invalid",
    "data-too-many-rows",
    "data-expansion-too-large",
];
const OLD: &[&str] = &[
    "link-scheme-refused",
    "link-to-invalid",
    "waffle-of-invalid",
    "countup-out-of-range",
    "countup-not-number",
];

#[test]
fn every_valid_program_of_the_corpus_checks() {
    let c = corpus();
    for case in c["valid"].as_array().expect("valid") {
        let f = workspace(&c, &source(&c, case));
        assert_eq!(codes(&f), (0, vec![]), "{}", case["name"]);
    }
}

#[test]
fn every_refusal_of_the_corpus_is_refused_by_its_tag() {
    let c = corpus();
    for case in c["refusals"].as_array().expect("refusals") {
        let f = workspace(&c, &source(&c, case));
        let (code, found) = codes(&f);
        let tag = case["tag"].as_str().expect("tag");
        assert_eq!(code, 1, "{}: {found:?}", case["name"]);
        assert_eq!(found, vec![tag.to_string()], "{}", case["name"]);
    }
}

#[test]
fn every_tag_is_documented_and_reached() {
    let c = corpus();
    let reached: Vec<&str> = c["refusals"]
        .as_array()
        .expect("refusals")
        .iter()
        .filter_map(|r| r["tag"].as_str())
        .collect();
    for tag in NEW.iter().chain(OLD) {
        assert!(reached.contains(tag), "no case reaches `{tag}`");
        assert!(
            README.contains(&format!("`{tag}`")),
            "`{tag}` is not documented"
        );
    }
    for tag in NEW {
        assert!(
            README.contains(&format!("`{tag}` | `Component`")),
            "`{tag}` has no row of its own"
        );
    }
}

#[test]
fn a_program_cut_anywhere_is_refused_and_never_a_panic() {
    let c = corpus();
    let full = source(&c, &c["valid"][1]);
    let file = workspace(&c, &full);
    let dir = file.parent().expect("dir").to_path_buf();
    let mut cuts: Vec<usize> = full.char_indices().map(|(i, _)| i).collect();
    cuts.push(full.len());
    for (n, at) in cuts.iter().enumerate() {
        let f = dir.join(format!("cut{n}.cui"));
        std::fs::write(&f, &full[..*at]).expect("cut");
        let (code, _, err) = run(&["check", p(&f)]);
        assert!(code == 0 || code == 1, "prefix {at}: exit {code}: {err}");
        if let Some(next) = cuts.get(n + 1) {
            let g = dir.join(format!("del{n}.cui"));
            std::fs::write(&g, format!("{}{}", &full[..*at], &full[*next..])).expect("cut");
            let (code, _, err) = run(&["check", p(&g)]);
            assert!(
                code == 0 || code == 1,
                "deleting at {at}: exit {code}: {err}"
            );
        }
    }
}

#[test]
fn every_prefix_of_every_hostile_file_is_a_refusal_and_never_a_panic() {
    let c = corpus();
    let case = &c["valid"][0];
    for name in [
        "x.json",
        "hostile.json",
        "bigints.json",
        "unicode.json",
        "dup.json",
    ] {
        let bytes = bytes_of(&c["files"][name]);
        let dir = scratch_dir();
        let program = source(&c, case).replace("x.json", "d.json");
        for n in 0..bytes.len() {
            let cut = &bytes[..n];
            std::fs::write(dir.join("d.json"), cut).expect("a file");
            let hash = sha256_of(cut);
            let text = program.replacen(&sha256_of(&bytes_of(&c["files"]["x.json"])), &hash, 1);
            std::fs::write(
                dir.join("p.cui"),
                text.replace(&sha256_of(&bytes_of(&c["files"][name])), &hash),
            )
            .expect("p");
            let (code, _, err) = run(&["check", p(&dir.join("p.cui"))]);
            assert!(
                code == 0 || code == 1,
                "{name} cut at {n}: exit {code}: {err}"
            );
        }
    }
}

fn program_with(c: &Value, view: &str, file: &str) -> PathBuf {
    let case = serde_json::json!({ "decl": format!("data X {{ file: \"{file}\" sha256: \"{{sha:{file}}}\"; }}"), "view": view, "state": "" });
    workspace(c, &source(c, &case))
}

const LIST: &str = "For { over: data.items(X, \"items\") as: p Rich { Link { to: json.get(p, \"url\", \"\") \"{json.get(p, \\\"name\\\", \\\"\\\")}\" } \" \" Chip { tone: \"accent\" \"{json.get(p, \\\"stage\\\", \\\"\\\")}\" } } }";

fn page(f: &Path) -> (PathBuf, String) {
    let out = f.parent().expect("dir").join("out");
    let _ = std::fs::remove_dir_all(&out);
    let (code, o, e) = run(&["build", p(f), "--page", "-o", p(&out)]);
    assert_eq!(code, 0, "{o}{e}");
    let html = std::fs::read_to_string(out.join("index.html")).expect("index.html");
    (out, html)
}

#[test]
fn the_page_has_the_elements_before_any_script_and_needs_nothing() {
    let c = corpus();
    let f = program_with(&c, LIST, "x.json");
    let (out, html) = page(&f);
    let root = html
        .split("<body>")
        .nth(1)
        .expect("body")
        .split("<script")
        .next()
        .expect("root");
    assert!(
        root.contains("<p><a href=\"https://example.org/a\">alpha</a> <span class=\"cui-chip cui-chip-accent\">built</span></p><p><a href=\"https://example.org/b\">beta</a> <span class=\"cui-chip cui-chip-accent\">alpha</span></p>"),
        "{root}"
    );
    assert!(
        !html.contains("need JavaScript"),
        "nothing is left to a script"
    );
    let js = std::fs::read_to_string(out.join("app.js")).expect("app.js");
    for banned in [
        "net.get",
        "fetch(",
        "data.items",
        "XMLHttpRequest",
        "x.json",
    ] {
        assert!(!js.contains(banned) && !html.contains(banned), "{banned}");
    }
    assert!(
        !out.join("x.json").exists(),
        "the data file is not copied: the page does not read it"
    );
    let (_, eff, _) = run(&["inspect", "effects", p(&f), "C"]);
    let v: Value = serde_json::from_str(&eff).expect("JSON");
    assert_eq!(v["requires"], serde_json::json!([]));
    assert_eq!(v["reachable_effects"], serde_json::json!([]));
    assert_eq!(
        v["links"],
        serde_json::json!(["example.org"]),
        "the hosts the data links to are listed like any link's"
    );
}

#[test]
fn inspect_effects_and_inspect_data_name_the_file_by_its_hash() {
    let c = corpus();
    let f = program_with(&c, LIST, "x.json");
    let want = sha256_of(&bytes_of(&c["files"]["x.json"]));
    let (_, eff, _) = run(&["inspect", "effects", p(&f), "C"]);
    let v: Value = serde_json::from_str(&eff).expect("JSON");
    assert_eq!(v["data"][0]["name"], "X");
    assert_eq!(v["data"][0]["file"], "x.json");
    assert_eq!(v["data"][0]["sha256"], want.as_str());
    assert_eq!(v["data"][0]["bytes"], bytes_of(&c["files"]["x.json"]).len());
    assert_eq!(v["data"][0]["rows"], 2);
    let (code, d, _) = run(&["inspect", "data", p(&f)]);
    assert_eq!(code, 0);
    let v: Value = serde_json::from_str(&d).expect("JSON");
    assert_eq!(v[0]["components"], serde_json::json!(["C"]));
    // a program that declares none has none to list
    let none = workspace(&c, "component C {\n    view { Text { \"x\" } }\n}\n");
    let (_, eff, _) = run(&["inspect", "effects", p(&none), "C"]);
    assert!(!eff.contains("\"data\""));
}

#[test]
fn every_tool_sees_the_expanded_view() {
    let c = corpus();
    let f = program_with(
        &c,
        "For { over: data.items(X, \"items\") as: p Text { \"{json.get(p, \\\"name\\\", \\\"\\\")}\" } } Button { label: \"Tick\" on: Tick }",
        "x.json",
    );
    // `inspect component`: the IR holds the texts, and no `For`, no `data.items`
    let (_, ir, _) = run(&["inspect", "component", p(&f), "C"]);
    assert!(ir.contains("alpha") && ir.contains("beta"), "{ir}");
    assert!(
        !ir.contains("data.items") && !ir.contains("\"For\""),
        "{ir}"
    );
    // `test`: replay runs the handlers of the program whose view was written out
    let trace = f.parent().expect("dir").join("t.trace.json");
    std::fs::write(&trace, "{\"schema\":\"cancho-ui-trace\",\"version\":\"0.1.0\",\"steps\":[{\"event\":\"Tick\",\"state\":{\"n\":1}}]}").expect("trace");
    let (code, out, err) = run(&["test", p(&f), p(&trace)]);
    assert_eq!(code, 0, "{out}{err}");
    // `build` to a module (no page): the same elements, drawn by the module with no data in it
    let (_, js, _) = run(&["build", p(&f)]);
    assert!(js.contains("alpha") && js.contains("beta") && !js.contains("data.items"));
}

#[test]
fn a_changed_file_is_a_changed_program_and_diff_says_so() {
    let c = corpus();
    let by_stage = "For { over: data.items(X, \"items\") as: p Rich { Link { to: json.get(p, \"url\", \"\") \"{json.get(p, \\\"name\\\", \\\"\\\")}\" } \" \" If { when: json.get(p, \"stage\", \"\") == \"built\" Chip { tone: \"accent\" \"built\" } } If { when: json.get(p, \"stage\", \"\") == \"alpha\" Chip { tone: \"warn\" \"alpha\" } } } }";
    let base = program_with(&c, by_stage, "x.json");
    let dir = base.parent().expect("dir").to_path_buf();
    // the candidate: the file with one stage changed, and the hash the program pins
    let mut data: Value = serde_json::from_slice(&bytes_of(&c["files"]["x.json"])).expect("json");
    data["items"][0]["stage"] = Value::String("alpha".into());
    let new = serde_json::to_vec(&data).expect("bytes");
    let old_hash = sha256_of(&bytes_of(&c["files"]["x.json"]));
    let cand_dir = scratch_dir();
    std::fs::write(cand_dir.join("x.json"), &new).expect("file");
    let src = std::fs::read_to_string(&base)
        .expect("src")
        .replace(&old_hash, &sha256_of(&new));
    std::fs::write(cand_dir.join("p.cui"), src).expect("cand");
    let cand = cand_dir.join("p.cui");
    let (code, human, err) = run(&["diff", p(&base), p(&cand)]);
    assert_eq!(code, 5, "{human}{err}");
    assert!(human.contains("~ data X.sha256:"), "{human}");
    assert!(human.contains("~ data X.bytes:"), "{human}");
    assert!(human.contains("authority: unchanged"), "{human}");
    // the elements the data writes are compared like any element: the chip that was `accent` is `warn`. Their text is not (`view_text_content` is in
    // `not_compared`), which is why the file's own hash and size are lines of the report
    assert!(
        human.contains("+ view Chip tone=\"warn\"")
            && human.contains("- view Chip tone=\"accent\""),
        "{human}"
    );
    let (code, out, _) = run(&["diff", p(&base), p(&cand), "--output", "json"]);
    assert_eq!(code, 5);
    let v: Value = serde_json::from_str(&out).expect("JSON");
    assert_eq!(v["data"][0]["setting"], "X.bytes");
    assert_eq!(
        v["change_count"], 4,
        "two elements and two settings of the file: {out}"
    );
    assert_eq!(v["authority"], "unchanged");
    // the same file edited and the pin left alone is not a build: the file is not what the program says it is
    std::fs::write(dir.join("x.json"), &new).expect("edit");
    let (code, found) = codes(&base);
    assert_eq!((code, found), (1, vec!["data-hash-mismatch".to_string()]));
}

#[test]
fn text_from_the_data_is_text_and_never_markup() {
    let c = corpus();
    let f = program_with(
        &c,
        "For { over: data.items(X, \"items\") as: p Text { \"{json.get(p, \\\"name\\\", \\\"\\\")}\" } }",
        "hostile.json",
    );
    let (_, html) = page(&f);
    assert!(
        html.contains("&lt;script&gt;alert(1)&lt;/script&gt; &amp; \"q\" 'r'"),
        "{html}"
    );
    assert!(!html.contains("<script>alert"));
    // duplicate keys: the last one, as `JSON.parse` reads them
    let f = program_with(
        &c,
        "For { over: data.items(X, \"items\") as: p Text { \"{json.get(p, \\\"name\\\", \\\"\\\")}\" } }",
        "dup.json",
    );
    let (_, html) = page(&f);
    assert!(
        html.contains("<span>two</span>") && !html.contains("<span>one</span>"),
        "{html}"
    );
}

#[test]
fn numbers_of_the_data_are_written_as_the_page_would_write_them() {
    let c = corpus();
    let f = program_with(
        &c,
        "For { over: data.items(X, \"items\") as: p Text { \"{num(json.get(p, \\\"n\\\", \\\"0\\\"), 0) + 1}\" } }",
        "bigints.json",
    );
    let (_, html) = page(&f);
    // 9007199254740993 is read as the double 9007199254740992 (what `JSON.parse` gives) and 1 more is the same double
    assert!(html.contains("<span>9007199254740992</span>"), "{html}");
    let g = program_with(
        &c,
        "For { over: data.items(X, \"items\") as: p Text { \"{num(json.get(p, \\\"s\\\", \\\"0\\\"), 0) + 1} {num(json.get(p, \\\"k\\\", \\\"0\\\"), 0)}\" } }",
        "bigints.json",
    );
    let (_, html) = page(&g);
    assert!(html.contains("<span>1.1 0</span>"), "{html}");
}

#[cfg(unix)]
#[test]
fn a_link_out_of_the_directory_is_not_the_file_of_the_program() {
    let c = corpus();
    let outside = scratch_dir();
    std::fs::write(outside.join("x.json"), bytes_of(&c["files"]["x.json"])).expect("outside");
    let f = program_with(&c, LIST, "x.json");
    let dir = f.parent().expect("dir").to_path_buf();
    std::fs::remove_file(dir.join("x.json")).expect("remove");
    std::os::unix::fs::symlink(outside.join("x.json"), dir.join("x.json")).expect("link");
    assert_eq!(codes(&f), (1, vec!["data-file-missing".to_string()]));
}

#[test]
fn a_program_named_by_a_bare_file_name_finds_its_files() {
    // `cancho-ui check page.cui` in the program's own directory: the directory of `page.cui` is the empty path, which is not one that can be looked at. It
    // was `asset-missing` and `island-source-missing` for every program with a picture, a page file or an island before this was found (docs/data.md section 7)
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    for (dir, file) in [
        ("ecosystem", "ecosystem.cui"),
        ("page", "page.cui"),
        ("islands", "islands.cui"),
        ("structure", "structure.cui"),
    ] {
        let out = Command::new(env!("CARGO_BIN_EXE_cancho-ui"))
            .args(["check", file])
            .current_dir(root.join(dir))
            .output()
            .expect("runs");
        assert!(
            out.status.success(),
            "{dir}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}
