//! Command-line definition (clap).
//!
//! There is deliberately no option for a password, recovery key or item secret: those come
//! from a TTY prompt or `--password-stdin` (see `secrets`).

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

/// Developer/test harness for the AryaVault Rust core.
#[derive(Debug, Parser)]
#[command(name = "arya-vault", version, about, disable_help_subcommand = true)]
pub struct Cli {
    /// The vault directory (holds `header-*.bin` and `vault.db`).
    #[arg(long, short = 'd', global = true)]
    pub vault_dir: Option<PathBuf>,
    /// Machine-readable output (one JSON document on stdout).
    #[arg(long, global = true)]
    pub json: bool,
    /// Read every secret the command needs from stdin, one line each, master password first.
    #[arg(long, global = true)]
    pub password_stdin: bool,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Create or inspect vaults.
    #[command(subcommand)]
    Vault(VaultCmd),
    /// Check the master password (opens the database too, when there is one).
    UnlockCheck,
    /// Item operations.
    #[command(subcommand)]
    Item(ItemCmd),
    /// Full-text search (titles, usernames, URLs, tags, notes; never secrets).
    Search(SearchArgs),
    /// Generate a password or passphrase.
    #[command(subcommand)]
    Gen(GenCmd),
    /// Master password operations.
    #[command(subcommand)]
    Password(PasswordCmd),
    /// Reset the master password using the recovery key.
    Recover(RecoverArgs),
    /// Replace the recovery key (needs the master password).
    RotateRecoveryKey(RevealArgs),
    /// Export the vault.
    Export(ExportArgs),
    /// Import into the vault.
    Import(ImportArgs),
    /// Benchmarks.
    #[command(subcommand)]
    Bench(BenchCmd),
    /// Format versions, SQLCipher settings and schema version.
    Info(InfoArgs),
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum KdfProfile {
    /// The floor: 64 MiB, t = 3, p = 1 (fast; for tests).
    Low,
    /// Calibrated for about 0.75 s on this machine (memory capped at 256 MiB).
    Default,
    /// Calibrated for about 1.5 s on this machine (memory capped at 512 MiB).
    High,
}

#[derive(Debug, Args)]
pub struct RevealArgs {
    /// Print the recovery key (it is shown exactly once and is required for this command).
    #[arg(long)]
    pub reveal: bool,
    /// Argon2 profile for the re-wrapped password.
    #[arg(long, value_enum, default_value_t = KdfProfile::Default)]
    pub kdf_profile: KdfProfile,
}

#[derive(Debug, Subcommand)]
pub enum VaultCmd {
    /// Create a new vault. Prints the recovery key once (requires `--reveal`).
    Create(CreateArgs),
}

#[derive(Debug, Args)]
pub struct CreateArgs {
    /// Argon2 profile (calibrated unless `low`).
    #[arg(long, value_enum, default_value_t = KdfProfile::Default)]
    pub kdf_profile: KdfProfile,
    /// Print the recovery key (it is shown exactly once and is required for this command).
    #[arg(long)]
    pub reveal: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum TypeArg {
    Login,
    Note,
    Card,
    Identity,
}

#[derive(Debug, Subcommand)]
pub enum ItemCmd {
    /// Add an item.
    Add(AddArgs),
    /// Show one item (secrets only with `--reveal`).
    Get(GetArgs),
    /// List items.
    List(ListArgs),
    /// Edit an item.
    Edit(EditArgs),
    /// Move an item to the trash.
    Delete(IdArg),
    /// Restore an item from the trash.
    Restore(IdArg),
    /// Permanently delete an item's content (leaves a tombstone).
    Purge(IdArg),
}

#[derive(Debug, Args)]
pub struct IdArg {
    /// Item id: 32 hex characters, or a unique prefix or suffix of at least 6.
    pub id: String,
}

#[derive(Debug, Args)]
pub struct AddArgs {
    /// Item type.
    #[arg(long = "type", value_enum, default_value_t = TypeArg::Login)]
    pub item_type: TypeArg,
    /// Title.
    #[arg(long)]
    pub title: String,
    /// A non-secret field, `name=value` (e.g. `username=alice`).
    #[arg(long = "field", value_name = "NAME=VALUE")]
    pub fields: Vec<String>,
    /// A secret field to read from the prompt/stdin (e.g. `password`), in the order given.
    #[arg(long = "set-secret", value_name = "NAME")]
    pub secrets: Vec<String>,
    /// A URL.
    #[arg(long = "url")]
    pub urls: Vec<String>,
    /// A tag.
    #[arg(long = "tag")]
    pub tags: Vec<String>,
    /// Mark as favorite.
    #[arg(long)]
    pub favorite: bool,
}

#[derive(Debug, Args)]
pub struct GetArgs {
    pub id: String,
    /// Print secret fields too.
    #[arg(long)]
    pub reveal: bool,
}

#[derive(Debug, Args)]
pub struct ListArgs {
    /// Only this type.
    #[arg(long = "type", value_enum)]
    pub item_type: Option<TypeArg>,
    /// Only items with this tag.
    #[arg(long)]
    pub tag: Option<String>,
    /// Only favorites.
    #[arg(long)]
    pub favorites: bool,
    /// List the trash instead.
    #[arg(long)]
    pub trash: bool,
}

#[derive(Debug, Args)]
pub struct EditArgs {
    pub id: String,
    /// New title.
    #[arg(long)]
    pub title: Option<String>,
    /// Set a non-secret field, `name=value`.
    #[arg(long = "field", value_name = "NAME=VALUE")]
    pub fields: Vec<String>,
    /// Set a secret field from the prompt/stdin, in the order given.
    #[arg(long = "set-secret", value_name = "NAME")]
    pub secrets: Vec<String>,
    /// Clear a field.
    #[arg(long = "clear", value_name = "NAME")]
    pub clear: Vec<String>,
    /// Add a tag.
    #[arg(long = "add-tag")]
    pub add_tags: Vec<String>,
    /// Remove a tag.
    #[arg(long = "remove-tag")]
    pub remove_tags: Vec<String>,
    /// Add a URL.
    #[arg(long = "add-url")]
    pub add_urls: Vec<String>,
    /// Toggle the favorite flag.
    #[arg(long)]
    pub toggle_favorite: bool,
}

#[derive(Debug, Args)]
pub struct SearchArgs {
    /// Words (each matched as a prefix; all must match).
    pub text: String,
    /// Only this type.
    #[arg(long = "type", value_enum)]
    pub item_type: Option<TypeArg>,
    /// Only items with this tag.
    #[arg(long)]
    pub tag: Option<String>,
    /// Maximum results.
    #[arg(long, default_value_t = 100)]
    pub limit: usize,
}

#[derive(Debug, Subcommand)]
pub enum GenCmd {
    /// A random password.
    Password(GenPasswordArgs),
    /// A random passphrase (EFF large wordlist).
    Passphrase(GenPassphraseArgs),
}

#[derive(Debug, Args)]
pub struct GenPasswordArgs {
    #[arg(long, default_value_t = 20)]
    pub length: usize,
    #[arg(long)]
    pub no_lower: bool,
    #[arg(long)]
    pub no_upper: bool,
    #[arg(long)]
    pub no_digits: bool,
    #[arg(long)]
    pub no_symbols: bool,
    #[arg(long)]
    pub exclude_ambiguous: bool,
    /// Print the generated secret (required).
    #[arg(long)]
    pub reveal: bool,
}

#[derive(Debug, Args)]
pub struct GenPassphraseArgs {
    #[arg(long, default_value_t = 6)]
    pub words: usize,
    #[arg(long, default_value = "-")]
    pub separator: String,
    #[arg(long)]
    pub capitalize: bool,
    #[arg(long)]
    pub number: bool,
    /// Print the generated secret (required).
    #[arg(long)]
    pub reveal: bool,
}

#[derive(Debug, Subcommand)]
pub enum PasswordCmd {
    /// Change the master password without the recovery key (reads: current, then new).
    Change {
        /// Argon2 profile for the new password.
        #[arg(long, value_enum, default_value_t = KdfProfile::Default)]
        kdf_profile: KdfProfile,
    },
}

#[derive(Debug, Args)]
pub struct RecoverArgs {
    /// Argon2 profile for the new password.
    #[arg(long, value_enum, default_value_t = KdfProfile::Default)]
    pub kdf_profile: KdfProfile,
    /// Also replace the recovery key and print the new one (requires `--reveal`).
    #[arg(long)]
    pub regenerate_recovery_key: bool,
    /// Allow printing the regenerated recovery key.
    #[arg(long)]
    pub reveal: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ExportFormat {
    /// Password-protected AryaVault export (docs/13).
    Aryavault,
    /// Plaintext CSV (needs `--acknowledge-plaintext-risk`).
    Csv,
}

#[derive(Debug, Args)]
pub struct ExportArgs {
    #[arg(long, value_enum, default_value_t = ExportFormat::Aryavault)]
    pub format: ExportFormat,
    /// Output file (must not exist).
    #[arg(long)]
    pub out: PathBuf,
    /// Include field history (aryavault only).
    #[arg(long)]
    pub include_history: bool,
    /// Argon2 profile for the export password (aryavault only; reads the export password
    /// after the master password).
    #[arg(long, value_enum, default_value_t = KdfProfile::Default)]
    pub kdf_profile: KdfProfile,
    /// Confirm that the CSV will contain every password in clear text.
    #[arg(long)]
    pub acknowledge_plaintext_risk: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ImportFormat {
    Csv,
    Bitwarden,
    Aryavault,
}

#[derive(Debug, Args)]
pub struct ImportArgs {
    #[arg(long, value_enum)]
    pub format: ImportFormat,
    /// Input file.
    #[arg(long)]
    pub file: PathBuf,
    /// Report what would happen without changing the vault.
    #[arg(long)]
    pub dry_run: bool,
    /// Import logins even if an identical one exists.
    #[arg(long)]
    pub keep_duplicates: bool,
}

#[derive(Debug, Subcommand)]
pub enum BenchCmd {
    /// Argon2 calibration and unlock timing.
    Kdf(BenchKdfArgs),
    /// Cold unlock, item writes and FTS search on a generated scratch vault.
    Search(BenchSearchArgs),
}

#[derive(Debug, Args)]
pub struct BenchKdfArgs {
    /// Target time per derivation in milliseconds.
    #[arg(long, default_value_t = 750)]
    pub target_ms: u64,
    /// Memory ceiling for calibration, MiB.
    #[arg(long, default_value_t = 256)]
    pub max_m_mib: u32,
    /// Timed derivations with the chosen parameters.
    #[arg(long, default_value_t = 3)]
    pub runs: usize,
}

#[derive(Debug, Args)]
pub struct BenchSearchArgs {
    /// Items to generate.
    #[arg(long, default_value_t = 20_000)]
    pub items: usize,
    /// Search queries to time.
    #[arg(long, default_value_t = 200)]
    pub queries: usize,
    /// Argon2 profile of the scratch vault (cold-unlock includes it).
    #[arg(long, value_enum, default_value_t = KdfProfile::Default)]
    pub kdf_profile: KdfProfile,
}

#[derive(Debug, Args)]
pub struct InfoArgs {
    /// Do not ask for the password; show header information only.
    #[arg(long)]
    pub no_unlock: bool,
}
