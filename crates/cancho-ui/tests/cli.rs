//! The documented command lines must work as documented.

use std::path::Path;
use std::process::Command;

fn run(args: &[&str]) -> (i32, String) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new(env!("CARGO_BIN_EXE_cancho-ui"))
        .args(args)
        .current_dir(root)
        .output()
        .expect("the cancho-ui binary runs");
    (
        out.status.code().expect("exited, not killed by a signal"),
        String::from_utf8_lossy(&out.stdout).into_owned(),
    )
}

#[test]
fn inspect_effects_takes_the_output_flag_after_the_name() {
    let (code, stdout) = run(&[
        "inspect",
        "effects",
        "examples/clock/clock.cui",
        "Clock",
        "--output",
        "json",
    ]);
    assert_eq!(code, 0, "stdout: {stdout}");
    assert!(stdout.contains("\"capability\": \"Clock\""), "{stdout}");
}
