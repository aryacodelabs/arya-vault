//! Reading secrets: a no-echo TTY prompt or explicit `--password-stdin`.
//!
//! Secrets are never taken from arguments or environment variables. With `--password-stdin`
//! every secret the command needs is read as one line from stdin, in the order the command
//! documents (master password first). Without it the prompt reads from the controlling
//! terminal with echo off, and fails unless stdin is a terminal (it never falls back to stdin).

use std::io::{self, BufRead, IsTerminal, Write};

use zeroize::Zeroizing;

use crate::error::{CliError, Result};

/// Where secrets come from.
#[derive(Debug, Clone, Copy)]
pub struct Secrets {
    from_stdin: bool,
}

impl Secrets {
    pub fn new(from_stdin: bool) -> Self {
        Self { from_stdin }
    }

    /// Reads one secret. `prompt` is shown on stderr in TTY mode only.
    pub fn read(&self, prompt: &str) -> Result<Zeroizing<String>> {
        if self.from_stdin {
            let mut line = Zeroizing::new(String::new());
            let n = io::stdin().lock().read_line(&mut line)?;
            if n == 0 {
                return Err(CliError::usage(
                    "no secret on stdin (expected one line per secret the command needs)",
                ));
            }
            // Strip only the line terminator: spaces are part of a password.
            while line.ends_with('\n') || line.ends_with('\r') {
                line.pop();
            }
            return Ok(line);
        }
        // The no-echo prompt is only for an interactive terminal. With a pipe or file on stdin
        // some platforms' console APIs block forever (observed on Windows CI) or read from an
        // unexpected place, so refuse instead of guessing: scripts must say --password-stdin.
        if !io::stdin().is_terminal() {
            return Err(CliError::usage(
                "cannot prompt for a secret: stdin is not a terminal; use --password-stdin",
            ));
        }
        let mut err = io::stderr();
        write!(err, "{prompt}: ")?;
        err.flush()?;
        rpassword::read_password()
            .map(Zeroizing::new)
            .map_err(|_| CliError::usage("cannot read a secret: no terminal; use --password-stdin"))
    }

    /// Reads a new secret; asks twice and compares in TTY mode.
    pub fn read_new(&self, prompt: &str) -> Result<Zeroizing<String>> {
        let first = self.read(prompt)?;
        if !self.from_stdin {
            let again = self.read(&format!("{prompt} (again)"))?;
            if *first != *again {
                return Err(CliError::usage("the two entries do not match"));
            }
        }
        Ok(first)
    }
}
