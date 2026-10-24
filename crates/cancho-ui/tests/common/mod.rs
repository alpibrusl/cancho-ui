//! Where the integration tests make their scratch files. Every test binary used to make directories straight in the system temporary directory and
//! leave them there: one machine held 125,660 of them, enough for Chromium to fail with `ERR_INSUFFICIENT_RESOURCES`. Now they go under one
//! directory per process, `<tmp>/cancho-ui-tests/<pid>/`, and the first use in a process removes the directories of processes that have ended (a
//! test binary cannot run code after its last test, so the sweep is at the start of the next run). Directories the old way made are removed too
//! when they are over an hour old.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, SystemTime};

fn alive(pid: u32) -> bool {
    if cfg!(target_os = "linux") {
        Path::new(&format!("/proc/{pid}")).exists()
    } else {
        // no cheap test elsewhere: keep a directory until it is a day old
        true
    }
}

fn older_than(path: &Path, age: Duration) -> bool {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| SystemTime::now().duration_since(t).ok())
        .is_some_and(|d| d > age)
}

fn sweep(tmp: &Path, base: &Path) {
    let me = std::process::id();
    for entry in std::fs::read_dir(base).into_iter().flatten().flatten() {
        let path = entry.path();
        let ended = entry
            .file_name()
            .to_str()
            .and_then(|n| n.parse::<u32>().ok())
            .is_some_and(|pid| pid != me && !alive(pid));
        if ended || older_than(&path, Duration::from_secs(24 * 3600)) {
            let _ = std::fs::remove_dir_all(&path);
        }
    }
    // what the tests made before this module: directly in the temporary directory, named `cancho-ui-*` or `cui-*`
    for entry in std::fs::read_dir(tmp).into_iter().flatten().flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if (name.starts_with("cancho-ui-") || name.starts_with("cui-"))
            && name != "cancho-ui-tests"
            && older_than(&entry.path(), Duration::from_secs(3600))
        {
            let path = entry.path();
            let _ = if path.is_dir() {
                std::fs::remove_dir_all(&path)
            } else {
                std::fs::remove_file(&path)
            };
        }
    }
}

/// The directory this process makes its scratch files in; made on first use.
pub fn scratch_root() -> PathBuf {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();
    ROOT.get_or_init(|| {
        let tmp = std::env::temp_dir();
        let base = tmp.join("cancho-ui-tests");
        let _ = std::fs::create_dir_all(&base);
        sweep(&tmp, &base);
        let mine = base.join(std::process::id().to_string());
        std::fs::create_dir_all(&mine).expect("a scratch directory");
        mine
    })
    .clone()
}
