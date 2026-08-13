//! Command-line interface for `DryMark`.

use std::fmt;
use std::io::{self, Read, Write};
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use drymark_core::{Policy, SanitizeReport, sanitize};
use zeroize::{Zeroize, Zeroizing};

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
const CHANGED_EXIT_CODE: u8 = 3;

#[derive(Debug, Parser)]
#[command(
    name = "drymark",
    version,
    about = "Remove inspectable hidden LLM watermark channels locally"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Remove watermark channels from UTF-8 read from standard input.
    Clean {
        /// Watermark removal policy.
        #[arg(long, value_enum, default_value_t = PolicyArg::Preserve)]
        policy: PolicyArg,
        /// Write nothing and exit 3 if watermark removal would change the input.
        #[arg(long)]
        check: bool,
    },
    /// Inspect UTF-8 from standard input without returning its contents.
    Scan {
        /// Watermark removal policy used for the inspection.
        #[arg(long, value_enum, default_value_t = PolicyArg::Preserve)]
        policy: PolicyArg,
        /// Emit a stable JSON report.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum PolicyArg {
    Preserve,
    Thorough,
}

impl From<PolicyArg> for Policy {
    fn from(value: PolicyArg) -> Self {
        match value {
            PolicyArg::Preserve => Self::PreserveAppearance,
            PolicyArg::Thorough => Self::Thorough,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CliError {
    InputRead,
    InputTooLarge,
    InvalidUtf8,
    OutputWrite,
    ReportWrite,
}

impl fmt::Display for CliError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InputRead => "could not read standard input",
            Self::InputTooLarge => "input exceeds the 16 MiB safety limit",
            Self::InvalidUtf8 => "input is not valid UTF-8",
            Self::OutputWrite => "could not write standard output",
            Self::ReportWrite => "could not write the watermark-removal report",
        })
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(&cli) {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("drymark: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: &Cli) -> Result<u8, CliError> {
    let input = Zeroizing::new(read_stdin_limited()?);
    match &cli.command {
        Command::Clean { policy, check } => {
            let result = sanitize(input.as_str(), (*policy).into());
            let output = Zeroizing::new(result.text);
            if *check {
                return Ok(if result.report.changed {
                    CHANGED_EXIT_CODE
                } else {
                    0
                });
            }
            write_bytes(output.as_bytes())?;
            Ok(0)
        }
        Command::Scan { policy, json } => {
            let result = sanitize(input.as_str(), (*policy).into());
            let _output = Zeroizing::new(result.text);
            let report = result.report;
            write_report(&report, *json)?;
            Ok(0)
        }
    }
}

fn read_stdin_limited() -> Result<String, CliError> {
    let mut bytes = Zeroizing::new(Vec::new());
    io::stdin()
        .take((MAX_INPUT_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| CliError::InputRead)?;
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(CliError::InputTooLarge);
    }
    match String::from_utf8(std::mem::take(&mut *bytes)) {
        Ok(text) => Ok(text),
        Err(error) => {
            let mut invalid = error.into_bytes();
            invalid.zeroize();
            Err(CliError::InvalidUtf8)
        }
    }
}

fn write_bytes(bytes: &[u8]) -> Result<(), CliError> {
    let mut stdout = io::stdout().lock();
    write_bytes_to(&mut stdout, bytes)
}

fn write_bytes_to(writer: &mut impl Write, bytes: &[u8]) -> Result<(), CliError> {
    writer.write_all(bytes).map_err(|_| CliError::OutputWrite)?;
    writer.flush().map_err(|_| CliError::OutputWrite)
}

fn write_report(report: &SanitizeReport, json: bool) -> Result<(), CliError> {
    let mut stdout = io::stdout().lock();
    write_report_to(&mut stdout, report, json)
}

fn write_report_to(
    writer: &mut impl Write,
    report: &SanitizeReport,
    json: bool,
) -> Result<(), CliError> {
    if json {
        serde_json::to_writer(&mut *writer, report).map_err(|_| CliError::ReportWrite)?;
        writer.write_all(b"\n").map_err(|_| CliError::ReportWrite)?;
    } else {
        let state = if report.changed {
            "changes found"
        } else {
            "clean"
        };
        writeln!(
            writer,
            "{state}; {} hidden scalars removable",
            report.total_removed()
        )
        .map_err(|_| CliError::ReportWrite)?;
    }
    writer.flush().map_err(|_| CliError::ReportWrite)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FlushFailingWriter;

    impl Write for FlushFailingWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Err(io::Error::other("synthetic flush failure"))
        }
    }

    #[test]
    fn output_flush_failures_are_never_reported_as_success() {
        assert_eq!(
            write_bytes_to(&mut FlushFailingWriter, b"private output"),
            Err(CliError::OutputWrite)
        );
        let report = sanitize("plain", Policy::PreserveAppearance).report;
        assert_eq!(
            write_report_to(&mut FlushFailingWriter, &report, true),
            Err(CliError::ReportWrite)
        );
        assert_eq!(
            write_report_to(&mut FlushFailingWriter, &report, false),
            Err(CliError::ReportWrite)
        );
    }
}
