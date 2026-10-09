//! The AryaVault encrypted export (docs/13-export-format.md).
//!
//! ```text
//! file    = "AVEX" ‖ canonical-CBOR map
//! map     = { "format_version": 1, "kdf": {alg,version,m_kib,t,p,salt}, "export_id": bstr16,
//!             "nonce": bstr24, "ct": bstr }
//! key     = HKDF-SHA256(salt = export_id, ikm = Argon2id(NFKD(password), kdf), info = "aryavault/kek-pw/v1")
//! ct      = XChaCha20-Poly1305(key, nonce, aad, ISO-7816-4-pad(payload)) ‖ tag
//! aad     = "aryavault/export/v1" ‖ canonical-CBOR({format_version, kdf, export_id})
//! payload = canonical CBOR {format, include_history, folders[], items[]}
//! ```
//! Only T01 primitives are used (Argon2id with its floors/ceilings, the `kek_pw` HKDF,
//! XChaCha20-Poly1305 with a fresh random nonce, the 1 KiB padding); there is no new
//! construction. See the Spec questions in the PR about reusing `kek_pw`.

use arya_vault_crypto::aead::{self, AeadError, NONCE_LEN};
use arya_vault_crypto::format::cbor::{self, Limits, Value};
use arya_vault_crypto::format::padding;
use arya_vault_crypto::hkdf::kek_pw;
use arya_vault_crypto::kdf::{KDF_ALG, KDF_VERSION, KdfParams, SALT_LEN, derive_master_key};
use arya_vault_crypto::keys::Kek;
use arya_vault_crypto::rng::Rng;
use zeroize::{Zeroize, Zeroizing};

use super::{
    ImportBundle, ImportCustom, ImportError, ImportHistory, ImportItem, ImportLimits, ImportSource,
    SkipReason, WarningKind, check_file_size,
};
use crate::model::{CustomKind, ItemType, StdField};

/// File magic.
pub const MAGIC: &[u8; 4] = b"AVEX";
/// The only export format version this build writes and reads.
pub const FORMAT_VERSION: u16 = 1;
const AAD_LABEL: &[u8] = b"aryavault/export/v1";
const FORMAT_NAME: &str = "aryavault-export";

/// What can be learned from an export file without the password (and without running Argon2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerInfo {
    /// Format version.
    pub format_version: u16,
    /// Argon2id memory cost the importing device must be able to allocate, in KiB.
    pub m_kib: u32,
    /// Argon2id passes.
    pub t: u32,
    /// Argon2id parallelism.
    pub p: u32,
    /// Size of the encrypted payload in bytes.
    pub ciphertext_len: usize,
}

pub(crate) struct Container {
    pub kdf: KdfParams,
    pub export_id: [u8; 16],
    pub nonce: [u8; NONCE_LEN],
    pub ct: Vec<u8>,
}

fn container_limits(limits: &ImportLimits) -> Limits {
    let mut l = Limits::new(
        usize::try_from(limits.max_file_bytes).unwrap_or(usize::MAX),
        16,
        64,
    );
    l.max_depth = 4;
    l
}

fn field<'a>(entries: &'a [(Value, Value)], name: &str) -> Option<&'a Value> {
    entries
        .iter()
        .find(|(k, _)| matches!(k, Value::Text(t) if t == name))
        .map(|(_, v)| v)
}

fn exact<'a>(v: &'a Value, names: &[&str]) -> Result<Vec<&'a Value>, ImportError> {
    let Value::Map(entries) = v else {
        return Err(ImportError::Malformed);
    };
    if entries.len() != names.len() {
        return Err(ImportError::Malformed);
    }
    names
        .iter()
        .map(|n| field(entries, n).ok_or(ImportError::Malformed))
        .collect()
}

fn uint(v: &Value) -> Result<u64, ImportError> {
    match v {
        Value::Uint(n) => Ok(*n),
        _ => Err(ImportError::Malformed),
    }
}

fn u32_of(v: &Value) -> Result<u32, ImportError> {
    u32::try_from(uint(v)?).map_err(|_| ImportError::Malformed)
}

fn bytes_n<const N: usize>(v: &Value) -> Result<[u8; N], ImportError> {
    match v {
        Value::Bytes(b) => b.as_slice().try_into().map_err(|_| ImportError::Malformed),
        _ => Err(ImportError::Malformed),
    }
}

fn kdf_from(v: &Value) -> Result<KdfParams, ImportError> {
    let f = exact(v, &["alg", "version", "m_kib", "t", "p", "salt"])?;
    if !matches!(f[0], Value::Text(a) if a == KDF_ALG) || uint(f[1])? != u64::from(KDF_VERSION) {
        return Err(ImportError::InvalidKdf);
    }
    let p = KdfParams {
        m_kib: u32_of(f[2])?,
        t: u32_of(f[3])?,
        p: u32_of(f[4])?,
        salt: bytes_n::<SALT_LEN>(f[5])?,
    };
    // Floors AND ceilings, before any hashing (SEC-C02, SEC-C11).
    p.validate().map_err(|_| ImportError::InvalidKdf)?;
    Ok(p)
}

/// Parses the container structure. Checks magic, the version (first, so a newer format is
/// reported as such), the KDF bounds and every length; does no cryptography.
pub(crate) fn parse_container(
    bytes: &[u8],
    limits: &ImportLimits,
) -> Result<Container, ImportError> {
    check_file_size(bytes, limits)?;
    let body = bytes.strip_prefix(MAGIC).ok_or(ImportError::BadMagic)?;
    let v = cbor::decode(body, &container_limits(limits)).map_err(|_| ImportError::Malformed)?;
    let Value::Map(entries) = &v else {
        return Err(ImportError::Malformed);
    };
    let ver = field(entries, "format_version").ok_or(ImportError::Malformed)?;
    let found = uint(ver)?;
    if found != u64::from(FORMAT_VERSION) {
        return Err(ImportError::UnsupportedFormat {
            found: u16::try_from(found).unwrap_or(u16::MAX),
            max_supported: FORMAT_VERSION,
        });
    }
    let f = exact(&v, &["format_version", "kdf", "export_id", "nonce", "ct"])?;
    let ct = match f[4] {
        Value::Bytes(b) => b.clone(),
        _ => return Err(ImportError::Malformed),
    };
    // Padded plaintext is a non-empty multiple of 1 KiB, plus the 16-byte tag.
    if ct.len() < padding::BUCKET + 16 || !(ct.len() - 16).is_multiple_of(padding::BUCKET) {
        return Err(ImportError::Malformed);
    }
    Ok(Container {
        kdf: kdf_from(f[1])?,
        export_id: bytes_n(f[2])?,
        nonce: bytes_n(f[3])?,
        ct,
    })
}

/// Reads the public header of an export (no password, no Argon2): lets a UI check that the
/// device can afford the KDF before asking for a password.
///
/// # Errors
/// The same structural errors as [`parse_aryavault`].
pub fn read_container_info(
    bytes: &[u8],
    limits: &ImportLimits,
) -> Result<ContainerInfo, ImportError> {
    let c = parse_container(bytes, limits)?;
    Ok(ContainerInfo {
        format_version: FORMAT_VERSION,
        m_kib: c.kdf.m_kib,
        t: c.kdf.t,
        p: c.kdf.p,
        ciphertext_len: c.ct.len(),
    })
}

fn kdf_value(k: &KdfParams) -> Value {
    Value::Map(vec![
        // Order is canonical (sorted by encoded key): p, t, alg, salt, m_kib, version.
        (Value::text("p"), Value::Uint(u64::from(k.p))),
        (Value::text("t"), Value::Uint(u64::from(k.t))),
        (Value::text("alg"), Value::text(KDF_ALG)),
        (Value::text("salt"), Value::Bytes(k.salt.to_vec())),
        (Value::text("m_kib"), Value::Uint(u64::from(k.m_kib))),
        (Value::text("version"), Value::Uint(u64::from(KDF_VERSION))),
    ])
}

/// The AEAD associated data.
pub(crate) fn aad_of(kdf: &KdfParams, export_id: &[u8; 16]) -> Result<Vec<u8>, ImportError> {
    let m = Value::map(vec![
        (
            Value::text("format_version"),
            Value::Uint(u64::from(FORMAT_VERSION)),
        ),
        (Value::text("kdf"), kdf_value(kdf)),
        (Value::text("export_id"), Value::Bytes(export_id.to_vec())),
    ])
    .and_then(|v| v.encode())
    .map_err(|_| ImportError::Crypto)?;
    let mut aad = AAD_LABEL.to_vec();
    aad.extend_from_slice(&m);
    Ok(aad)
}

/// Derives the container key. Runs Argon2id with the (already validated) parameters.
pub(crate) fn derive_key(
    password: &str,
    kdf: &KdfParams,
    export_id: &[u8; 16],
) -> Result<Kek, ImportError> {
    let mk = derive_master_key(password, kdf).map_err(|_| ImportError::InvalidKdf)?;
    kek_pw(&mk, export_id).map_err(|_| ImportError::Crypto)
}

/// Authenticates and decrypts the container, returning the unpadded payload CBOR.
pub(crate) fn open_container(c: &Container, kek: &Kek) -> Result<Zeroizing<Vec<u8>>, ImportError> {
    let aad = aad_of(&c.kdf, &c.export_id)?;
    let padded = aead::open(kek, &aad, &c.nonce, &c.ct).map_err(|e| match e {
        AeadError::AuthenticationFailed => ImportError::AuthenticationFailed,
        _ => ImportError::Malformed,
    })?;
    // Padding is validated only after authentication.
    let payload = padding::unpad(&padded).map_err(|_| ImportError::Malformed)?;
    Ok(Zeroizing::new(payload.to_vec()))
}

/// Serialises and encrypts `payload` (canonical CBOR) under `password`.
///
/// Draws, in this order from `rng`: the KDF salt (16 B), the export id (16 B) and, inside
/// the AEAD, the nonce (24 B).
pub(crate) fn seal_container(
    payload: &[u8],
    password: &str,
    cost: &KdfParams,
    rng: &mut dyn Rng,
) -> Result<Vec<u8>, ImportError> {
    if password.is_empty() {
        return Err(ImportError::Crypto);
    }
    let kdf = cost.with_fresh_salt(rng).map_err(|_| ImportError::Crypto)?;
    kdf.validate().map_err(|_| ImportError::InvalidKdf)?;
    let mut export_id = [0u8; 16];
    rng.fill_bytes(&mut export_id)
        .map_err(|_| ImportError::Crypto)?;
    let kek = derive_key(password, &kdf, &export_id)?;
    let padded = padding::pad(payload);
    let (nonce, ct) = aead::seal(&kek, &aad_of(&kdf, &export_id)?, &padded, rng)
        .map_err(|_| ImportError::Crypto)?;
    let body = Value::map(vec![
        (
            Value::text("format_version"),
            Value::Uint(u64::from(FORMAT_VERSION)),
        ),
        (Value::text("kdf"), kdf_value(&kdf)),
        (Value::text("export_id"), Value::Bytes(export_id.to_vec())),
        (Value::text("nonce"), Value::Bytes(nonce.to_vec())),
        (Value::text("ct"), Value::Bytes(ct)),
    ])
    .and_then(|v| v.encode())
    .map_err(|_| ImportError::Crypto)?;
    let mut out = MAGIC.to_vec();
    out.extend_from_slice(&body);
    Ok(out)
}

/// Parses, decrypts and converts an AryaVault export.
///
/// # Errors
/// Structural errors, [`ImportError::InvalidKdf`] for out-of-range parameters (checked before
/// hashing), [`ImportError::AuthenticationFailed`] for a wrong password or any modified byte.
pub fn parse_aryavault(
    bytes: &[u8],
    password: &str,
    limits: &ImportLimits,
) -> Result<ImportBundle, ImportError> {
    let c = parse_container(bytes, limits)?;
    let kek = derive_key(password, &c.kdf, &c.export_id)?;
    let payload = open_container(&c, &kek)?;
    payload_to_bundle(&payload, limits)
}

/// Converts a *decrypted* payload (canonical CBOR, docs/13 section 5) into a bundle, with all the
/// limits of [`parse_aryavault`]. Exposed for tooling and fuzzing: the payload is a parser of
/// untrusted structure even though it is only reachable after authentication (the file's author
/// chose the password).
///
/// # Errors
/// [`ImportError::Malformed`] and the limit errors.
#[doc(hidden)]
pub fn parse_aryavault_payload(
    payload: &[u8],
    limits: &ImportLimits,
) -> Result<ImportBundle, ImportError> {
    payload_to_bundle(payload, limits)
}

// ------------------------------------------------------------------------- payload write

/// A canonical CBOR fragment that wipes itself on drop (it may contain secrets).
pub(crate) type Frag = Zeroizing<Vec<u8>>;

fn head(out: &mut Vec<u8>, major: u8, n: u64) {
    let m = major << 5;
    if n < 24 {
        out.push(m | n as u8);
    } else if n <= 0xff {
        out.extend([m | 24, n as u8]);
    } else if n <= 0xffff {
        out.push(m | 25);
        out.extend((n as u16).to_be_bytes());
    } else if n <= 0xffff_ffff {
        out.push(m | 26);
        out.extend((n as u32).to_be_bytes());
    } else {
        out.push(m | 27);
        out.extend(n.to_be_bytes());
    }
}

pub(crate) fn w_text(s: &str) -> Frag {
    let mut o = Zeroizing::new(Vec::with_capacity(s.len() + 9));
    head(&mut o, 3, s.len() as u64);
    o.extend_from_slice(s.as_bytes());
    o
}

pub(crate) fn w_bytes(b: &[u8]) -> Frag {
    let mut o = Zeroizing::new(Vec::with_capacity(b.len() + 9));
    head(&mut o, 2, b.len() as u64);
    o.extend_from_slice(b);
    o
}

pub(crate) fn w_bool(b: bool) -> Frag {
    Zeroizing::new(vec![if b { 0xf5 } else { 0xf4 }])
}

pub(crate) fn w_null() -> Frag {
    Zeroizing::new(vec![0xf6])
}

pub(crate) fn w_array(items: Vec<Frag>) -> Frag {
    let mut o = Zeroizing::new(Vec::new());
    head(&mut o, 4, items.len() as u64);
    for i in &items {
        o.extend_from_slice(i);
    }
    o
}

/// A map with text keys, emitted in canonical order (sorted by encoded key).
pub(crate) fn w_map(mut entries: Vec<(&str, Frag)>) -> Frag {
    // Encoded text keys sort by (length, bytes) because the head carries the length.
    entries.sort_by(|a, b| (a.0.len(), a.0).cmp(&(b.0.len(), b.0)));
    let mut o = Zeroizing::new(Vec::new());
    head(&mut o, 5, entries.len() as u64);
    for (k, v) in &entries {
        head(&mut o, 3, k.len() as u64);
        o.extend_from_slice(k.as_bytes());
        o.extend_from_slice(v);
    }
    o
}

/// One item to export.
pub(crate) struct ExportItem {
    pub id: [u8; 16],
    pub item_type: ItemType,
    pub title: String,
    pub folder: Option<[u8; 16]>,
    pub favorite: bool,
    pub fields: Vec<(StdField, Zeroizing<String>)>,
    pub urls: Vec<String>,
    pub tags: Vec<String>,
    pub custom: Vec<ImportCustom>,
    /// Older versions of fields with a current value, oldest first (empty unless history is exported).
    pub history: Vec<ImportHistory>,
}

pub(crate) struct ExportFolder {
    pub id: [u8; 16],
    pub name: String,
    pub parent: Option<[u8; 16]>,
}

/// Builds the payload CBOR.
pub(crate) fn build_payload(
    items: &[ExportItem],
    folders: &[ExportFolder],
    include_history: bool,
) -> Frag {
    let folders = w_array(
        folders
            .iter()
            .map(|f| {
                w_map(vec![
                    ("id", w_bytes(&f.id)),
                    ("name", w_text(&f.name)),
                    ("parent", f.parent.map_or_else(w_null, |p| w_bytes(&p))),
                ])
            })
            .collect(),
    );
    let items = w_array(
        items
            .iter()
            .map(|it| {
                let mut m = vec![
                    ("id", w_bytes(&it.id)),
                    ("type", w_text(it.item_type.as_str())),
                    ("title", w_text(&it.title)),
                    ("folder", it.folder.map_or_else(w_null, |p| w_bytes(&p))),
                    ("favorite", w_bool(it.favorite)),
                    (
                        "fields",
                        w_map(
                            it.fields
                                .iter()
                                .map(|(k, v)| (k.key(), w_text(v)))
                                .collect(),
                        ),
                    ),
                    ("urls", w_array(it.urls.iter().map(|u| w_text(u)).collect())),
                    ("tags", w_array(it.tags.iter().map(|t| w_text(t)).collect())),
                    (
                        "custom",
                        w_array(
                            it.custom
                                .iter()
                                .map(|c| {
                                    w_map(vec![
                                        ("kind", w_text(c.kind.as_str())),
                                        ("label", w_text(&c.label)),
                                        ("value", w_text(&c.value)),
                                    ])
                                })
                                .collect(),
                        ),
                    ),
                ];
                if include_history {
                    m.push((
                        "history",
                        w_map(
                            it.history
                                .iter()
                                .map(|h| {
                                    (
                                        h.field.key(),
                                        w_array(h.older.iter().map(|v| w_text(v)).collect()),
                                    )
                                })
                                .collect(),
                        ),
                    ));
                }
                w_map(m)
            })
            .collect(),
    );
    w_map(vec![
        ("format", w_text(FORMAT_NAME)),
        ("include_history", w_bool(include_history)),
        ("folders", folders),
        ("items", items),
    ])
}

// -------------------------------------------------------------------------- payload read

/// Wipes the strings and byte strings of a decoded tree when dropped.
struct Wipe(Value);

fn wipe(v: &mut Value) {
    match v {
        Value::Text(s) => s.zeroize(),
        Value::Bytes(b) => b.zeroize(),
        Value::Array(a) => a.iter_mut().for_each(wipe),
        Value::Map(m) => m.iter_mut().for_each(|(k, v)| {
            wipe(k);
            wipe(v);
        }),
        _ => {}
    }
}

impl Drop for Wipe {
    fn drop(&mut self) {
        wipe(&mut self.0);
    }
}

fn text_of(v: &Value) -> Option<&str> {
    match v {
        Value::Text(s) => Some(s),
        _ => None,
    }
}

/// Converts decrypted payload CBOR into a bundle. Parses untrusted structure, so it is bounded
/// by `limits` and has a fuzz target of its own.
pub(crate) fn payload_to_bundle(
    payload: &[u8],
    limits: &ImportLimits,
) -> Result<ImportBundle, ImportError> {
    let mut l = Limits::new(
        limits.max_field_bytes,
        limits.max_records.max(1024),
        limits.max_json_values(),
    );
    l.max_depth = limits.max_depth;
    let root = Wipe(cbor::decode(payload, &l).map_err(|e| match e {
        cbor::CborError::TooDeep => ImportError::TooDeeplyNested,
        cbor::CborError::TooLong => ImportError::FieldTooLarge,
        cbor::CborError::TooManyItems => ImportError::TooManyRecords,
        _ => ImportError::Malformed,
    })?);
    let Value::Map(top) = &root.0 else {
        return Err(ImportError::Malformed);
    };
    if top.len() != 4
        || field(top, "format").and_then(text_of) != Some(FORMAT_NAME)
        || !matches!(field(top, "include_history"), Some(Value::Bool(_)))
    {
        return Err(ImportError::Malformed);
    }
    let (Some(Value::Array(folders)), Some(Value::Array(items))) =
        (field(top, "folders"), field(top, "items"))
    else {
        return Err(ImportError::Malformed);
    };
    if items.len() > limits.max_records {
        return Err(ImportError::TooManyRecords);
    }

    let mut bundle = ImportBundle::new(ImportSource::AryaVault);
    // Source folder index -> index in bundle.folders (None when the folder is unusable).
    let mut folder_ids: Vec<[u8; 16]> = Vec::new();
    let mut folder_map: Vec<Option<usize>> = Vec::new();
    for f in folders {
        let parts = exact(f, &["id", "name", "parent"])?;
        let id = bytes_n::<16>(parts[0])?;
        let parent = match parts[2] {
            Value::Null => None,
            v => Some(bytes_n::<16>(v)?),
        };
        folder_ids.push(id);
        let Some(name) = text_of(parts[1]).and_then(super::clean_folder_name) else {
            bundle.warn(0, WarningKind::InvalidFolder);
            folder_map.push(None);
            continue;
        };
        let parent_idx = match parent {
            None => None,
            Some(p) => {
                let found = folder_ids[..folder_ids.len() - 1]
                    .iter()
                    .position(|x| *x == p)
                    .and_then(|i| folder_map[i]);
                if found.is_none() {
                    bundle.warn(0, WarningKind::InvalidFolder);
                }
                found
            }
        };
        bundle.folders.push(super::ImportFolder {
            name,
            parent: parent_idx,
        });
        folder_map.push(Some(bundle.folders.len() - 1));
    }

    for (n, it) in items.iter().enumerate() {
        let rec = n + 1;
        match item_from(it, rec, &folder_ids, &folder_map, &mut bundle.warnings) {
            Ok(item) => bundle.push(item, limits)?,
            Err(reason) => bundle.skip(rec, reason, limits)?,
        }
    }
    Ok(bundle)
}

fn item_from(
    it: &Value,
    rec: usize,
    folder_ids: &[[u8; 16]],
    folder_map: &[Option<usize>],
    warnings: &mut Vec<super::ImportWarning>,
) -> Result<ImportItem, SkipReason> {
    let bad = SkipReason::Invalid("malformed item");
    let Value::Map(entries) = it else {
        return Err(bad);
    };
    let required = [
        "id", "type", "title", "folder", "favorite", "fields", "urls", "tags", "custom",
    ];
    let has_hist = field(entries, "history");
    if entries.len() != required.len() + usize::from(has_hist.is_some()) {
        return Err(bad);
    }
    let get = |k: &str| field(entries, k).ok_or(bad);
    let item_type = text_of(get("type")?)
        .and_then(ItemType::parse)
        .ok_or(SkipReason::Invalid("unknown item type"))?;
    let title = text_of(get("title")?).ok_or(bad)?;
    let mut item = ImportItem::new(item_type, title, rec);
    if let Value::Bool(b) = get("favorite")? {
        item.favorite = *b;
    } else {
        return Err(bad);
    }
    match get("folder")? {
        Value::Null => {}
        Value::Bytes(b) => match folder_ids.iter().position(|f| f.as_slice() == b.as_slice()) {
            Some(i) if folder_map[i].is_some() => item.folder = folder_map[i],
            _ => warnings.push(super::ImportWarning {
                record: rec,
                kind: WarningKind::InvalidFolder,
            }),
        },
        _ => return Err(bad),
    }
    let Value::Map(fields) = get("fields")? else {
        return Err(bad);
    };
    for (k, v) in fields {
        let f = text_of(k)
            .and_then(StdField::from_key)
            .ok_or(SkipReason::Invalid("unknown field"))?;
        let s = text_of(v).ok_or(bad)?;
        if f == StdField::Title {
            return Err(SkipReason::Invalid("title in fields"));
        }
        item.fields.push((f, Zeroizing::new(s.to_owned())));
    }
    let Value::Array(urls) = get("urls")? else {
        return Err(bad);
    };
    for u in urls {
        item.urls.push(text_of(u).ok_or(bad)?.to_owned());
    }
    let Value::Array(tags) = get("tags")? else {
        return Err(bad);
    };
    for t in tags {
        item.tags.push(text_of(t).ok_or(bad)?.to_owned());
    }
    let Value::Array(custom) = get("custom")? else {
        return Err(bad);
    };
    for c in custom {
        let Value::Map(cm) = c else { return Err(bad) };
        if cm.len() != 3 {
            return Err(bad);
        }
        let kind = field(cm, "kind")
            .and_then(text_of)
            .and_then(CustomKind::parse)
            .ok_or(bad)?;
        let label = field(cm, "label").and_then(text_of).ok_or(bad)?;
        let value = field(cm, "value").and_then(text_of).ok_or(bad)?;
        item.custom.push(ImportCustom {
            kind,
            label: label.to_owned(),
            value: Zeroizing::new(value.to_owned()),
        });
    }
    if let Some(Value::Map(h)) = has_hist {
        for (k, v) in h {
            let f = text_of(k)
                .and_then(StdField::from_key)
                .ok_or(SkipReason::Invalid("unknown history field"))?;
            let Value::Array(vs) = v else { return Err(bad) };
            let older = vs
                .iter()
                .map(|x| text_of(x).map(|s| Zeroizing::new(s.to_owned())).ok_or(bad))
                .collect::<Result<Vec<_>, _>>()?;
            item.history.push(ImportHistory { field: f, older });
        }
    } else if has_hist.is_some() {
        return Err(bad);
    }
    Ok(item)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use arya_vault_crypto::rng::{OsRng, RngError};

    /// A fixed byte stream (0, 1, 2, ...) so tests can reproduce exact bytes.
    pub(crate) struct CountingRng(pub u8);
    impl Rng for CountingRng {
        fn fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), RngError> {
            for b in dest {
                *b = self.0;
                self.0 = self.0.wrapping_add(1);
            }
            Ok(())
        }
    }

    fn lim() -> ImportLimits {
        ImportLimits::default()
    }

    fn sample_items() -> (Vec<ExportItem>, Vec<ExportFolder>) {
        let mut it = ExportItem {
            id: [1; 16],
            item_type: ItemType::Login,
            title: "Example".into(),
            folder: Some([9; 16]),
            favorite: true,
            fields: vec![
                (StdField::Username, Zeroizing::new("alice".into())),
                (StdField::Password, Zeroizing::new("CANARY-pw".into())),
            ],
            urls: vec!["https://a.example".into()],
            tags: vec!["t1".into()],
            custom: vec![ImportCustom {
                kind: CustomKind::Hidden,
                label: "pin".into(),
                value: Zeroizing::new("CANARY-1".into()),
            }],
            history: vec![ImportHistory {
                field: StdField::Password,
                older: vec![Zeroizing::new("CANARY-old".into())],
            }],
        };
        it.fields.sort_by_key(|(k, _)| *k);
        (
            vec![it],
            vec![ExportFolder {
                id: [9; 16],
                name: "Work".into(),
                parent: None,
            }],
        )
    }

    #[test]
    fn payload_writer_is_canonical_and_round_trips() {
        let (items, folders) = sample_items();
        let p = build_payload(&items, &folders, true);
        // The strict canonical decoder accepting it proves the writer emitted canonical CBOR,
        // and re-encoding gives the identical bytes.
        let mut l = Limits::new(1 << 20, 1024, 4096);
        l.max_depth = 16;
        let v = cbor::decode(&p, &l).unwrap();
        assert_eq!(v.encode().unwrap(), *p);
        let b = payload_to_bundle(&p, &lim()).unwrap();
        assert_eq!((b.items.len(), b.folders.len(), b.skipped.len()), (1, 1, 0));
        let it = &b.items[0];
        assert_eq!(it.title, "Example");
        assert_eq!(it.folder, Some(0));
        assert!(it.favorite);
        assert_eq!(it.history[0].older[0].as_str(), "CANARY-old");
        assert_eq!(it.custom[0].kind, CustomKind::Hidden);
    }

    #[test]
    fn history_is_omitted_when_not_requested() {
        let (items, folders) = sample_items();
        let p = build_payload(&items, &folders, false);
        let b = payload_to_bundle(&p, &lim()).unwrap();
        assert!(b.items[0].history.is_empty());
        let text = String::from_utf8_lossy(&p).into_owned();
        assert!(!text.contains("CANARY-old"));
    }

    #[test]
    fn payload_structure_errors() {
        for bad in [
            &b""[..],
            b"\x80",
            b"\xa0",
            b"\xa4\x66format\x61x\x6finclude_history\xf4\x67folders\x80\x65items\x80",
        ] {
            assert!(payload_to_bundle(bad, &lim()).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn bad_items_are_skipped_not_fatal() {
        let (mut items, folders) = sample_items();
        items.push(ExportItem {
            id: [2; 16],
            item_type: ItemType::Note,
            title: "N".into(),
            folder: None,
            favorite: false,
            fields: vec![],
            urls: vec![],
            tags: vec![],
            custom: vec![],
            history: vec![],
        });
        let mut p = build_payload(&items, &folders, false).to_vec();
        // Corrupt the first item's type string ("login" -> "lugin") without changing lengths.
        let pos = p.windows(5).position(|w| w == b"login").unwrap();
        p[pos + 1] = b'u';
        let b = payload_to_bundle(&p, &lim()).unwrap();
        assert_eq!(b.items.len(), 1);
        assert_eq!(b.skipped.len(), 1);
    }

    #[test]
    fn container_round_trip_and_wrong_password() {
        let (items, folders) = sample_items();
        let payload = build_payload(&items, &folders, true);
        let file = seal_container(
            &payload,
            "CANARY-export-pw",
            &KdfParams::floor([0; 16]),
            &mut OsRng,
        )
        .unwrap();
        assert!(file.starts_with(b"AVEX"));
        let b = parse_aryavault(&file, "CANARY-export-pw", &lim()).unwrap();
        assert_eq!(b.items[0].title, "Example");
        assert!(matches!(
            parse_aryavault(&file, "CANARY-wrong", &lim()),
            Err(ImportError::AuthenticationFailed)
        ));
        let info = read_container_info(&file, &lim()).unwrap();
        assert_eq!(
            (info.format_version, info.m_kib, info.t, info.p),
            (1, 65_536, 3, 1)
        );
    }

    #[test]
    fn exports_are_randomised_and_padded() {
        let payload = build_payload(&[], &[], false);
        let a = seal_container(
            &payload,
            "CANARY-pw",
            &KdfParams::floor([0; 16]),
            &mut OsRng,
        )
        .unwrap();
        let b = seal_container(
            &payload,
            "CANARY-pw",
            &KdfParams::floor([0; 16]),
            &mut OsRng,
        )
        .unwrap();
        assert_ne!(a, b);
        let c = parse_container(&a, &lim()).unwrap();
        assert_eq!((c.ct.len() - 16) % 1024, 0);
    }

    // SEC-Y05 / SEC-C05: every byte of the file is covered by parsing or authentication.
    // The key is derived once (Argon2 is slow); a flip that changes the KDF parameters or the
    // export id changes the AAD, so it must fail with the original key as well.
    #[test]
    fn every_byte_flip_is_rejected_without_rerunning_argon2() {
        let payload = build_payload(&sample_items().0, &sample_items().1, true);
        let file = seal_container(
            &payload,
            "CANARY-pw",
            &KdfParams::floor([0; 16]),
            &mut CountingRng(0),
        )
        .unwrap();
        let orig = parse_container(&file, &lim()).unwrap();
        let kek = derive_key("CANARY-pw", &orig.kdf, &orig.export_id).unwrap();
        assert!(open_container(&orig, &kek).is_ok());
        for i in 0..file.len() {
            for bit in [0x01u8, 0x80] {
                let mut m = file.clone();
                m[i] ^= bit;
                match parse_container(&m, &lim()) {
                    Err(_) => {}
                    Ok(c) => assert!(
                        open_container(&c, &kek).is_err(),
                        "flip at byte {i} (bit {bit:#x}) was accepted"
                    ),
                }
            }
        }
        // Truncation at every length and trailing garbage.
        for n in 0..file.len() {
            let r = parse_container(&file[..n], &lim()).and_then(|c| open_container(&c, &kek));
            assert!(r.is_err(), "truncation to {n} bytes accepted");
        }
        let mut longer = file.clone();
        longer.push(0);
        assert!(parse_container(&longer, &lim()).is_err());
    }

    #[test]
    fn version_magic_and_kdf_bounds() {
        let payload = build_payload(&[], &[], false);
        let file = seal_container(
            &payload,
            "CANARY-pw",
            &KdfParams::floor([0; 16]),
            &mut CountingRng(0),
        )
        .unwrap();
        assert!(matches!(
            parse_container(b"XXXX", &lim()),
            Err(ImportError::BadMagic)
        ));
        assert!(matches!(
            parse_container(b"", &lim()),
            Err(ImportError::BadMagic)
        ));
        // Rebuild the container map with a different version / hostile KDF values.
        let v = cbor::decode(&file[4..], &container_limits(&lim())).unwrap();
        let Value::Map(entries) = v else { panic!() };
        type Entries = [(Value, Value)];
        let rebuild = |edit: &dyn Fn(&mut Entries)| {
            let mut e = entries.clone();
            edit(&mut e);
            let mut out = MAGIC.to_vec();
            out.extend(Value::map(e).unwrap().encode().unwrap());
            out
        };
        for v in [0u64, 2, 99, 70_000] {
            let f = rebuild(&|e| {
                e.iter_mut().for_each(|(k, val)| {
                    if matches!(k, Value::Text(t) if t == "format_version") {
                        *val = Value::Uint(v)
                    }
                })
            });
            assert!(
                matches!(
                    parse_container(&f, &lim()),
                    Err(ImportError::UnsupportedFormat {
                        max_supported: 1,
                        ..
                    })
                ),
                "{v}"
            );
        }
        let with_kdf = |m: u64, t: u64, p: u64| {
            rebuild(&|e| {
                for (k, val) in e.iter_mut() {
                    if matches!(k, Value::Text(x) if x == "kdf") {
                        let Value::Map(km) = val else { panic!() };
                        for (kk, vv) in km.iter_mut() {
                            match kk {
                                Value::Text(n) if n == "m_kib" => *vv = Value::Uint(m),
                                Value::Text(n) if n == "t" => *vv = Value::Uint(t),
                                Value::Text(n) if n == "p" => *vv = Value::Uint(p),
                                _ => {}
                            }
                        }
                    }
                }
            })
        };
        assert!(parse_container(&with_kdf(65_536, 3, 1), &lim()).is_ok());
        for (m, t, p) in [
            (1 << 30, 3, 1),
            (65_535, 3, 1),
            (1_048_577, 3, 1),
            (65_536, 2, 1),
            (65_536, 11, 1),
            (65_536, 3, 0),
            (65_536, 3, 9),
        ] {
            // Rejected at parse time, i.e. before any Argon2 run (this test never derives a key).
            assert!(
                matches!(
                    parse_container(&with_kdf(m, t, p), &lim()),
                    Err(ImportError::InvalidKdf)
                ),
                "{m},{t},{p}"
            );
            assert!(matches!(
                parse_aryavault(&with_kdf(m, t, p), "x", &lim()),
                Err(ImportError::InvalidKdf)
            ));
        }
    }

    #[test]
    fn size_limit_applies_before_parsing() {
        let l = ImportLimits {
            max_file_bytes: 16,
            ..lim()
        };
        assert!(matches!(
            parse_container(&[b'A'; 17], &l),
            Err(ImportError::FileTooLarge)
        ));
    }

    #[test]
    fn empty_password_is_refused_on_export() {
        assert!(seal_container(b"x", "", &KdfParams::floor([0; 16]), &mut OsRng).is_err());
    }
}
