//! `cancho-ui` — the compiler CLI.
//!
//! Commands (`check` and `build` first; `diff` is Phase 3):
//!
//! - `cancho-ui check <file.cui>...`  — parse, type-check, lower.
//!   Exits 0 on success, 1 on refusal (with a located diagnostic),
//!   2 on command-line error.
//!
//! - `cancho-ui build <file.cui>...`  — same pipeline, then emit
//!   JavaScript to stdout (or to `-o <path>` if given).
//!
//! - `cancho-ui diff <base> <candidate>` — semantic diff of two
//!   programs' typed IRs (docs/semantic-diff.md).
//!
//! - `cancho-ui test <file.cui> <trace.json>` — deterministic replay of an
//!   event trace over the typed IR (docs/replay.md).
//!
//! Exit codes match Cancho Core's:
//!   0 success, 1 refused, 2 command line wrong, 3 environment;
//!   `diff` adds 5 = the programs differ (1 stays "refused"); `test`
//!   adds 4 = a test failed.
//!
//! The CLI is hand-rolled (no clap) for the same reason the cancho
//! binary is: we want stable, agent-facing command names without a
//! generator's defaults. See `docs/agent-cli.md` in Cancho Core for
//! the lessons; in particular, single-dash flags render with their
//! literal dash in `cancho-ui skill` output (Phase 2).

mod assets;
mod diff_cmd;
mod islands;
mod motion;
mod page_files;
mod pipeline;
mod pwa;
mod test_cmd;
mod theme;

use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "\
cancho-ui — Cancho UI compiler CLI

USAGE:
    cancho-ui check <file.cui>... [--output json]
    cancho-ui build <file.cui>... [-o <path>] [--target web] [--no-auto-mount] [--output json]
    cancho-ui build <file.cui> --pwa -o <dir> [--name <text>]   an installable web app (docs/mobile-backend.md)
    cancho-ui build <file.cui> --page -o <dir> [--name <text>]  a web page that is not an app: index.html and app.js only
    cancho-ui inspect component <file.cui> <name> [--output json]
    cancho-ui inspect effects   <file.cui> <name> [--output json]
    cancho-ui inspect ir       <file.cui>          [--output json]
    cancho-ui inspect page     <file.cui>          [--output json]
    cancho-ui diff <base.cui> <candidate.cui>      [--output json]
    cancho-ui test <file.cui> <trace.json>         [--output json] [--full]
    cancho-ui report           <file.cui>...       [--output json]
    cancho-ui skill [--output json] [<out-file>]
    cancho-ui introspect [--output json]
    cancho-ui --version
    cancho-ui --help

Exit codes (matching cancho Core's CLI):
    0  success
    1  refused (located diagnostic printed)
    2  command line wrong
    3  environment failed (missing file, read error)
    4  (test only) a test failed: an expectation, a malformed trace, a refusal
    5  (diff only) the programs differ semantically; 0 means no change

Note on --output json: where supported, the JSON shape is locked
in spec/diagnostics/README.md §4. Consumers match on `code`, not
on `message`.
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(code) => code,
        Err(CliError::Usage(msg)) => {
            eprintln!("{msg}");
            eprintln!("\n{USAGE}");
            ExitCode::from(2)
        }
        Err(CliError::Env(msg)) => {
            eprintln!("cancho-ui: {msg}");
            ExitCode::from(3)
        }
    }
}

#[derive(Debug)]
enum CliError {
    Usage(String),
    Env(String),
}

fn run(args: &[String]) -> Result<ExitCode, CliError> {
    if args.is_empty() {
        return Err(CliError::Usage("missing command".into()));
    }
    match args[0].as_str() {
        "--help" | "-h" => {
            println!("{USAGE}");
            Ok(ExitCode::from(0))
        }
        "--version" | "-V" => {
            println!(
                "cancho-ui {} (Phase 1, web backend)",
                env!("CARGO_PKG_VERSION")
            );
            Ok(ExitCode::from(0))
        }
        "check" => run_check(&args[1..]),
        "build" => run_build(&args[1..]),
        "inspect" => run_inspect(&args[1..]),
        "diff" => diff_cmd::run_diff(&args[1..]),
        "test" => test_cmd::run_test(&args[1..]),
        "report" => run_report(&args[1..]),
        "skill" => run_skill(&args[1..]),
        "introspect" => run_introspect(&args[1..]),
        other => Err(CliError::Usage(format!("unknown command `{}`", other))),
    }
}

fn run_check(args: &[String]) -> Result<ExitCode, CliError> {
    let opts = CheckOpts::parse(args)?;
    let mut failures = 0;
    let mut all_diags: Vec<pipeline::Diagnostic> = Vec::new();
    for file in &opts.files {
        match pipeline::compile_to_ir(file) {
            Ok(_) => {}
            Err(pipeline::PipelineError::Refused(diag)) => {
                failures += 1;
                match opts.output {
                    OutputFormat::Human => {
                        eprintln!("{}", pipeline::format_diagnostic(&diag, file));
                    }
                    OutputFormat::Json => {
                        all_diags.push(*diag);
                    }
                }
            }
            Err(pipeline::PipelineError::Env(msg)) => {
                return Err(CliError::Env(msg));
            }
        }
    }
    if opts.output == OutputFormat::Json && !all_diags.is_empty() {
        let json = cancho_ui_ir::pretty_json(&all_diags);
        println!("{}", json);
    }
    if failures > 0 {
        Ok(ExitCode::from(1))
    } else {
        Ok(ExitCode::from(0))
    }
}

fn run_build(args: &[String]) -> Result<ExitCode, CliError> {
    let opts = BuildOpts::parse(args)?;
    let mut failures = 0;
    let mut all_diags: Vec<pipeline::Diagnostic> = Vec::new();
    for file in &opts.files {
        let emit = cancho_ui_codegen_web::EmitOptions {
            auto_mount: !opts.no_auto_mount,
            // a page or an app carries the skeleton of what the module mounts, so the module builds only what the skeleton leaves to it
            hydrate: opts.pwa || opts.page,
        };
        match pipeline::compile_to_js(file, opts.target.as_deref(), &emit) {
            Ok(out) if opts.pwa || opts.page => {
                let Some(dir) = opts.output.as_ref() else {
                    return Err(CliError::Usage(
                        "`--pwa` and `--page` need `-o <dir>`".to_string(),
                    ));
                };
                let name = opts.name.clone().unwrap_or_else(|| {
                    file.file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("app")
                        .to_string()
                });
                // a program with a map gets a Content-Security-Policy listing the hosts it declared (docs/maps.md)
                let ir = pipeline::compile_to_ir(file).ok();
                let csp = ir.as_ref().and_then(cancho_ui_ir::island::page_policy);
                // a theme (docs/theme.md): its stylesheet, and its font files copied into the build
                let theme = match ir.as_ref().and_then(|i| i.theme.as_ref()) {
                    Some(t) => {
                        let dir = file.parent().unwrap_or_else(|| std::path::Path::new(""));
                        let fonts = theme::load_fonts(t, dir, file)
                            .map_err(|d| CliError::Env(d.message.clone()))?;
                        Some(pwa::ThemeAssets {
                            css: theme::css(t),
                            fonts,
                        })
                    }
                    None => None,
                };
                // the pictures and videos the views name, copied into the build at the paths the page uses
                let mut site = match ir.as_ref() {
                    Some(i) => {
                        assets::load(i, file.parent().unwrap_or_else(|| std::path::Path::new("")))
                            .map_err(CliError::Env)?
                    }
                    None => Vec::new(),
                };
                // the head and the skip link of a `page { .. }` (docs/page.md), and the files it names, copied beside `index.html`
                let page = ir.as_ref().map(page_files::parts).unwrap_or_default();
                if let Some(i) = ir.as_ref() {
                    let files = page_files::copies(
                        i,
                        file.parent().unwrap_or_else(|| std::path::Path::new("")),
                    )
                    .map_err(CliError::Env)?;
                    for f in files {
                        if !site.iter().any(|(p, _)| *p == f.0) {
                            site.push(f);
                        }
                    }
                }
                // the static part of the component that is mounted into the page, written into `index.html` (docs/prerender.md)
                let prerender = if opts.no_auto_mount {
                    None
                } else {
                    match ir.as_ref().and_then(cancho_ui_ir::recipe::hydrating) {
                        // the page carries the skeleton and the module hydrates it (the same function decided in the module)
                        Some((_, skeleton)) => Some(skeleton),
                        None => ir
                            .as_ref()
                            .and_then(|i| i.components.last())
                            .map(|c| cancho_ui_ir::recipe::prerender(&c.view.body)),
                    }
                };
                let files = if opts.pwa {
                    pwa::generate(
                        &out,
                        &name,
                        csp.as_deref(),
                        theme.as_ref(),
                        &site,
                        prerender.as_ref(),
                        &page,
                    )
                } else {
                    pwa::generate_page(
                        &out,
                        &name,
                        csp.as_deref(),
                        theme.as_ref(),
                        &site,
                        prerender.as_ref(),
                        &page,
                    )
                };
                pwa::write_all(dir, &files).map_err(CliError::Env)?;
            }
            Ok(out) => match &opts.output {
                Some(path) => {
                    std::fs::write(path, &out).map_err(|e| {
                        CliError::Env(format!("writing `{}`: {}", path.display(), e))
                    })?;
                }
                None => {
                    println!("{}", out);
                }
            },
            Err(pipeline::PipelineError::Refused(diag)) => {
                failures += 1;
                match opts.output_format {
                    OutputFormat::Human => {
                        eprintln!("{}", pipeline::format_diagnostic(&diag, file));
                    }
                    OutputFormat::Json => all_diags.push(*diag),
                }
            }
            Err(pipeline::PipelineError::Env(msg)) => {
                return Err(CliError::Env(msg));
            }
        }
    }
    if opts.output_format == OutputFormat::Json && !all_diags.is_empty() {
        let json = cancho_ui_ir::pretty_json(&all_diags);
        println!("{}", json);
    }
    if failures > 0 {
        Ok(ExitCode::from(1))
    } else {
        Ok(ExitCode::from(0))
    }
}

fn run_inspect(args: &[String]) -> Result<ExitCode, CliError> {
    let opts = InspectOpts::parse(args)?;
    let ir = pipeline::compile_to_ir(&opts.file).map_err(map_pipeline_env)?;
    let json = match opts.what.as_str() {
        "component" => {
            let name = opts.arg.as_ref().ok_or_else(|| {
                CliError::Usage("`inspect component` requires a component name".into())
            })?;
            cancho_ui_ir::pretty_json(&ir.components.iter().find(|c| &c.name == name))
        }
        "effects" => {
            let name = opts.arg.as_ref().ok_or_else(|| {
                CliError::Usage("`inspect effects` requires a component name".into())
            })?;
            let comp = ir
                .components
                .iter()
                .find(|c| &c.name == name)
                .ok_or_else(|| CliError::Env(format!("component `{}` not found", name)))?;
            let mut v = serde_json::json!({
                "component": comp.name,
                "requires": comp.requires,
                "reachable_effects": comp.reachable_effects,
            });
            // the hosts a `Link` names (docs/structure.md): listed, because they are where a reader can be sent; not an effect
            let links = cancho_ui_ir::structure::link_hosts(&comp.view.body);
            if !links.is_empty()
                && let Some(o) = v.as_object_mut()
            {
                o.insert("links".to_string(), serde_json::json!(links));
            }
            // the hosts the page's head names (docs/page.md section 3): where a crawler is sent; the page makes no request to them. Listed on the
            // component that is mounted into the page, which is the last
            if ir.components.last().is_some_and(|c| c.name == comp.name)
                && let Some(p) = &ir.page
            {
                let hosts = cancho_ui_ir::page::hosts(p);
                if !hosts.is_empty()
                    && let Some(o) = v.as_object_mut()
                {
                    o.insert("page_hosts".to_string(), serde_json::json!(hosts));
                }
            }
            // the sealed islands the view instantiates (docs/islands.md section 6): what each runs, by hash, and what it reaches
            let used: Vec<serde_json::Value> = ir
                .islands
                .iter()
                .filter(|i| cancho_ui_ir::island::view_uses(&comp.view.body, &i.name))
                .map(cancho_ui_ir::island::describe)
                .collect();
            if !used.is_empty()
                && let Some(o) = v.as_object_mut()
            {
                o.insert("islands".to_string(), serde_json::json!(used));
            }
            cancho_ui_ir::pretty_json(&v)
        }
        "page" => match &ir.page {
            // the whole head as it will be written: the checked values, the card's absolute address, the files with their hashes, the hosts
            Some(p) => cancho_ui_ir::pretty_json(&cancho_ui_ir::page::to_json(p)),
            None => cancho_ui_ir::pretty_json(&serde_json::Value::Null),
        },
        "prerender" => {
            // what `build --page` writes into the page for this component and what it leaves to the script (docs/prerender.md)
            let name = opts.arg.as_ref().ok_or_else(|| {
                CliError::Usage("`inspect prerender` requires a component name".into())
            })?;
            let idx = ir
                .components
                .iter()
                .position(|c| &c.name == name)
                .ok_or_else(|| CliError::Env(format!("component `{}` not found", name)))?;
            let p = cancho_ui_ir::recipe::prerender(&ir.components[idx].view.body);
            cancho_ui_ir::pretty_json(&serde_json::json!({
                "component": name,
                // only the last component of the entry file is mounted into the page
                "mounted": idx + 1 == ir.components.len(),
                // whether `build --page` hydrates this page (the script builds only the holes) or builds all of it
                "hydrates": cancho_ui_ir::recipe::hydrating(&ir).is_some_and(|(i, _)| i == idx),
                "static_elements": p.static_elements,
                "content": p.content,
                "html_bytes": p.html.len(),
                "holes": p.holes.iter().map(|h| serde_json::json!({"what": h.what, "reason": h.reason})).collect::<Vec<_>>(),
            }))
        }
        "ir" => cancho_ui_ir::pretty_json(&ir),
        other => {
            return Err(CliError::Usage(format!(
                "unknown `inspect` subject `{}`",
                other
            )));
        }
    };
    match opts.output_path {
        Some(path) => {
            let p = path.clone();
            std::fs::write(&path, &json)
                .map_err(move |e| CliError::Env(format!("writing `{}`: {}", p.display(), e)))?;
        }
        None => println!("{}", json),
    }
    Ok(ExitCode::from(0))
}

fn run_report(args: &[String]) -> Result<ExitCode, CliError> {
    let opts = ReportOpts::parse(args)?;
    let mut failures = 0;
    let mut all_diags: Vec<pipeline::Diagnostic> = Vec::new();
    let mut report = serde_json::json!({
        "schema": "cancho-ui-report",
        "version": "0.1.0",
        "static": { "checks_run": [], "checks_passed": 0, "checks_failed": 0, "diagnostics": [] },
        "tested": { "tests_run": [], "tests_passed": 0, "tests_failed": 0, "details": [] },
        "runtime": { "checks_performed": [], "checks_passed": 0, "checks_failed": 0, "details": [] },
        "trust": { "boundaries": [], "count": 0 },
        "unknown": { "items": [] },
    });
    for file in &opts.files {
        match pipeline::compile_to_ir(file) {
            Ok(ir) => {
                let trust_count = ir
                    .components
                    .iter()
                    .map(|c| c.requires.len())
                    .sum::<usize>();
                report["static"]["checks_passed"] =
                    serde_json::json!(report["static"]["checks_passed"].as_u64().unwrap_or(0) + 1);
                report["trust"]["count"] = serde_json::json!(
                    report["trust"]["count"].as_u64().unwrap_or(0)
                        + trust_count as u64
                        + ir.islands.len() as u64
                );
                // each island is a boundary: code the program did not write, pinned by hash (docs/islands.md section 6)
                for i in &ir.islands {
                    let mut d = cancho_ui_ir::island::describe(i);
                    d["kind"] = serde_json::json!(if i.sealed {
                        "sealed island"
                    } else {
                        "open island"
                    });
                    d["from"] = serde_json::json!("island");
                    d["symbol"] = serde_json::json!(i.name);
                    d["tier"] = serde_json::json!("Trust");
                    if let Some(b) = report["trust"]["boundaries"].as_array_mut() {
                        b.push(d);
                    }
                }
            }
            Err(pipeline::PipelineError::Refused(diag)) => {
                failures += 1;
                if opts.output == OutputFormat::Human {
                    eprintln!("{}", pipeline::format_diagnostic(&diag, file));
                } else {
                    all_diags.push(*diag);
                }
            }
            Err(pipeline::PipelineError::Env(msg)) => {
                return Err(CliError::Env(msg));
            }
        }
    }
    if opts.output == OutputFormat::Json {
        if !all_diags.is_empty() {
            let json = cancho_ui_ir::pretty_json(&all_diags);
            println!("{}", json);
        } else {
            println!("{}", cancho_ui_ir::pretty_json(&report));
        }
    } else {
        println!("{}", cancho_ui_ir::pretty_json(&report));
    }
    if failures > 0 {
        Ok(ExitCode::from(1))
    } else {
        Ok(ExitCode::from(0))
    }
}

fn run_skill(args: &[String]) -> Result<ExitCode, CliError> {
    // Emits an agentskills.io-shaped markdown describing every
    // command. Phase 1 emits a hand-written guide; Phase 2
    // generates it from the registered CommandInfo vector.
    let opts = SelfDescribeOpts::parse("skill", args)?;
    let body = match opts.format {
        OutputFormat::Human => SKILL_MD.to_string(),
        OutputFormat::Json => cancho_ui_ir::pretty_json(&serde_json::json!({
            "schema": "cancho-ui-skill",
            "version": "0.1.0",
            "name": "cancho-ui",
            "format": "markdown",
            "content": SKILL_MD,
        })),
    };
    match opts.output_path {
        Some(path) => {
            let p = path.clone();
            std::fs::write(&path, &body)
                .map_err(move |e| CliError::Env(format!("writing `{}`: {}", p.display(), e)))?;
        }
        None => println!("{}", body),
    }
    Ok(ExitCode::from(0))
}

fn run_introspect(args: &[String]) -> Result<ExitCode, CliError> {
    // The command tree is data: `--output text` prints the same JSON.
    let opts = SelfDescribeOpts::parse("introspect", args)?;
    let tree = serde_json::json!({
        "schema": "cancho-ui-cli",
        "version": "0.1.0",
        "commands": [
            { "name": "check",       "args": ["<file.cui>...", "--output json"] },
            { "name": "build",       "args": ["<file.cui>...", "-o <path>", "--target web", "--no-auto-mount", "--pwa", "--page", "--name <text>", "--output json"] },
            { "name": "inspect",     "subcommands": ["component <file> <name>", "effects <file> <name>", "page <file>", "ir <file>"], "args": ["--output json"] },
            { "name": "diff",        "args": ["<base.cui>", "<candidate.cui>", "--output json"], "exit_codes": { "0": "no semantic change", "1": "a file was refused", "2": "command line wrong", "3": "environment failed", "5": "the programs differ semantically" } },
            { "name": "test",        "args": ["<file.cui>", "<trace.json>", "--output json", "--full"], "exit_codes": { "0": "the trace passed", "1": "the program did not compile", "2": "command line wrong", "3": "environment failed", "4": "a test failed (expectation, malformed trace, or a construct replay refuses)" } },
            { "name": "report",      "args": ["<file.cui>...", "--output json"] },
            { "name": "skill",       "args": ["--output json", "[<out-file>]"] },
            { "name": "introspect",  "args": ["--output json"] },
        ],
        "exit_codes": { "0": "success", "1": "refused", "2": "command line wrong", "3": "environment failed", "4": "test only: a test failed", "5": "diff only: the programs differ semantically" },
        "refusal_catalog_ref": "spec/diagnostics/README.md §3",
        "ir_schema_ref": "spec/ir-schema/README.md",
    });
    let json = cancho_ui_ir::pretty_json(&tree);
    match opts.output_path {
        Some(path) => {
            let p = path.clone();
            std::fs::write(&path, &json)
                .map_err(move |e| CliError::Env(format!("writing `{}`: {}", p.display(), e)))?;
        }
        None => println!("{}", json),
    }
    Ok(ExitCode::from(0))
}

fn map_pipeline_env(e: pipeline::PipelineError) -> CliError {
    match e {
        pipeline::PipelineError::Env(msg) => CliError::Env(msg),
        other => CliError::Env(format!("refused: {:?}", other)),
    }
}

#[derive(Debug, Default)]
struct CheckOpts {
    files: Vec<PathBuf>,
    output: OutputFormat,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum OutputFormat {
    #[default]
    Human,
    Json,
}

impl CheckOpts {
    fn parse(args: &[String]) -> Result<Self, CliError> {
        let mut out = CheckOpts::default();
        let mut i = 0;
        while i < args.len() {
            let a = &args[i];
            match a.as_str() {
                "--output" => {
                    let v = args
                        .get(i + 1)
                        .ok_or_else(|| CliError::Usage("--output requires an argument".into()))?;
                    out.output = match v.as_str() {
                        "json" => OutputFormat::Json,
                        "text" | "human" => OutputFormat::Human,
                        other => {
                            return Err(CliError::Usage(format!(
                                "unknown --output value `{}`",
                                other
                            )));
                        }
                    };
                    i += 2;
                    continue;
                }
                s if s.starts_with("--") => {
                    return Err(CliError::Usage(format!("unknown flag `{}`", s)));
                }
                _ => out.files.push(PathBuf::from(a)),
            }
            i += 1;
        }
        if out.files.is_empty() {
            return Err(CliError::Usage("`check` requires at least one file".into()));
        }
        Ok(out)
    }
}

#[derive(Debug, Default)]
struct BuildOpts {
    files: Vec<PathBuf>,
    output: Option<PathBuf>,
    target: Option<String>,
    output_format: OutputFormat,
    pwa: bool,
    /// `--page`: a web page that is not an app (docs/mobile-backend.md section 5.2).
    page: bool,
    no_auto_mount: bool,
    name: Option<String>,
}

impl BuildOpts {
    fn parse(args: &[String]) -> Result<Self, CliError> {
        let mut out = BuildOpts::default();
        let mut i = 0;
        while i < args.len() {
            let a = &args[i];
            match a.as_str() {
                "--target" => {
                    let v = args
                        .get(i + 1)
                        .ok_or_else(|| CliError::Usage("--target requires an argument".into()))?;
                    out.target = Some(v.clone());
                    i += 2;
                    continue;
                }
                "--pwa" => {
                    out.pwa = true;
                    i += 1;
                    continue;
                }
                "--page" => {
                    out.page = true;
                    i += 1;
                    continue;
                }
                "--no-auto-mount" => {
                    out.no_auto_mount = true;
                    i += 1;
                    continue;
                }
                "--name" => {
                    let v = args
                        .get(i + 1)
                        .ok_or_else(|| CliError::Usage("--name requires an argument".into()))?;
                    out.name = Some(v.clone());
                    i += 2;
                    continue;
                }
                "--output" => {
                    let v = args
                        .get(i + 1)
                        .ok_or_else(|| CliError::Usage("--output requires an argument".into()))?;
                    out.output_format = match v.as_str() {
                        "json" => OutputFormat::Json,
                        "text" | "human" => OutputFormat::Human,
                        other => {
                            return Err(CliError::Usage(format!(
                                "unknown --output value `{}`",
                                other
                            )));
                        }
                    };
                    i += 2;
                    continue;
                }
                "-o" => {
                    let v = args
                        .get(i + 1)
                        .ok_or_else(|| CliError::Usage("-o requires an argument".into()))?;
                    out.output = Some(PathBuf::from(v));
                    i += 2;
                    continue;
                }
                s if s.starts_with("--") => {
                    return Err(CliError::Usage(format!("unknown flag `{}`", s)));
                }
                _ => out.files.push(PathBuf::from(a)),
            }
            i += 1;
        }
        if out.files.is_empty() {
            return Err(CliError::Usage("`build` requires at least one file".into()));
        }
        if out.pwa && out.page {
            return Err(CliError::Usage(
                "`--pwa` and `--page` are two different outputs: an installable app, or a page that is not one"
                    .into(),
            ));
        }
        if (out.pwa || out.page) && out.no_auto_mount {
            return Err(CliError::Usage(
                "`--no-auto-mount` cannot be combined with `--pwa` or `--page`: the page relies on the module mounting itself"
                    .into(),
            ));
        }
        if out.pwa || out.page {
            if out.files.len() != 1 {
                return Err(CliError::Usage(
                    "`build --pwa` and `build --page` take exactly one file".into(),
                ));
            }
            if out.output.is_none() {
                return Err(CliError::Usage(
                    "`build --pwa` and `build --page` require `-o <dir>`".into(),
                ));
            }
        } else if out.name.is_some() {
            return Err(CliError::Usage(
                "`--name` is only for `build --pwa` and `build --page`".into(),
            ));
        }
        Ok(out)
    }
}

#[derive(Debug, Default)]
struct InspectOpts {
    what: String,
    file: PathBuf,
    arg: Option<String>,
    output_path: Option<PathBuf>,
}

impl InspectOpts {
    fn parse(args: &[String]) -> Result<Self, CliError> {
        let mut out = InspectOpts::default();
        if args.is_empty() {
            return Err(CliError::Usage(
                "`inspect` requires a subject (`component`, `effects`, `prerender`, `page`, `ir`)"
                    .into(),
            ));
        }
        out.what = args[0].to_string();
        let mut i = 1;
        while i < args.len() {
            let a = &args[i];
            match a.as_str() {
                "--output" => {
                    // The value (`json`) belongs to the flag, not to `arg`.
                    i += 2;
                    continue;
                }
                "-o" => {
                    let v = args
                        .get(i + 1)
                        .ok_or_else(|| CliError::Usage("-o requires an argument".into()))?;
                    out.output_path = Some(PathBuf::from(v));
                    i += 2;
                    continue;
                }
                s if s.starts_with("--") => {
                    return Err(CliError::Usage(format!("unknown flag `{}`", s)));
                }
                _ if out.file.as_os_str().is_empty() => {
                    out.file = PathBuf::from(a);
                    i += 1;
                }
                _ => {
                    out.arg = Some(a.clone());
                    i += 1;
                }
            }
        }
        if out.file.as_os_str().is_empty() {
            return Err(CliError::Usage(
                "`inspect` requires a <file.cui> argument".into(),
            ));
        }
        Ok(out)
    }
}

#[derive(Debug, Default)]
struct ReportOpts {
    files: Vec<PathBuf>,
    output: OutputFormat,
}

impl ReportOpts {
    fn parse(args: &[String]) -> Result<Self, CliError> {
        let mut out = ReportOpts::default();
        let mut i = 0;
        while i < args.len() {
            let a = &args[i];
            match a.as_str() {
                "--output" => {
                    let v = args
                        .get(i + 1)
                        .ok_or_else(|| CliError::Usage("--output requires an argument".into()))?;
                    out.output = match v.as_str() {
                        "json" => OutputFormat::Json,
                        _ => OutputFormat::Human,
                    };
                    i += 2;
                    continue;
                }
                s if s.starts_with("--") => {
                    return Err(CliError::Usage(format!("unknown flag `{}`", s)));
                }
                _ => out.files.push(PathBuf::from(a)),
            }
            i += 1;
        }
        if out.files.is_empty() {
            return Err(CliError::Usage(
                "`report` requires at least one file".into(),
            ));
        }
        Ok(out)
    }
}

/// Arguments of the two self-describing commands, `skill` and
/// `introspect`: `[--output json] [<out-file>]`. The flag takes its
/// value; a bare argument is the output file; anything else is a usage
/// error (it used to be silently taken as the output path, so
/// `introspect --output json` wrote a file literally named `--output`).
#[derive(Debug, Default)]
struct SelfDescribeOpts {
    output_path: Option<PathBuf>,
    format: OutputFormat,
}

impl SelfDescribeOpts {
    fn parse(command: &str, args: &[String]) -> Result<Self, CliError> {
        let mut out = SelfDescribeOpts::default();
        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--output" => {
                    let v = args.get(i + 1).ok_or_else(|| {
                        CliError::Usage(format!("`{command}`: --output requires an argument"))
                    })?;
                    out.format = match v.as_str() {
                        "json" => OutputFormat::Json,
                        "text" | "human" => OutputFormat::Human,
                        other => {
                            return Err(CliError::Usage(format!(
                                "unknown --output value `{other}`"
                            )));
                        }
                    };
                    i += 2;
                }
                s if s.starts_with('-') && s.len() > 1 => {
                    return Err(CliError::Usage(format!("`{command}`: unknown flag `{s}`")));
                }
                s => {
                    if out.output_path.is_some() {
                        return Err(CliError::Usage(format!(
                            "`{command}` takes at most one output file; got a second, `{s}`"
                        )));
                    }
                    out.output_path = Some(PathBuf::from(s));
                    i += 1;
                }
            }
        }
        Ok(out)
    }
}

const SKILL_MD: &str = "\
---
name: cancho-ui
description: Compile cancho-ui source (.cui files) to JavaScript+DOM for the browser, with rule-tagged refusals and machine-readable diagnostics.
---

# cancho-ui

A compiler for a small declarative UI language whose programs compile to
JavaScript and DOM, with the capability model, refusal discipline, and
machine-readable diagnostics modelled on [cancho](https://github.com/alpibrusl/cancho).

## When to use

- You are authoring or modifying a `.cui` source file and need a check / build / inspect pipeline that an agent can drive.
- You need to know what capability, state, or effect a component declares — read the IR.
- You need a deterministic refusal code, not a stack trace, when something is wrong.

## When not to use

- You are doing anything that needs real-time DOM interaction. Use a browser, not this CLI.
- You need to write code. This tool only checks and emits.

## Quick start

```sh
cancho-ui check <file.cui>                       # parse + type-check
cancho-ui check <file.cui> --output json        # machine-readable
cancho-ui build <file.cui> -o <file.js>          # compile to JS
cancho-ui inspect component <file.cui> <name>   # one component, as data
cancho-ui inspect effects <file.cui> <name>     # reachable effects + capabilities
cancho-ui inspect ir <file.cui>                 # full IR
cancho-ui diff <base.cui> <candidate.cui>       # semantic diff: authority, state, events, effects
cancho-ui test <file.cui> <trace.json>          # replay an event trace; check state after each step (--full: no value cut to 120 chars)
cancho-ui report <file.cui>                     # evidence report (5 sections)
cancho-ui skill                                 # this file
cancho-ui introspect                            # full command tree as data
```

Exit codes match cancho Core: `0` ok, `1` refused, `2` command line wrong, `3` environment failed. `diff` adds `5` = the programs differ (`0` = no semantic change; `1` stays 'a file was refused'). `test` adds `4` = a test failed (an expectation, a malformed trace, or a construct replay refuses; read the `rule`). Read `authority` in the diff first: `expanded` means a component gained a capability.

## Refusal discipline

Match on `code`, not on `message`. The shape is locked in `spec/diagnostics/README.md`. A refusal says:

- `code` — stable kebab-case tag (`type-not-pascal-case`, `capability-missing`, …).
- `severity` — `error` today; warnings are a refusal tomorrow.
- `tier` — `static | tested | runtime | trust | unknown`.
- `subject` — what is being refused.
- `span` — file + start/end line/column.
- `message` — one line, agent-readable.
- `remediation` — when the fix is mechanical; never an LLM-generated explanation.

See `spec/diagnostics/README.md` §3 for the full catalog. The
catalog grows with each phase; the shape is the contract.

## Tier semantics

Every guarantee is one of `static | tested | runtime | trust | unknown`. The five sections of the evidence report (`cancho-ui report`) correspond to these tiers. The report never claims a guarantee the implementation does not enforce.

## Authority

The Tier 2 compiler emits JavaScript + DOM that runs in any browser. It does not call any platform API without a declared capability (Phase 2 enforces; Phase 1 emits the declarations). Foreign JavaScript is identified as a trust boundary, never silently typed.

## Where to read more

- `docs/proposal.md` — the position and Phase 1 exit criteria.
- `docs/semantics.md` — the 12 semantic questions answered with guarantee tiers.
- `docs/semantic-diff.md` — what `diff` compares, its JSON shape, and what it does not claim.
- `docs/replay.md` — the trace format, what `test` replays, the scripted network and fake clock, and what it does not claim.
- `docs/security-model.md` — the five-tier guarantee model and capability/effect rules.
- `spec/diagnostics/README.md` — the diagnostic format and rule catalog.
- `spec/ir-schema/README.md` — the typed UI IR.
";
