use crate::CROSS_L2_INBOX_ADDRESS;
use alloy_eips::eip2930::AccessListItem;
use alloy_primitives::B256;

/// Parses [`AccessListItem`]s to inbox entries.
///
/// See [`parse_access_list_item_to_inbox_entries`] for more details. Return flattened iterator with
/// all inbox entries.
pub fn parse_access_list_items_to_inbox_entries<'a>(
    access_list_items: impl Iterator<Item = &'a AccessListItem>,
) -> impl Iterator<Item = &'a B256> {
    access_list_items.filter_map(parse_access_list_item_to_inbox_entries).flatten()
}

/// Parse [`AccessListItem`] to inbox entries, if any.
/// Max 3 inbox entries can exist per [`AccessListItem`] that points to [`CROSS_L2_INBOX_ADDRESS`].
///
/// Returns `Vec::new()` if [`AccessListItem`] address doesn't point to [`CROSS_L2_INBOX_ADDRESS`].
///
/// See: <https://github.com/ethereum-optimism/specs/blob/main/specs/interop/predeploys.md#access-list>
pub fn parse_access_list_item_to_inbox_entries(
    access_list_item: &AccessListItem,
) -> Option<impl Iterator<Item = &B256>> {
    (access_list_item.address == CROSS_L2_INBOX_ADDRESS)
        .then(|| access_list_item.storage_keys.iter())
}
