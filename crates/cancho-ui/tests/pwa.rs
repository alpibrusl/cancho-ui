//! `cancho-ui build --pwa` (`docs/mobile-backend.md`): what it writes, that it refuses like `build`, and that two
//! builds are the same bytes. The browser-side checks (installable, offline, touch targets) are
//! `scripts/check-pwa.js`, outside the gate because they need a browser.

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

/// A fresh, empty scratch path for one test (not created).
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cancho-ui-pwa-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn pwa(source: &str, dir: &Path) -> (i32, String, String) {
    run(&[
        "build",
        source,
        "--pwa",
        "-o",
        dir.to_str().expect("UTF-8 temp path"),
    ])
}

fn files_of(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .expect("the directory exists")
        .map(|e| {
            e.expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    names
}

const FILES: [&str; 6] = [
    "app.js",
    "icon-192.png",
    "icon-512.png",
    "index.html",
    "manifest.webmanifest",
    "sw.js",
];

#[test]
fn it_writes_exactly_the_files_of_the_design_and_app_js_is_the_plain_build() {
    let dir = scratch("files");
    let (code, _, stderr) = pwa("examples/counter/counter.cui", &dir);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(files_of(&dir), FILES);
    let plain = scratch("plain.js");
    let (code, ..) = run(&[
        "build",
        "examples/counter/counter.cui",
        "-o",
        plain.to_str().expect("UTF-8 temp path"),
    ]);
    assert_eq!(code, 0);
    assert_eq!(
        std::fs::read(dir.join("app.js")).expect("app.js"),
        std::fs::read(&plain).expect("the plain build"),
        "app.js is byte-identical to `build` without --pwa"
    );
}

#[test]
fn the_page_and_the_manifest_say_what_a_phone_needs() {
    let dir = scratch("page");
    assert_eq!(pwa("examples/counter/counter.cui", &dir).0, 0);
    let html = std::fs::read_to_string(dir.join("index.html")).expect("index.html");
    for needle in [
        "name=\"viewport\" content=\"width=device-width, initial-scale=1, viewport-fit=cover\"",
        "rel=\"manifest\" href=\"./manifest.webmanifest\"",
        "<script type=\"module\" src=\"./app.js\">",
        "navigator.serviceWorker.register('./sw.js')",
        "min-height: 44px",
        "<title>counter</title>",
    ] {
        assert!(
            html.contains(needle),
            "index.html lacks {needle:?}:\n{html}"
        );
    }
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.join("manifest.webmanifest")).expect("manifest"))
            .expect("the manifest is JSON");
    assert_eq!(manifest["name"], "counter");
    assert_eq!(manifest["short_name"], "counter");
    assert_eq!(manifest["start_url"], "./");
    assert_eq!(manifest["scope"], "./");
    assert_eq!(manifest["display"], "standalone");
    let sizes: Vec<(&str, &str, &str)> = manifest["icons"]
        .as_array()
        .expect("icons")
        .iter()
        .map(|i| {
            (
                i["src"].as_str().expect("src"),
                i["sizes"].as_str().expect("sizes"),
                i["purpose"].as_str().expect("purpose"),
            )
        })
        .collect();
    // #67: each size is declared for "any" and, separately, as "maskable" (a combined "any maskable" is discouraged).
    assert_eq!(
        sizes,
        [
            ("icon-192.png", "192x192", "any"),
            ("icon-512.png", "512x512", "any"),
            ("icon-192.png", "192x192", "maskable"),
            ("icon-512.png", "512x512", "maskable"),
        ]
    );
}

/// The CRC-32 of the PNG specification, by table.
fn crc32(bytes: &[u8]) -> u32 {
    let table: Vec<u32> = (0..256u32)
        .map(|n| {
            (0..8).fold(n, |c, _| {
                if c & 1 == 1 {
                    0xedb8_8320 ^ (c >> 1)
                } else {
                    c >> 1
                }
            })
        })
        .collect();
    !bytes.iter().fold(0xffff_ffffu32, |c, b| {
        table[((c ^ u32::from(*b)) & 0xff) as usize] ^ (c >> 8)
    })
}

#[test]
fn the_icons_are_valid_pngs_of_the_sizes_the_manifest_names() {
    let dir = scratch("icons");
    assert_eq!(pwa("examples/counter/counter.cui", &dir).0, 0);
    for (file, size) in [("icon-192.png", 192u32), ("icon-512.png", 512)] {
        let png = std::fs::read(dir.join(file)).expect("an icon");
        assert_eq!(
            &png[..8],
            &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a],
            "{file}"
        );
        let mut at = 8;
        let mut kinds = Vec::new();
        let mut zlib = Vec::new();
        while at < png.len() {
            let len = u32::from_be_bytes(png[at..at + 4].try_into().unwrap()) as usize;
            let body = &png[at + 4..at + 8 + len];
            let crc = u32::from_be_bytes(png[at + 8 + len..at + 12 + len].try_into().unwrap());
            assert_eq!(crc32(body), crc, "{file}: the CRC of {:?}", &body[..4]);
            kinds.push(String::from_utf8_lossy(&body[..4]).into_owned());
            match &body[..4] {
                b"IHDR" => {
                    assert_eq!(u32::from_be_bytes(body[4..8].try_into().unwrap()), size);
                    assert_eq!(u32::from_be_bytes(body[8..12].try_into().unwrap()), size);
                    assert_eq!(&body[12..17], &[1, 3, 0, 0, 0], "{file}: 1 bit, palette");
                }
                b"IDAT" => zlib.extend_from_slice(&body[4..]),
                _ => {}
            }
            at += 12 + len;
        }
        assert_eq!(kinds, ["IHDR", "PLTE", "IDAT", "IEND"], "{file}");
        // The stored-deflate stream carries exactly size rows of (1 filter byte + ceil(size/8) bytes).
        let raw = (1 + size.div_ceil(8)) * size;
        let mut at = 2;
        let mut got = 0u32;
        loop {
            let last = zlib[at] == 1;
            let len = u16::from_le_bytes([zlib[at + 1], zlib[at + 2]]);
            assert_eq!(
                u16::from_le_bytes([zlib[at + 3], zlib[at + 4]]),
                !len,
                "{file}: NLEN"
            );
            got += u32::from(len);
            at += 5 + len as usize;
            if last {
                break;
            }
        }
        assert_eq!(got, raw, "{file}: the pixel data");
        assert_eq!(at + 4, zlib.len(), "{file}: only the Adler-32 follows");
    }
}

#[test]
fn the_worker_precaches_every_file_it_serves_and_names_a_version() {
    let dir = scratch("worker");
    assert_eq!(pwa("examples/counter/counter.cui", &dir).0, 0);
    let sw = std::fs::read_to_string(dir.join("sw.js")).expect("sw.js");
    for file in FILES.iter().filter(|f| **f != "sw.js") {
        assert!(
            sw.contains(&format!("\"./{file}\"")),
            "sw.js does not precache {file}:\n{sw}"
        );
    }
    assert!(sw.contains("\"./\""), "the start URL is precached");
    let version = sw
        .lines()
        .find_map(|l| l.strip_prefix("const CACHE = \"cancho-ui-"))
        .expect("a cache name")
        .trim_end_matches("\";");
    assert_eq!(version.len(), 16, "an FNV-1a 64 hash in hex: {version}");
    assert!(version.chars().all(|c| c.is_ascii_hexdigit()));
    assert!(
        !sw.contains("skipWaiting"),
        "a new version waits for the old tabs to close"
    );
}

#[test]
fn two_builds_are_the_same_bytes_and_another_program_is_another_version() {
    let (a, b, other) = (scratch("det-a"), scratch("det-b"), scratch("det-other"));
    assert_eq!(pwa("examples/counter/counter.cui", &a).0, 0);
    assert_eq!(pwa("examples/counter/counter.cui", &b).0, 0);
    for file in FILES {
        assert_eq!(
            std::fs::read(a.join(file)).unwrap(),
            std::fs::read(b.join(file)).unwrap(),
            "{file} differs between two builds"
        );
    }
    assert_eq!(pwa("examples/clock/clock.cui", &other).0, 0);
    assert_ne!(
        std::fs::read(a.join("sw.js")).unwrap(),
        std::fs::read(other.join("sw.js")).unwrap(),
        "a different program is a different cache version"
    );
}

#[test]
fn a_name_is_taken_from_the_flag_or_the_file() {
    let dir = scratch("name");
    let (code, _, stderr) = run(&[
        "build",
        "examples/counter/counter.cui",
        "--pwa",
        "-o",
        dir.to_str().unwrap(),
        "--name",
        "My Counter",
    ]);
    assert_eq!(code, 0, "{stderr}");
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.join("manifest.webmanifest")).unwrap()).unwrap();
    assert_eq!(manifest["name"], "My Counter");
    assert!(
        std::fs::read_to_string(dir.join("index.html"))
            .unwrap()
            .contains("<title>My Counter</title>")
    );
}

#[test]
fn a_refused_program_writes_nothing_and_exits_one_like_build() {
    let dir = scratch("refused");
    let broken = scratch("broken.cui");
    let clock = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/clock/clock.cui"),
    )
    .expect("the clock example");
    assert!(
        clock.contains("requires Clock;"),
        "the example declares its capability"
    );
    std::fs::write(&broken, clock.replace("requires Clock;", "")).expect("a scratch source");
    let (code, _, stderr) = pwa(broken.to_str().unwrap(), &dir);
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.contains("capability-missing"), "{stderr}");
    assert!(!dir.exists(), "nothing was created: {:?}", dir);
}

#[test]
fn the_command_line_is_checked() {
    let dir = scratch("usage");
    let d = dir.to_str().unwrap();
    let (code, _, stderr) = run(&["build", "examples/counter/counter.cui", "--pwa"]);
    assert_eq!(code, 2, "{stderr}");
    assert!(stderr.contains("require `-o <dir>`"), "{stderr}");
    let (code, ..) = run(&[
        "build",
        "examples/counter/counter.cui",
        "examples/clock/clock.cui",
        "--pwa",
        "-o",
        d,
    ]);
    assert_eq!(code, 2);
    let (code, _, stderr) = run(&["build", "examples/counter/counter.cui", "--name", "x"]);
    assert_eq!(code, 2, "{stderr}");
    assert!(!dir.exists());
}
