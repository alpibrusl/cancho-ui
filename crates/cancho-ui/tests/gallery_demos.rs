//! The landing-page gallery demos as the repository holds them (docs/mobile-backend.md section 5.2, alpibrusl/cancho-ui#65).
//!
//! GitHub Pages serves what is committed. Every `docs/demo/<id>/app.js` was once git-ignored (`*.js` in `.gitignore`), so the
//! site had each demo's `index.html` and no script, and every iframe of the gallery was blank on a phone, while the checks
//! that read the working tree, where the file existed, passed. These tests are in the gate because CI checks out the
//! repository and nothing else: a demo file that is not committed fails here.
mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn manifest() -> Value {
    let text = std::fs::read_to_string(root().join("docs/gallery.json")).expect("the manifest");
    serde_json::from_str(&text).expect("the manifest is JSON")
}

fn build_page(source: &str, name: &str, out: &Path) {
    let r = Command::new(env!("CARGO_BIN_EXE_cancho-ui"))
        .args(["build", source, "--page", "-o"])
        .arg(out)
        .args(["--name", name])
        .current_dir(root())
        .output()
        .expect("the cancho-ui binary runs");
    assert!(
        r.status.success(),
        "{source}: {}",
        String::from_utf8_lossy(&r.stderr)
    );
}

fn files(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    fn walk(base: &Path, dir: &Path, out: &mut Vec<String>) {
        for e in std::fs::read_dir(dir).expect("a directory") {
            let e = e.expect("an entry").path();
            if e.is_dir() {
                walk(base, &e, out);
            } else {
                out.push(
                    e.strip_prefix(base)
                        .expect("under base")
                        .to_string_lossy()
                        .into_owned(),
                );
            }
        }
    }
    walk(dir, dir, &mut out);
    out.sort();
    out
}

#[test]
fn every_demo_is_what_the_compiler_writes_as_a_page() {
    let m = manifest();
    let tiles = m["supported"].as_array().expect("supported tiles");
    assert!(tiles.len() >= 7, "the gallery shrank");
    for t in tiles {
        let id = t["id"].as_str().expect("id");
        let tmp =
            common::scratch_root().join(format!("cancho-ui-gallery-{}-{id}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        build_page(
            t["source"].as_str().expect("source"),
            t["title"].as_str().expect("title"),
            &tmp,
        );
        let have = root().join("docs/demo").join(id);
        assert!(have.is_dir(), "docs/demo/{id}/ is missing");
        for f in files(&tmp) {
            let committed = std::fs::read(have.join(&f)).unwrap_or_else(|_| {
                panic!("docs/demo/{id}/{f} is missing (rebuild: node scripts/build-gallery.js)")
            });
            let fresh = std::fs::read(tmp.join(&f)).expect("a fresh file");
            assert!(
                committed == fresh,
                "docs/demo/{id}/{f} is not what `cancho-ui build --page` writes (rebuild: node scripts/build-gallery.js)"
            );
        }
        let _ = std::fs::remove_dir_all(&tmp);
    }
}

#[test]
fn no_demo_is_an_app() {
    // A demo is a page that is embedded and opened: no manifest to install from, no worker to cache it.
    let m = manifest();
    for t in m["supported"].as_array().expect("supported tiles") {
        let id = t["id"].as_str().expect("id");
        let dir = root().join("docs/demo").join(id);
        let mut names = files(&dir);
        names.retain(|f| !f.starts_with("api/"));
        // what a tile says it copies beside the page (the stand-in tiles of the map) is data, not an app
        if let Some(copy) = t["copy"].as_object() {
            names.retain(|f| !copy.keys().any(|k| f.starts_with(&format!("{k}/"))));
        }
        assert_eq!(names, ["app.js", "index.html"], "docs/demo/{id}/");
        let html = std::fs::read_to_string(dir.join("index.html")).expect("the page");
        for banned in [
            "manifest",
            "serviceWorker",
            "theme-color",
            "apple-touch-icon",
        ] {
            assert!(
                !html.contains(banned),
                "docs/demo/{id}/index.html mentions {banned}"
            );
        }
    }
}

#[test]
fn no_demo_file_is_git_ignored() {
    // The bug itself: a file that exists on the disk and is ignored by git is not published.
    let m = manifest();
    for t in m["supported"].as_array().expect("supported tiles") {
        let id = t["id"].as_str().expect("id");
        let dir = root().join("docs/demo").join(id);
        for f in files(&dir) {
            let rel = format!("docs/demo/{id}/{f}");
            let r = Command::new("git")
                .args(["check-ignore", "-q", "--no-index", &rel])
                .current_dir(root())
                .status();
            match r {
                // 1: not ignored. 128 or a missing git: not a git checkout, nothing to check.
                Ok(s) if s.code() == Some(1) => {}
                Ok(s) if s.code() == Some(0) => {
                    panic!("{rel} is git-ignored, so it is not published")
                }
                _ => {}
            }
        }
    }
}

#[test]
fn the_landing_page_links_every_demo_it_embeds() {
    let m = manifest();
    let html = std::fs::read_to_string(root().join("docs/index.html")).expect("the landing page");
    for t in m["supported"].as_array().expect("supported tiles") {
        let id = t["id"].as_str().expect("id");
        assert!(
            html.contains(&format!("src=\"demo/{id}/\" loading=\"lazy\"")),
            "the iframe of {id}"
        );
        assert!(
            html.contains(&format!("href=\"demo/{id}/\"")),
            "the link of {id}"
        );
    }
}
