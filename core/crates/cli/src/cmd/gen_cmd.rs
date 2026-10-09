//! `gen password|passphrase`.

use arya_vault_generator::{
    OsRandom, PassphraseOptions, PasswordOptions, entropy_bits, entropy_bits_passphrase,
    generate_passphrase, generate_password,
};
use serde_json::json;

use super::Ctx;
use crate::args::GenCmd;
use crate::error::{CliError, Result};

fn need_reveal(reveal: bool) -> Result<()> {
    if reveal {
        Ok(())
    } else {
        Err(CliError::usage(
            "generated secrets are printed only with --reveal",
        ))
    }
}

pub fn run(ctx: &Ctx, cmd: GenCmd) -> Result<()> {
    match cmd {
        GenCmd::Password(a) => {
            need_reveal(a.reveal)?;
            let opts = PasswordOptions {
                length: a.length,
                lower: !a.no_lower,
                upper: !a.no_upper,
                digits: !a.no_digits,
                symbols: !a.no_symbols,
                exclude_ambiguous: a.exclude_ambiguous,
                ..PasswordOptions::default()
            };
            let bits = entropy_bits(&opts).map_err(|e| CliError::usage(e.to_string()))?;
            let pw = generate_password(&opts, &mut OsRandom)?;
            ctx.out.emit(
                pw.as_str(),
                &json!({ "value": pw.as_str(), "entropy_bits": bits }),
            )
        }
        GenCmd::Passphrase(a) => {
            need_reveal(a.reveal)?;
            let opts = PassphraseOptions {
                word_count: a.words,
                separator: a.separator,
                capitalize: a.capitalize,
                number_suffix: a.number,
            };
            let bits =
                entropy_bits_passphrase(&opts).map_err(|e| CliError::usage(e.to_string()))?;
            let p = generate_passphrase(&opts, &mut OsRandom)?;
            ctx.out.emit(
                p.as_str(),
                &json!({ "value": p.as_str(), "entropy_bits": bits }),
            )
        }
    }
}
