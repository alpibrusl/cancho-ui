//! `cancho-ui test <file.cui> <trace.json> [--output json] [--full]`.
//!
//! Glue only: the program goes through the same pipeline as `check`, and the
//! engine in `cancho-ui-replay` interprets the typed IR over the trace. Exit
//! codes: 0 the trace passed, 1 the program did not compile, 2 command line
//! wrong, 3 environment failed, 4 a test failed (docs/replay.md §7).

use std::path::PathBuf;
use std::process::ExitCode;

use crate::pipeline::{self, PipelineError};
use crate::{CliError, OutputFormat};

/// Exit code for "a test failed" (CONTRIBUTING.md §5 reserves 4 for `test`).
pub const EXIT_TEST_FAILED: u8 = 4;

#[derive(Debug)]
struct TestOpts {
    file: PathBuf,
    trace: PathBuf,
    output: OutputFormat,
    full: bool,
}

impl TestOpts {
    fn parse(args: &[String]) -> Result<Self, CliError> {
        let mut files: Vec<PathBuf> = Vec::new();
        let mut output = OutputFormat::Human;
        let mut full = false;
        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--output" => {
                    let v = args
                        .get(i + 1)
                        .ok_or_else(|| CliError::Usage("--output requires an argument".into()))?;
                    output = match v.as_str() {
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
                "--full" => {
                    full = true;
                    i += 1;
                }
                s if s.starts_with('-') && s.len() > 1 => {
                    return Err(CliError::Usage(format!("unknown flag `{s}`")));
                }
                s => {
                    files.push(PathBuf::from(s));
                    i += 1;
                }
            }
        }
        if files.len() != 2 {
            return Err(CliError::Usage(format!(
                "`test` takes exactly a program and a trace, <file.cui> <trace.json>; got {} file(s)",
                files.len()
            )));
        }
        let trace = files.pop().expect("two files");
        let file = files.pop().expect("two files");
        Ok(TestOpts {
            file,
            trace,
            output,
            full,
        })
    }
}

pub fn run_test(args: &[String]) -> Result<ExitCode, CliError> {
    let opts = TestOpts::parse(args)?;
    let ir = match pipeline::compile_to_ir(&opts.file) {
        Ok(ir) => ir,
        Err(PipelineError::Refused(d)) => {
            match opts.output {
                OutputFormat::Human => eprintln!("{}", pipeline::format_diagnostic(&d, &opts.file)),
                OutputFormat::Json => println!(
                    "{}",
                    serde_json::to_string_pretty(&[&*d])
                        .expect("diagnostic serialization is infallible")
                ),
            }
            return Ok(ExitCode::from(1));
        }
        Err(PipelineError::Env(msg)) => return Err(CliError::Env(msg)),
    };
    let bytes = std::fs::read(&opts.trace)
        .map_err(|e| CliError::Env(format!("reading `{}`: {}", opts.trace.display(), e)))?;
    // A trace that is not UTF-8 is a malformed trace, not an environment error:
    // the lossy text fails JSON parsing with a located `trace-malformed`.
    let text = String::from_utf8_lossy(&bytes);
    let report = cancho_ui_replay::replay(
        &ir,
        &text,
        &opts.file.display().to_string(),
        &opts.trace.display().to_string(),
    );
    match opts.output {
        OutputFormat::Human if opts.full => {
            print!("{}", cancho_ui_replay::render_human_full(&report))
        }
        OutputFormat::Human => print!("{}", cancho_ui_replay::render_human(&report)),
        OutputFormat::Json => println!(
            "{}",
            serde_json::to_string_pretty(&report).expect("report serialization is infallible")
        ),
    }
    Ok(ExitCode::from(if report.passed() {
        0
    } else {
        EXIT_TEST_FAILED
    }))
}
