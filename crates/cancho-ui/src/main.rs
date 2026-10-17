//! `cancho-ui` — the compiler CLI.
//!
//! Phase 1 supports two commands:
//!
//! - `cancho-ui check <file.cui>...`  — parse, type-check, lower.
//!   Exits 0 on success, 1 on refusal (with a located diagnostic),
//!   2 on command-line error.
//!
//! - `cancho-ui build <file.cui>...`  — same pipeline, then emit
//!   JavaScript to stdout (or to `-o <path>` if given).
//!
//! Exit codes match Cancho Core's:
//!   0 success, 1 refused, 2 command line wrong, 3 environment.
//!
//! The CLI is hand-rolled (no clap) for the same reason the cancho
//! binary is: we want stable, agent-facing command names without a
//! generator's defaults. See `docs/agent-cli.md` in Cancho Core for
//! the lessons; in particular, single-dash flags render with their
//! literal dash in `cancho-ui skill` output (Phase 2).

mod pipeline;

use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "\
cancho-ui — Cancho UI compiler CLI

USAGE:
    cancho-ui check <file.cui>... [--output json]
    cancho-ui build <file.cui>... [-o <path>] [--target web]
    cancho-ui --version
    cancho-ui --help

Exit codes (matching Cancho Core's CLI):
    0  success
    1  refused (located diagnostic printed)
    2  command line wrong
    3  environment failed (missing file, read error)
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
        other => Err(CliError::Usage(format!("unknown command `{}`", other))),
    }
}

fn run_check(args: &[String]) -> Result<ExitCode, CliError> {
    let opts = CheckOpts::parse(args)?;
    let mut failures = 0;
    for file in &opts.files {
        match pipeline::compile_to_ir(file) {
            Ok(_) => {}
            Err(pipeline::PipelineError::Refused(diag)) => {
                failures += 1;
                eprintln!("{}", pipeline::format_diagnostic(&diag, file));
            }
            Err(pipeline::PipelineError::Env(msg)) => {
                return Err(CliError::Env(msg));
            }
        }
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
    for file in &opts.files {
        match pipeline::compile_to_js(file, opts.target.as_deref()) {
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
                eprintln!("{}", pipeline::format_diagnostic(&diag, file));
            }
            Err(pipeline::PipelineError::Env(msg)) => {
                return Err(CliError::Env(msg));
            }
        }
    }
    if failures > 0 {
        Ok(ExitCode::from(1))
    } else {
        Ok(ExitCode::from(0))
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
                    let v = args.get(i + 1).ok_or_else(|| {
                        CliError::Usage("--output requires an argument".into())
                    })?;
                    out.output = match v.as_str() {
                        "json" => OutputFormat::Json,
                        "text" | "human" => OutputFormat::Human,
                        other => {
                            return Err(CliError::Usage(format!(
                                "unknown --output value `{}`",
                                other
                            )))
                        }
                    };
                    i += 2;
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
}

impl BuildOpts {
    fn parse(args: &[String]) -> Result<Self, CliError> {
        let mut out = BuildOpts::default();
        let mut i = 0;
        while i < args.len() {
            let a = &args[i];
            match a.as_str() {
                "--target" => {
                    let v = args.get(i + 1).ok_or_else(|| {
                        CliError::Usage("--target requires an argument".into())
                    })?;
                    out.target = Some(v.clone());
                    i += 2;
                }
                "-o" => {
                    let v = args
                        .get(i + 1)
                        .ok_or_else(|| CliError::Usage("-o requires an argument".into()))?;
                    out.output = Some(PathBuf::from(v));
                    i += 2;
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
        Ok(out)
    }
}