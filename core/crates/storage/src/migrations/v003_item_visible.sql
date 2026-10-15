-- Schema version 3: list paging (docs/05 section 5.1).
--
-- `item.visible` stores the visibility rule so a list page is one index range read: an item is
-- visible when it is not deleted, or when some register other than the deletion bookkeeping is
-- newer than the deletion (`engine::is_visible` in the vault crate, doc 06 section 5.2). It is
-- derived data, rewritten wherever the rest of the item row is rebuilt from the registers, and it
-- is not synced or exported.
ALTER TABLE item ADD COLUMN visible INTEGER NOT NULL DEFAULT 1;

UPDATE item SET visible = EXISTS (
  SELECT 1 FROM field f
  WHERE f.item_id = item.id
    AND f.key NOT IN ('deleted', 'deleted_at')
    AND f.hlc > item.deleted_hlc
) WHERE deleted <> 0;

CREATE INDEX item_visible_updated ON item(visible, updated_hlc DESC, id);
