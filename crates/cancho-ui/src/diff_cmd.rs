//! `cancho-ui diff <base.cui> <candidate.cui> [--output json]`.
//!
//! Glue only: both files go through the same pipeline as `check`, and
//! the engine in `cancho-ui-diff` compares the two typed IRs. Exit
//! codes: 0 no semantic change, 1 a file was refused, 2 command line
//! wrong, 3 environment failed, 5 the programs differ
//! (docs/semantic-diff.md §5 for why 5 and not diff(1)'s 1).

use std::path::PathBuf;
use std::process::ExitCode;

use crate::pipeline::{self, PipelineError};
use crate::{CliError, OutputFormat};

/// Exit code for "the programs differ semantically".
pub const EXIT_DIFFERS: u8 = 5;

#[derive(Debug)]
struct DiffOpts {
    base: PathBuf,
    candidate: PathBuf,
    output: OutputFormat,
}

impl DiffOpts {
    fn parse(args: &[String]) -> Result<Self, CliError> {
        let mut files: Vec<PathBuf> = Vec::new();
        let mut output = OutputFormat::Human;
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
                "`diff` takes exactly two files, <base.cui> <candidate.cui>; got {}",
                files.len()
            )));
        }
        let candidate = files.pop().expect("two files");
        let base = files.pop().expect("two files");
        Ok(DiffOpts {
            base,
            candidate,
            output,
        })
    }
}

pub fn run_diff(args: &[String]) -> Result<ExitCode, CliError> {
    let opts = DiffOpts::parse(args)?;
    // Compile both before reporting, so one run shows every refusal.
    let mut irs = Vec::new();
    let mut diags: Vec<(PathBuf, pipeline::Diagnostic)> = Vec::new();
    for file in [&opts.base, &opts.candidate] {
        match pipeline::compile_to_ir(file) {
            Ok(ir) => irs.push(ir),
            Err(PipelineError::Refused(d)) => diags.push((file.clone(), *d)),
            Err(PipelineError::Env(msg)) => return Err(CliError::Env(msg)),
        }
    }
    if !diags.is_empty() {
        match opts.output {
            OutputFormat::Human => {
                for (file, d) in &diags {
                    eprintln!("{}", pipeline::format_diagnostic(d, file));
                }
            }
            OutputFormat::Json => {
                let only: Vec<&pipeline::Diagnostic> = diags.iter().map(|(_, d)| d).collect();
                println!(
                    "{}",
                    serde_json::to_string_pretty(&only)
                        .expect("diagnostic serialization is infallible")
                );
            }
        }
        return Ok(ExitCode::from(1));
    }
    let report = cancho_ui_diff::diff_ir(
        &irs[0],
        &irs[1],
        &opts.base.display().to_string(),
        &opts.candidate.display().to_string(),
    );
    match opts.output {
        OutputFormat::Human => print!("{}", cancho_ui_diff::render_human(&report)),
        OutputFormat::Json => println!(
            "{}",
            serde_json::to_string_pretty(&report).expect("report serialization is infallible")
        ),
    }
    Ok(ExitCode::from(if report.changed {
        EXIT_DIFFERS
    } else {
        0
    }))
}
