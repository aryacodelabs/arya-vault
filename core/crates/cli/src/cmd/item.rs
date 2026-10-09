//! `item add|get|list|edit|delete|restore|purge` and `search`.

use arya_vault_storage::Id;
use arya_vault_vault::{
    ItemSummary, ItemType, ListFilter, NewItem, Page, SearchQuery, StdField, Vault,
};
use serde_json::{Value, json};

use super::Ctx;
use crate::args::{AddArgs, EditArgs, GetArgs, IdArg, ListArgs, SearchArgs, TypeArg};
use crate::error::{CliError, Result};
use crate::layout::Unlocked;
use crate::out::{hex, unhex};

fn item_type(t: TypeArg) -> ItemType {
    match t {
        TypeArg::Login => ItemType::Login,
        TypeArg::Note => ItemType::Note,
        TypeArg::Card => ItemType::Card,
        TypeArg::Identity => ItemType::Identity,
    }
}

fn open(ctx: &Ctx) -> Result<Unlocked> {
    let dir = ctx.vault_dir()?;
    let pw = ctx.master_password()?;
    dir.unlock(&pw)
}

fn field_by_name(name: &str) -> Result<StdField> {
    StdField::from_key(name).ok_or_else(|| CliError::usage(format!("unknown field `{name}`")))
}

/// A non-secret field given as `name=value`. Secret fields must use `--set-secret`.
fn parse_field(spec: &str) -> Result<(StdField, &str)> {
    let (name, value) = spec
        .split_once('=')
        .ok_or_else(|| CliError::usage("expected --field NAME=VALUE"))?;
    let field = field_by_name(name)?;
    if field.is_secret() {
        return Err(CliError::usage(format!(
            "`{name}` is a secret field: use --set-secret {name} (read from the prompt or stdin)"
        )));
    }
    Ok((field, value))
}

fn secret_field(name: &str) -> Result<StdField> {
    let field = field_by_name(name)?;
    if !field.is_secret() {
        return Err(CliError::usage(format!(
            "`{name}` is not a secret field: use --field {name}=VALUE"
        )));
    }
    Ok(field)
}

fn summary_json(s: &ItemSummary) -> Value {
    json!({
        "id": hex(&s.id),
        "type": s.item_type.as_str(),
        "title": s.title,
        "favorite": s.favorite,
        "folder_id": s.folder_id.map(|f| hex(&f)),
    })
}

fn summary_line(s: &ItemSummary) -> String {
    format!(
        "{}  {:<8} {}{}",
        hex(&s.id),
        s.item_type.as_str(),
        s.title,
        if s.favorite { "  *" } else { "" }
    )
}

/// Resolves a full id, or a unique prefix or suffix of at least 6 hex characters, among live
/// items and the trash. Ids are UUIDv7 (time-ordered), so items created close together share
/// their leading characters while the trailing ones are random: both ends are accepted.
fn resolve(vault: &mut Vault, text: &str) -> Result<Id> {
    let text = text.to_ascii_lowercase();
    if text.len() == 32 {
        if let Some(b) = unhex(&text) {
            if let Ok(id) = Id::try_from(b) {
                return Ok(id);
            }
        }
    }
    if text.len() < 6 || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(CliError::usage(
            "an item id is 32 hex characters, or a unique prefix or suffix of at least 6",
        ));
    }
    let mut matches: Vec<Id> = vault
        .list(&ListFilter::default(), Page::ALL)?
        .into_iter()
        .map(|s| s.id)
        .chain(vault.list_trash()?.into_iter().map(|t| t.summary.id))
        .filter(|id| {
            let h = hex(id);
            h.starts_with(&text) || h.ends_with(&text)
        })
        .collect();
    matches.sort_unstable();
    matches.dedup();
    match matches.as_slice() {
        [id] => Ok(*id),
        [] => Err(CliError::failure("no such item")),
        _ => Err(CliError::usage("that id prefix is ambiguous")),
    }
}

pub fn add(ctx: &Ctx, a: AddArgs) -> Result<()> {
    // Validate the specs before asking for any secret.
    let fields: Vec<(StdField, &str)> = a
        .fields
        .iter()
        .map(|s| parse_field(s))
        .collect::<Result<_>>()?;
    let secret_names: Vec<StdField> = a
        .secrets
        .iter()
        .map(|s| secret_field(s))
        .collect::<Result<_>>()?;
    let mut u = open(ctx)?;
    let mut new = NewItem::new(item_type(a.item_type), &a.title);
    for (f, v) in fields {
        new = new.with_field(f, v);
    }
    for f in secret_names {
        let v = ctx.secrets.read(&format!("Value for {}", f.key()))?;
        new = new.with_field(f, &v);
    }
    new.urls = a.urls;
    new.tags = a.tags;
    new.favorite = a.favorite;
    let id = u.vault.create_item(new)?;
    u.vault.close()?;
    ctx.out.emit(&hex(&id), &json!({ "id": hex(&id) }))
}

pub fn get(ctx: &Ctx, a: GetArgs) -> Result<()> {
    let mut u = open(ctx)?;
    let id = resolve(&mut u.vault, &a.id)?;
    let view = u.vault.get_item(&id)?;
    let mut lines = vec![
        format!("id:       {}", hex(&id)),
        format!("type:     {}", view.summary.item_type.as_str()),
        format!("title:    {}", view.summary.title),
        format!("favorite: {}", view.summary.favorite),
    ];
    let mut fields = serde_json::Map::new();
    for (f, v) in &view.fields {
        lines.push(format!("{}: {v}", f.key()));
        fields.insert(f.key().to_owned(), json!(v));
    }
    let mut secrets = serde_json::Map::new();
    for f in &view.secret_fields {
        if a.reveal {
            let v = u.vault.reveal(&id, *f)?;
            let v = v.as_deref().map_or("", String::as_str);
            lines.push(format!("{}: {v}", f.key()));
            secrets.insert(f.key().to_owned(), json!(v));
        } else {
            lines.push(format!("{}: (set; use --reveal to print)", f.key()));
            secrets.insert(f.key().to_owned(), Value::Null);
        }
    }
    for url in &view.urls {
        lines.push(format!("url: {}", url.url));
    }
    if !view.tags.is_empty() {
        lines.push(format!("tags: {}", view.tags.join(", ")));
    }
    let mut custom = Vec::new();
    for c in &view.custom {
        let value = match (&c.value, c.has_value, a.reveal) {
            (Some(v), _, _) => Some(v.clone()),
            (None, true, true) => u
                .vault
                .reveal_custom(&id, &c.id)?
                .map(|z| z.as_str().to_owned()),
            _ => None,
        };
        lines.push(format!(
            "custom {}: {}",
            c.label,
            match (&value, c.has_value) {
                (Some(v), _) => v.as_str(),
                (None, true) => "(set; use --reveal to print)",
                (None, false) => "",
            }
        ));
        custom.push(json!({ "label": c.label, "value": value, "has_value": c.has_value }));
    }
    let value = json!({
        "id": hex(&id),
        "type": view.summary.item_type.as_str(),
        "title": view.summary.title,
        "favorite": view.summary.favorite,
        "fields": fields,
        "secrets": secrets,
        "urls": view.urls.iter().map(|u| u.url.clone()).collect::<Vec<_>>(),
        "tags": view.tags,
        "custom": custom,
    });
    u.vault.close()?;
    ctx.out.lines(&lines, &value)
}

pub fn list(ctx: &Ctx, a: ListArgs) -> Result<()> {
    let mut u = open(ctx)?;
    let rows: Vec<ItemSummary> = if a.trash {
        u.vault
            .list_trash()?
            .into_iter()
            .filter(|t| !t.purged)
            .map(|t| t.summary)
            .collect()
    } else {
        let filter = ListFilter {
            item_type: a.item_type.map(item_type),
            tag: a.tag,
            folder_id: None,
            favorites_only: a.favorites,
        };
        u.vault.list(&filter, Page::ALL)?
    };
    u.vault.close()?;
    print_rows(ctx, &rows)
}

fn print_rows(ctx: &Ctx, rows: &[ItemSummary]) -> Result<()> {
    let lines: Vec<String> = rows.iter().map(summary_line).collect();
    ctx.out.lines(
        &lines,
        &json!({ "items": rows.iter().map(summary_json).collect::<Vec<_>>() }),
    )
}

pub fn search(ctx: &Ctx, a: SearchArgs) -> Result<()> {
    let mut u = open(ctx)?;
    let rows = u.vault.search(&SearchQuery {
        text: a.text,
        filter: ListFilter {
            item_type: a.item_type.map(item_type),
            tag: a.tag,
            ..ListFilter::default()
        },
        limit: a.limit,
    })?;
    u.vault.close()?;
    print_rows(ctx, &rows)
}

pub fn edit(ctx: &Ctx, a: EditArgs) -> Result<()> {
    let fields: Vec<(StdField, &str)> = a
        .fields
        .iter()
        .map(|s| parse_field(s))
        .collect::<Result<_>>()?;
    let secret_names: Vec<StdField> = a
        .secrets
        .iter()
        .map(|s| secret_field(s))
        .collect::<Result<_>>()?;
    let clears: Vec<StdField> = a
        .clear
        .iter()
        .map(|s| field_by_name(s))
        .collect::<Result<_>>()?;
    let mut u = open(ctx)?;
    let id = resolve(&mut u.vault, &a.id)?;
    if let Some(t) = &a.title {
        u.vault.set_field(&id, StdField::Title, t)?;
    }
    for (f, v) in fields {
        u.vault.set_field(&id, f, v)?;
    }
    for f in secret_names {
        let v = ctx.secrets.read(&format!("Value for {}", f.key()))?;
        u.vault.set_field(&id, f, &v)?;
    }
    for f in clears {
        u.vault.clear_field(&id, f)?;
    }
    for t in &a.add_tags {
        u.vault.add_tag(&id, t)?;
    }
    for t in &a.remove_tags {
        u.vault.remove_tag(&id, t)?;
    }
    for url in &a.add_urls {
        u.vault.add_url(&id, url)?;
    }
    if a.toggle_favorite {
        u.vault.toggle_favorite(&id)?;
    }
    u.vault.close()?;
    ctx.out.emit(
        &format!("updated {}", hex(&id)),
        &json!({ "id": hex(&id), "updated": true }),
    )
}

pub fn delete(ctx: &Ctx, a: IdArg) -> Result<()> {
    mutate(ctx, &a.id, "deleted (in the trash)", |v, id| {
        Ok(v.delete_item(id)?)
    })
}

pub fn restore(ctx: &Ctx, a: IdArg) -> Result<()> {
    mutate(ctx, &a.id, "restored", |v, id| Ok(v.restore_item(id)?))
}

pub fn purge(ctx: &Ctx, a: IdArg) -> Result<()> {
    mutate(ctx, &a.id, "purged", |v, id| Ok(v.purge_item(id)?))
}

fn mutate(
    ctx: &Ctx,
    id: &str,
    what: &str,
    f: impl FnOnce(&mut Vault, &Id) -> Result<()>,
) -> Result<()> {
    let mut u = open(ctx)?;
    let id = resolve(&mut u.vault, id)?;
    f(&mut u.vault, &id)?;
    u.vault.close()?;
    ctx.out.emit(
        &format!("{} {what}", hex(&id)),
        &json!({ "id": hex(&id), "status": what }),
    )
}
