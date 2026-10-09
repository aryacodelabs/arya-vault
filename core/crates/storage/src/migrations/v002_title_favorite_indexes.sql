-- Schema version 2: access paths for "this register of every item".
--
-- Listing needs the `title` and `favorite` registers of every item. Without an index that is a
-- scan of the whole `field` table (every page holds one of each, so a plain index on
-- `field(key)` would read the same pages and gain nothing; measured at 20,000 items: 57 ms
-- scan, 58 ms with `field(key)`, 10 ms with the covering indexes below).
--
-- Partial *covering* indexes, one per register, because SQLite only uses a partial index when
-- the query repeats the same literal (`WHERE key = 'title'`) and only skips the table when the
-- index holds every selected column. They contain exactly the rows of those two non-secret
-- registers: no password, TOTP seed, card number or note body is copied anywhere (SEC-S05),
-- and, like the rest of the file, they exist only inside the encrypted database.
-- `Tx::fields_with_key` picks them for these two keys.
CREATE INDEX field_title_cover
  ON field(item_id, key, value, hlc, device_id, base_hlc) WHERE key = 'title';
CREATE INDEX field_favorite_cover
  ON field(item_id, key, value, hlc, device_id, base_hlc) WHERE key = 'favorite';
