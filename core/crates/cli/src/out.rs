//! Output: human text or `--json`. Secrets are passed to these helpers only by callers that
//! hold a `--reveal` flag.

use std::io::{self, Write};

use serde_json::Value;

use crate::error::Result;

/// Output mode.
#[derive(Debug, Clone, Copy)]
pub struct Out {
    pub json: bool,
}

impl Out {
    /// Writes either `human` or the JSON `value`, plus a newline, to stdout.
    pub fn emit(&self, human: &str, value: &Value) -> Result<()> {
        let mut o = io::stdout().lock();
        if self.json {
            writeln!(o, "{value}")?;
        } else {
            writeln!(o, "{human}")?;
        }
        Ok(())
    }

    /// Writes `human` lines (text mode) or `value` (JSON mode).
    pub fn lines(&self, lines: &[String], value: &Value) -> Result<()> {
        let mut o = io::stdout().lock();
        if self.json {
            writeln!(o, "{value}")?;
        } else {
            for l in lines {
                writeln!(o, "{l}")?;
            }
        }
        Ok(())
    }
}

pub fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

pub fn unhex(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 || !s.is_ascii() {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}
