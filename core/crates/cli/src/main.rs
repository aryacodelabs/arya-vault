//! Developer/test harness over the AryaVault Rust core (headless vault operations).
//!
//! Secrets are read from a no-echo TTY prompt or `--password-stdin`, never from arguments or
//! environment variables, and are printed only with `--reveal`. This binary calls public APIs
//! of the existing crates only: it implements no cryptography of its own.

mod args;
mod cmd;
mod error;
mod layout;
mod out;
mod secrets;

use std::io::Write;
use std::process::ExitCode;

use clap::Parser;
use clap::error::ErrorKind;

use args::Cli;
use error::{CliError, Exit};

/// Replaces the default panic message (which can quote values) with a generic one.
fn install_panic_hook() {
    std::panic::set_hook(Box::new(|_| {
        let _ = writeln!(
            std::io::stderr(),
            "internal error: unexpected condition; exiting (details withheld to avoid leaking secrets)"
        );
    }));
}

fn report(json: bool, e: &CliError) {
    let mut err = std::io::stderr();
    if json {
        let v = serde_json::json!({ "error": { "code": e.code, "message": e.message } });
        let _ = writeln!(err, "{v}");
    } else {
        let _ = writeln!(err, "error: {}", e.message);
    }
}

fn main() -> ExitCode {
    install_panic_hook();
    let cli = match Cli::try_parse() {
        Ok(c) => c,
        Err(e) => {
            return match e.kind() {
                ErrorKind::DisplayHelp
                | ErrorKind::DisplayVersion
                | ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand => {
                    let _ = e.print();
                    if e.use_stderr() {
                        ExitCode::from(Exit::Usage as u8)
                    } else {
                        ExitCode::SUCCESS
                    }
                }
                kind => {
                    // clap's own message can quote the offending argument text. A mistyped
                    // `--password hunter2` must not echo `hunter2`, so only the kind is shown.
                    let _ = writeln!(
                        std::io::stderr(),
                        "error: invalid command line ({kind:?}); run with --help. \
                         Secrets are never accepted as arguments: use the prompt or --password-stdin."
                    );
                    ExitCode::from(Exit::Usage as u8)
                }
            };
        }
    };
    let json = cli.json;
    match cmd::run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            report(json, &e);
            ExitCode::from(e.exit as u8)
        }
    }
}
