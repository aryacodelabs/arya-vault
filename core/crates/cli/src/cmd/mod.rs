//! Command implementations.

mod bench;
mod gen_cmd;
mod item;
mod transfer;
mod vault_cmd;

use std::path::PathBuf;

use crate::args::{Cli, Command, ItemCmd};
use crate::error::{CliError, Result};
use crate::layout::VaultDir;
use crate::out::Out;
use crate::secrets::Secrets;

/// What every command needs.
pub struct Ctx {
    pub dir: Option<PathBuf>,
    pub out: Out,
    pub secrets: Secrets,
}

impl Ctx {
    pub fn vault_dir(&self) -> Result<VaultDir> {
        self.dir
            .clone()
            .map(VaultDir::new)
            .ok_or_else(|| CliError::usage("this command needs --vault-dir <DIR>"))
    }

    pub fn master_password(&self) -> Result<zeroize::Zeroizing<String>> {
        self.secrets.read("Master password")
    }
}

pub fn run(cli: Cli) -> Result<()> {
    let ctx = Ctx {
        dir: cli.vault_dir,
        out: Out { json: cli.json },
        secrets: Secrets::new(cli.password_stdin),
    };
    match cli.command {
        Command::Vault(c) => vault_cmd::vault(&ctx, c),
        Command::UnlockCheck => vault_cmd::unlock_check(&ctx),
        Command::Item(c) => match c {
            ItemCmd::Add(a) => item::add(&ctx, a),
            ItemCmd::Get(a) => item::get(&ctx, a),
            ItemCmd::List(a) => item::list(&ctx, a),
            ItemCmd::Edit(a) => item::edit(&ctx, a),
            ItemCmd::Delete(a) => item::delete(&ctx, a),
            ItemCmd::Restore(a) => item::restore(&ctx, a),
            ItemCmd::Purge(a) => item::purge(&ctx, a),
        },
        Command::Search(a) => item::search(&ctx, a),
        Command::Gen(c) => gen_cmd::run(&ctx, c),
        Command::Password(c) => vault_cmd::password(&ctx, c),
        Command::Recover(a) => vault_cmd::recover(&ctx, a),
        Command::RotateRecoveryKey(a) => vault_cmd::rotate_recovery_key(&ctx, a),
        Command::Export(a) => transfer::export(&ctx, a),
        Command::Import(a) => transfer::import(&ctx, a),
        Command::Bench(c) => bench::run(&ctx, c),
        Command::Info(a) => vault_cmd::info(&ctx, a),
    }
}
