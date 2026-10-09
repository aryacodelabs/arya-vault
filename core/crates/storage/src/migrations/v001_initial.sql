-- Schema version 1: docs/05-data-model.md section 5 (including the review
-- additions: base_hlc columns, outbox, manifest_seen). Do not edit once
-- released; add a new migration instead.
CREATE TABLE meta (
  key TEXT PRIMARY KEY, value BLOB NOT NULL
);

CREATE TABLE item (
  id BLOB PRIMARY KEY,
  type TEXT NOT NULL,
  folder_id BLOB,
  deleted INTEGER NOT NULL DEFAULT 0,
  deleted_hlc INTEGER,
  updated_hlc INTEGER NOT NULL
);

CREATE TABLE field (
  item_id BLOB NOT NULL REFERENCES item(id),
  key TEXT NOT NULL,
  value BLOB,
  hlc INTEGER NOT NULL,
  device_id BLOB NOT NULL,
  base_hlc INTEGER,
  PRIMARY KEY (item_id, key)
);

CREATE TABLE field_history (
  item_id BLOB NOT NULL, key TEXT NOT NULL,
  value BLOB, hlc INTEGER NOT NULL, device_id BLOB NOT NULL,
  base_hlc INTEGER,
  PRIMARY KEY (item_id, key, hlc, device_id)
);

CREATE TABLE folder (
  id BLOB PRIMARY KEY, name TEXT NOT NULL, parent_id BLOB,
  hlc INTEGER NOT NULL, device_id BLOB NOT NULL, deleted INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE local_op (
  seq INTEGER PRIMARY KEY AUTOINCREMENT,
  item_id BLOB NOT NULL, key TEXT NOT NULL, value BLOB,
  hlc INTEGER NOT NULL, base_hlc INTEGER
);
CREATE TABLE outbox (
  seq INTEGER PRIMARY KEY,
  bytes BLOB NOT NULL,
  uploaded INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE manifest_seen (
  device_id BLOB PRIMARY KEY, counter INTEGER NOT NULL
);
CREATE TABLE segment_seen (
  device_id BLOB NOT NULL, seq INTEGER NOT NULL, hash BLOB NOT NULL,
  applied_at INTEGER NOT NULL, PRIMARY KEY (device_id, seq)
);
CREATE TABLE device (
  device_id BLOB PRIMARY KEY, name TEXT, first_seen INTEGER, last_seen INTEGER, revoked INTEGER DEFAULT 0
);
CREATE TABLE provider_state (
  provider TEXT PRIMARY KEY, cursor TEXT, updated_at INTEGER
);

-- Search index lives only inside the encrypted database (SEC-S05).
CREATE VIRTUAL TABLE item_fts USING fts5(title, username, urls, notes, tags, content='');

CREATE INDEX item_type_deleted ON item(type, deleted);
CREATE INDEX item_folder ON item(folder_id);
CREATE INDEX field_item ON field(item_id);
CREATE INDEX field_history_item_key ON field_history(item_id, key);
