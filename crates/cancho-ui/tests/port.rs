//! What the port of the cancho-www home page needed (docs/www-port/gaps.md), as the compiler's other tools see it: `examples/port` needs no capability and
//! no request, its data file, its island and its files are listed with their hashes, a changed file is a line of `diff` and not a change of authority, a
//! program cut anywhere is refused and never a panic, and a list nested far past the bound is a refusal. What a browser does is `scripts/check-port.js`.
mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

const README: &str = include_str!("../../../spec/diagnostics/README.md");
const EXAMPLE: &str = include_str!("../../../examples/port/port.cui");

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn run(args: &[&str]) -> (i32, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_cancho-ui"))
        .args(args)
        .current_dir(root())
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

fn example() -> String {
    p(&root().join("examples/port/port.cui")).to_string()
}

/// A copy of `examples/port` in a scratch directory, with `edit` applied to one file of it.
fn copy_with(file: &str, edit: impl Fn(&str) -> String) -> PathBuf {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = common::scratch_root().join(format!("cancho-ui-port-{}-{n}", std::process::id()));
    for sub in ["assets", "data"] {
        std::fs::create_dir_all(dir.join(sub)).expect("a directory");
    }
    for f in [
        "port.cui",
        "notes.md",
        "assets/meter.js",
        "assets/meter.css",
        "data/claims.json",
    ] {
        std::fs::copy(root().join("examples/port").join(f), dir.join(f)).expect("copy");
    }
    let path = dir.join(file);
    let text = std::fs::read_to_string(&path).expect("read");
    std::fs::write(&path, edit(&text)).expect("write");
    dir.join("port.cui")
}

#[test]
fn the_example_needs_no_capability_and_no_request_and_names_what_it_reads() {
    let (code, out, err) = run(&["inspect", "effects", &example(), "Port"]);
    assert_eq!(code, 0, "{err}");
    let v: Value = serde_json::from_str(&out).expect("JSON");
    assert_eq!(v["requires"], serde_json::json!([]), "no capability");
    assert_eq!(v["reachable_effects"], serde_json::json!([]), "no effect");
    assert_eq!(v["data"][0]["file"], "data/claims.json");
    assert_eq!(
        v["data"][0]["sha256"],
        "8ef72490d1681fce81b98e73886fc9394034d1574698afa34b0515de53013a75"
    );
    // the island reads state and is still a sealed frame that reaches nothing but the page
    assert_eq!(v["islands"][0]["sealed"], true);
    assert_eq!(
        v["islands"][0]["reaches"],
        "nothing but this page, by messages (props in, events out)"
    );
    assert_eq!(v["links"], serde_json::json!(["github.com"]));
    // the report names the island as the one trust boundary of the program, and the program has no other
    let (_, rep, _) = run(&["report", &example(), "--output", "json"]);
    let v: Value = serde_json::from_str(&rep).expect("JSON");
    let kinds: Vec<&str> = v["trust"]["boundaries"]
        .as_array()
        .expect("boundaries")
        .iter()
        .filter_map(|b| b["kind"].as_str())
        .collect();
    assert_eq!(kinds, vec!["sealed island"], "{rep}");
}

#[test]
fn the_build_carries_the_files_the_links_name_and_hashes_them() {
    let f = copy_with("port.cui", |t| t.to_string());
    let out = f.parent().expect("dir").join("out");
    let (code, o, e) = run(&["build", p(&f), "--page", "-o", p(&out)]);
    assert_eq!(code, 0, "{o}{e}");
    for file in ["notes.md", "data/claims.json"] {
        assert_eq!(
            std::fs::read(out.join(file)).expect("the file is in the build"),
            std::fs::read(f.parent().expect("dir").join(file)).expect("the program's file"),
            "{file}"
        );
    }
    let html = std::fs::read_to_string(out.join("index.html")).expect("index.html");
    assert!(html.contains("href=\"notes.md\"") && html.contains("href=\"data/claims.json\""));
    // the toggle buttons say whether they are pressed, and the script holds nothing of the data
    let js = std::fs::read_to_string(out.join("app.js")).expect("app.js");
    assert!(
        js.contains("setAttribute('aria-pressed', String(!!("),
        "pressed"
    );
    assert!(
        !js.contains("claims.json") && !js.contains("data.items") && !js.contains("data.value")
    );
    // the sealed island gets the props it reads from state, with their types
    assert!(
        js.contains("__cui_islands_scan(root, {  }, { \"Meter\": { \"step\": __state_step, \"note\": __state_note } });"),
        "the island's props that read state are what the script gives the frame at every render"
    );
}

#[test]
fn a_changed_file_is_a_line_of_diff_and_the_authority_is_unchanged() {
    let a = copy_with("port.cui", |t| t.to_string());
    let b = copy_with("notes.md", |t| format!("{t}one more line\n"));
    let (code, human, _) = run(&["diff", p(&a), p(&b)]);
    assert_eq!(code, 5, "{human}");
    assert!(human.contains("~ file notes.md: "), "{human}");
    assert!(human.contains("authority: unchanged"), "{human}");
    // a link that now names another file is a line too, and the files a program names are all in its IR
    let (_, js, _) = run(&["diff", p(&a), p(&b), "--output", "json"]);
    let v: Value = serde_json::from_str(&js).expect("JSON");
    assert_eq!(v["authority"], "unchanged");
    assert_eq!(v["assets"][0]["path"], "notes.md");
}

#[test]
fn a_prop_that_reads_state_changes_no_authority() {
    let literal = copy_with("port.cui", |t| {
        t.replace(
            "Meter { step: step note: note }",
            "Meter { step: 1 note: \"abc\" }",
        )
    });
    let state = copy_with("port.cui", |t| t.to_string());
    let (code, human, _) = run(&["diff", p(&literal), p(&state)]);
    assert!(code == 0 || code == 5, "{human}");
    assert!(human.contains("authority: unchanged"), "{human}");
    let (_, out, _) = run(&["inspect", "effects", p(&literal), "Port"]);
    let (_, out2, _) = run(&["inspect", "effects", p(&state), "Port"]);
    let (a, b): (Value, Value) = (
        serde_json::from_str(&out).expect("JSON"),
        serde_json::from_str(&out2).expect("JSON"),
    );
    assert_eq!(a["requires"], b["requires"]);
    assert_eq!(a["islands"], b["islands"]);
}

#[test]
fn a_program_cut_anywhere_is_refused_and_never_a_panic() {
    let full = EXAMPLE;
    let f = copy_with("port.cui", |t| t.to_string());
    let dir = f.parent().expect("dir").to_path_buf();
    let mut cuts: Vec<usize> = full.char_indices().map(|(i, _)| i).collect();
    cuts.push(full.len());
    // every prefix and every one-character deletion, over the lists, the buttons and the island (a stride keeps the run short)
    for (n, at) in cuts.iter().enumerate().step_by(3) {
        let c = dir.join(format!("cut{n}.cui"));
        std::fs::write(&c, &full[..*at]).expect("cut");
        let (code, _, err) = run(&["check", p(&c)]);
        assert!(code == 0 || code == 1, "prefix {at}: exit {code}: {err}");
        if let Some(next) = cuts.get(n + 1) {
            let d = dir.join(format!("del{n}.cui"));
            std::fs::write(&d, format!("{}{}", &full[..*at], &full[*next..])).expect("cut");
            let (code, _, err) = run(&["check", p(&d)]);
            assert!(
                code == 0 || code == 1,
                "deleting at {at}: exit {code}: {err}"
            );
        }
    }
}

#[test]
fn lists_nested_past_the_bound_are_a_refusal_and_not_a_panic() {
    for depth in [8usize, 31, 33, 200] {
        let mut view = String::from("Text { \"x\" }");
        for _ in 0..depth {
            view = format!("List {{ Item {{ {view} }} }}");
        }
        let text = format!("component C {{\n    view {{ Column {{ {view} }} }}\n}}\n");
        let f = copy_with("port.cui", |_| text.clone());
        let (code, _, err) = run(&["check", p(&f)]);
        assert!(code == 0 || code == 1, "depth {depth}: exit {code}: {err}");
        if depth >= 33 {
            assert_eq!(code, 1, "depth {depth} is past the bound of nesting");
        }
    }
}

#[test]
fn the_tags_this_pr_added_are_documented() {
    for tag in [
        "list-child-invalid",
        "item-outside-list",
        "list-empty",
        "item-empty",
        "list-ordered-invalid",
        "link-file-invalid",
        "data-code-unbalanced",
    ] {
        assert!(
            README.contains(&format!("`{tag}` | `Component`")),
            "`{tag}` has no row of its own in the README"
        );
    }
    // the tag that the sealed island's state props removed is gone from the README, and nothing else mentions it as a refusal
    assert!(
        !README.contains("`island-prop-dynamic`"),
        "island-prop-dynamic is not a tag any more"
    );
}
