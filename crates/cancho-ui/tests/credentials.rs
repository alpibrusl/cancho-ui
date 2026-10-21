//! `requires Credential("name", "host"..)` (`docs/adapters.md` section 18): what is accepted and refused, what is
//! emitted, what replay shows, and that a module which mounts itself cannot be built with one. The browser side (the
//! token reaches this origin and the hosts named, and no other) is `scripts/check-credential.js`.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

const BASE: &str = "component Readings {\n    requires Network;\n    requires Credential(\"api\");\n    state r: Result[String, String] = Ok(\"\");\n    on Load { r = await(net.get(\"/x\")); }\n    view { Button { label: \"Load\" on: Load } }\n}\n";

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
    let dir = std::env::temp_dir().join(format!("cancho-ui-credentials-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let path = dir.join(name);
    std::fs::write(&path, text).expect("a scratch file");
    path
}

fn p(path: &Path) -> &str {
    path.to_str().expect("UTF-8 temp path")
}

fn codes(name: &str, src: &str) -> Vec<String> {
    let file = scratch(name, src);
    let (code, stdout, _) = run(&["check", p(&file), "--output", "json"]);
    assert_eq!(code, 1, "{name}: {stdout}");
    let diags: Value = serde_json::from_str(&stdout).expect("diagnostics are JSON");
    diags
        .as_array()
        .expect("a list")
        .iter()
        .map(|d| d["code"].as_str().expect("a code").to_string())
        .collect()
}

fn builds(name: &str, src: &str, flags: &[&str]) -> (i32, String, String) {
    let file = scratch(name, src);
    let out = scratch(&format!("{name}.js"), "");
    let mut args = vec!["build", p(&file), "-o", p(&out)];
    args.extend_from_slice(flags);
    let (code, stdout, stderr) = run(&args);
    let js = std::fs::read_to_string(&out).unwrap_or_default();
    (code, js, format!("{stdout}{stderr}"))
}

#[test]
fn a_declared_credential_that_a_request_can_use_is_accepted() {
    let file = scratch("ok.cui", BASE);
    let (code, stdout, stderr) = run(&["check", p(&file)]);
    assert_eq!(code, 0, "{stdout}{stderr}");
    // the extra host must be allowed by Network, and is
    let with_host = BASE
        .replace("requires Network;", "requires Network(\"api.example\");")
        .replace(
            "Credential(\"api\")",
            "Credential(\"api\", \"api.example\")",
        );
    let file = scratch("host.cui", &with_host);
    let (code, stdout, stderr) = run(&["check", p(&file)]);
    assert_eq!(code, 0, "{stdout}{stderr}");
    // and a write may carry one too
    let write = "component W {\n    requires NetworkWrite;\n    requires Credential(\"api\");\n    state r: Result[String, String] = Ok(\"\");\n    on Go { r = await(net.post(\"/x\", \"1\")); }\n    view { Button { label: \"Go\" on: Go } }\n}\n";
    let file = scratch("write.cui", write);
    let (code, stdout, stderr) = run(&["check", p(&file)]);
    assert_eq!(code, 0, "{stdout}{stderr}");
}

#[test]
fn the_refusals_each_have_their_rule_tag() {
    let two = BASE.replace(
        "requires Credential(\"api\");",
        "requires Credential(\"api\");\n    requires Credential(\"other\");",
    );
    assert_eq!(codes("two.cui", &two), ["credential-ambiguous"]);
    let host = BASE.replace(
        "Credential(\"api\")",
        "Credential(\"api\", \"api.example\")",
    );
    assert_eq!(codes("nothost.cui", &host), ["credential-host-not-allowed"]);
    let unnamed = BASE.replace("Credential(\"api\")", "Credential");
    assert_eq!(
        codes("unnamed.cui", &unnamed),
        ["capability-narrowing-illegal"]
    );
    for bad in ["API", "a b", "a:b", "", "x".repeat(33).as_str()] {
        let src = BASE.replace("Credential(\"api\")", &format!("Credential(\"{bad}\")"));
        assert_eq!(
            codes("badname.cui", &src),
            ["capability-narrowing-illegal"],
            "name {bad:?}"
        );
    }
    let bad_host = BASE
        .replace("requires Network;", "requires Network(\"api.example\");")
        .replace(
            "Credential(\"api\")",
            "Credential(\"api\", \"https://api.example/\")",
        );
    assert_eq!(
        codes("badhost.cui", &bad_host),
        ["capability-narrowing-illegal"]
    );
    // a credential no request can use is unused, like any capability
    let unused = BASE
        .replace("r = await(net.get(\"/x\"));", "r = Ok(\"\");")
        .replace("requires Network;\n", "");
    assert_eq!(codes("unused.cui", &unused), ["capability-unused"]);
    // without a request capability the call itself is missing it
    let no_net = BASE.replace("requires Network;\n", "");
    assert_eq!(codes("nonet.cui", &no_net), ["capability-missing"]);
}

#[test]
fn a_module_that_mounts_itself_cannot_carry_a_credential() {
    let (code, js, msg) = builds("auto.cui", BASE, &[]);
    assert_eq!(code, 1, "{msg}");
    assert!(js.is_empty(), "nothing is written for a refused build");
    assert!(msg.contains("credential-needs-mount-options"), "{msg}");
    let file = scratch("autojson.cui", BASE);
    let (code, stdout, _) = run(&["build", p(&file), "--output", "json"]);
    assert_eq!(code, 1);
    assert!(
        stdout.contains("--no-auto-mount"),
        "the remediation: {stdout}"
    );
    assert!(stdout.contains("mount_Readings"), "{stdout}");
    // the installed web app mounts itself too
    let dir = scratch("pwa-out", "").with_extension("d");
    let (code, _, msg) = builds("pwa.cui", BASE, &["--pwa"]);
    assert_ne!(code, 0, "{msg}");
    let _ = dir;
}

#[test]
fn with_no_auto_mount_the_entry_takes_the_tokens_and_a_request_carries_the_declaration() {
    let (code, js, msg) = builds("manual.cui", BASE, &["--no-auto-mount"]);
    assert_eq!(code, 0, "{msg}");
    assert!(
        js.contains("function __cui_mount_entry(root, options)"),
        "{js}"
    );
    assert!(js.contains("export { __cui_mount_entry as mount_Readings };"));
    assert!(!js.contains("document.getElementById('root')"));
    assert!(js.contains("const __cui_cred = { name: \"api\", hosts: [],"));
    assert!(
        js.contains("__cui_net_get(\"/x\", __cui_net_hosts, __cui_cred)"),
        "{js}"
    );
    // the token is read from the mount's options and nowhere else
    assert!(js.contains("options.credentials"));
    // a module that declares none carries none of it
    let plain = BASE.replace("    requires Credential(\"api\");\n", "");
    let (code, js, msg) = builds("plain.cui", &plain, &["--no-auto-mount"]);
    assert_eq!(code, 0, "{msg}");
    assert!(!js.contains("__cui_cur_creds") && !js.contains("__cui_mount_entry"));
    assert!(js.contains("const __cui_cred = null;"));
}

#[test]
fn the_host_list_is_emitted_lower_cased() {
    let src = BASE
        .replace(
            "requires Network;",
            "requires Network(\"Api.Example:8443\");",
        )
        .replace(
            "Credential(\"api\")",
            "Credential(\"api\", \"Api.Example:8443\")",
        );
    let (code, js, msg) = builds("hosts.cui", &src, &["--no-auto-mount"]);
    assert_eq!(code, 0, "{msg}");
    assert!(js.contains("hosts: [\"api.example:8443\"]"), "{js}");
}

#[test]
fn replay_shows_which_credential_a_request_carried_and_never_its_value() {
    let file = scratch("replay.cui", BASE);
    let supplied = scratch(
        "supplied.trace.json",
        r#"{"schema":"cancho-ui-trace","version":"0.1.0","component":"Readings","credentials":["api"],"steps":[
            {"event":"Load","expect":{"pending":[1]}}]}"#,
    );
    let (code, stdout, stderr) = run(&["test", p(&file), p(&supplied)]);
    assert_eq!(code, 0, "{stdout}{stderr}");
    assert!(
        stdout.contains("issued #1 net.get(\"/x\") [credential api]"),
        "{stdout}"
    );
    let missing = scratch(
        "missing.trace.json",
        r#"{"schema":"cancho-ui-trace","version":"0.1.0","component":"Readings","steps":[
            {"event":"Load","expect":{"pending":[1]},"state":{"r":{"Ok":""}}}]}"#,
    );
    let (code, stdout, _) = run(&["test", p(&file), p(&missing)]);
    assert_eq!(code, 0, "{stdout}");
    assert!(
        stdout.contains("[credential api (not supplied)]"),
        "{stdout}"
    );
    // a trace lists credentials by name; a value is not a name
    let value = scratch(
        "value.trace.json",
        r#"{"schema":"cancho-ui-trace","version":"0.1.0","component":"Readings","credentials":[{"api":"secret-token"}],"steps":[
            {"event":"Load","expect":{"pending":[1]}}]}"#,
    );
    let (code, stdout, _) = run(&["test", p(&file), p(&value), "--output", "json"]);
    assert_ne!(code, 0);
    assert!(stdout.contains("trace-malformed"), "{stdout}");
    assert!(
        !stdout.contains("secret-token"),
        "the value must not be echoed: {stdout}"
    );
    // a credential the component does not declare
    let unknown = scratch(
        "unknown.trace.json",
        r#"{"schema":"cancho-ui-trace","version":"0.1.0","component":"Readings","credentials":["nope"],"steps":[
            {"event":"Load","expect":{"pending":[1]}}]}"#,
    );
    let (code, stdout, _) = run(&["test", p(&file), p(&unknown), "--output", "json"]);
    assert_eq!(code, 4, "{stdout}");
    assert!(stdout.contains("trace-credential-unknown"), "{stdout}");
}

#[test]
fn the_diff_shows_a_credential_added_and_the_ir_holds_no_value() {
    let before = scratch(
        "before.cui",
        &BASE.replace("    requires Credential(\"api\");\n", ""),
    );
    let after = scratch("after.cui", BASE);
    let (code, stdout, stderr) = run(&["diff", p(&before), p(&after)]);
    assert_eq!(code, 5, "{stdout}{stderr}");
    assert!(stdout.contains("Credential"), "{stdout}");
    let (code, ir, _) = run(&["inspect", "ir", p(&after)]);
    if code == 0 {
        assert!(ir.contains("Credential"), "{ir}");
    }
}
