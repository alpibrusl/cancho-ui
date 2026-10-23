//! The emitted module (`docs/mobile-backend.md` section 9): `--no-auto-mount` changes exactly the mount lines, and every
//! byte of the module is printable ASCII (or a newline or tab), whatever the program or its path contains.

use std::path::{Path, PathBuf};
use std::process::Command;

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

fn scratch_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "cancho-ui-build-output-{}-{name}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir
}

fn p(path: &Path) -> &str {
    path.to_str().expect("UTF-8 temp path")
}

/// The first byte that is neither a newline, a tab nor printable ASCII, with its offset.
fn first_non_ascii(bytes: &[u8]) -> Option<(usize, u8)> {
    bytes
        .iter()
        .copied()
        .enumerate()
        .find(|&(_, b)| !(b == b'\n' || b == b'\t' || (32..=126).contains(&b)))
}

fn examples() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let mut found = Vec::new();
    for dir in std::fs::read_dir(&root).expect("examples/") {
        let dir = dir.expect("an entry").path();
        if !dir.is_dir() {
            continue;
        }
        for f in std::fs::read_dir(&dir).expect("an example directory") {
            let f = f.expect("an entry").path();
            if f.extension().is_some_and(|e| e == "cui") {
                found.push(f);
            }
        }
    }
    found.sort();
    found
}

const HOSTILE: &str = "component Cafe {\n    state s: String = \"caf\u{e9} \u{1F600} \u{2014}\";\n    on Go { s = \"\u{e9}\"; }\n    view { Column { Text { \"na\u{ef}ve {s} \u{2014} \u{1F600}\" } Button { label: \"\u{dc}ber\" on: Go } } }\n}\n";

#[test]
fn every_example_builds_to_ascii() {
    let dir = scratch_dir("examples");
    let all = examples();
    assert!(all.len() >= 14, "{} examples", all.len());
    for (i, f) in all.iter().enumerate() {
        let out = dir.join(format!("{i}.js"));
        let (code, _, stderr) = run(&["build", p(f), "-o", p(&out)]);
        assert_eq!(code, 0, "{}: {stderr}", f.display());
        let bytes = std::fs::read(&out).expect("the built module");
        assert_eq!(first_non_ascii(&bytes), None, "{}", f.display());
    }
}

#[test]
fn a_program_with_non_ascii_text_in_a_non_ascii_directory_builds_to_ascii() {
    let dir = scratch_dir("hostile").join("d\u{ef}r");
    std::fs::create_dir_all(&dir).expect("a non-ASCII directory");
    let src = dir.join("caf\u{e9}.cui");
    std::fs::write(&src, HOSTILE).expect("the source");
    let out = scratch_dir("hostile-out").join("out.js");
    let (code, _, stderr) = run(&["build", p(&src), "-o", p(&out)]);
    assert_eq!(code, 0, "{stderr}");
    let js = std::fs::read_to_string(&out).expect("the built module");
    assert_eq!(first_non_ascii(js.as_bytes()), None);
    // the strings survive as escapes, one per UTF-16 unit: an astral character is a surrogate pair
    assert!(js.contains("caf\\u00e9 \\ud83d\\ude00 \\u2014"), "{js}");
    assert!(js.contains("\\u00dcber"), "{js}");
    // the Source comment shows the path with each non-ASCII character as `?`
    let source = js
        .lines()
        .find(|l| l.starts_with("// Source: "))
        .expect("a Source comment");
    assert!(
        source.contains("d?r") && source.contains("caf?.cui"),
        "{source}"
    );
}

#[test]
fn no_auto_mount_changes_exactly_the_mount_lines() {
    let dir = scratch_dir("mount");
    let src = dir.join("counter.cui");
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/counter/counter.cui"),
        &src,
    )
    .expect("copy the counter example");
    let (a, b) = (dir.join("default.js"), dir.join("embed.js"));
    assert_eq!(run(&["build", p(&src), "-o", p(&a)]).0, 0);
    assert_eq!(
        run(&["build", p(&src), "--no-auto-mount", "-o", p(&b)]).0,
        0
    );
    let default = std::fs::read_to_string(&a).expect("default module");
    let embed = std::fs::read_to_string(&b).expect("embeddable module");
    // the default is as it was: it mounts into #root and exports
    assert!(
        default.contains("document.getElementById('root')"),
        "{default}"
    );
    assert!(default.contains("export { mount_Counter };"), "{default}");
    // the flag omits the side effect and keeps the export
    assert!(!embed.contains("getElementById"), "{embed}");
    assert!(embed.contains("export { mount_Counter };"), "{embed}");
    // and nothing else: remove the two mount lines from the default and the rest is identical
    let stripped: String = default
        .lines()
        .filter(|l| !l.starts_with("const root = ") && !l.starts_with("if (root) { mount_"))
        .map(|l| format!("{l}\n"))
        .collect();
    assert_eq!(stripped, embed);
}

#[test]
fn no_auto_mount_with_pwa_is_a_usage_error_and_writes_nothing() {
    let dir = scratch_dir("pwa");
    let src = dir.join("counter.cui");
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/counter/counter.cui"),
        &src,
    )
    .expect("copy the counter example");
    let out = dir.join("app");
    let (code, _, stderr) = run(&["build", p(&src), "--pwa", "--no-auto-mount", "-o", p(&out)]);
    assert_eq!(code, 2, "{stderr}");
    assert!(stderr.contains("--no-auto-mount"), "{stderr}");
    assert!(!out.exists(), "nothing was written");
}

#[test]
fn a_non_ascii_identifier_is_refused_so_the_module_never_carries_one() {
    let dir = scratch_dir("ident");
    for (name, src) in [
        (
            "component.cui",
            "component Caf\u{e9} {\n    state n: Int = 0;\n    view { Text { \"{n}\" } }\n}\n",
        ),
        (
            "state.cui",
            "component C {\n    state caf\u{e9}: Int = 0;\n    view { Text { \"{caf\u{e9}}\" } }\n}\n",
        ),
    ] {
        let file = dir.join(name);
        std::fs::write(&file, src).expect("the source");
        let out = dir.join(format!("{name}.js"));
        let (code, _, stderr) = run(&["build", p(&file), "-o", p(&out)]);
        assert_eq!(code, 1, "{name}: {stderr}");
        assert!(stderr.contains("lex-unexpected-char"), "{name}: {stderr}");
        assert!(!out.exists(), "{name}: nothing was written");
    }
}
