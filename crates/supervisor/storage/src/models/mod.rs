//! Database table schemas used by the Supervisor.
//!
//! This module defines the value types, keys, and table layouts for all data
//! persisted by the `supervisor` component of the node.
//!
//! The tables are registered using [`reth_db_api::table::TableInfo`] and grouped into a
//! [`reth_db_api::TableSet`] for database initialization via Reth's storage-api.

mod log;
pub use log::{LogEntries, LogEntry};
mod block;
pub use block::{BlockHeader, BlockHeaders};

/// Implements [`reth_db_api::table::Compress`] and [`reth_db_api::table::Decompress`] traits for
/// types that implement [`reth_codecs::Compact`].
///
/// This macro defines how to serialize and deserialize a type into a compressed
/// byte format using Reth's compact codec system.
///
/// # Example
/// ```ignore
/// impl_compression_for_compact!(BlockHeader, LogEntry);
/// ```
macro_rules! impl_compression_for_compact {
    ($($name:ident$(<$($generic:ident),*>)?),+) => {
        $(
            impl$(<$($generic: core::fmt::Debug + Send + Sync + Compact),*>)? reth_db_api::table::Compress for $name$(<$($generic),*>)? {
                type Compressed = Vec<u8>;

                fn compress_to_buf<B: bytes::BufMut + AsMut<[u8]>>(&self, buf: &mut B) {
                    let _ = reth_codecs::Compact::to_compact(self, buf);
                }
            }

            impl$(<$($generic: core::fmt::Debug + Send + Sync + Compact),*>)? reth_db_api::table::Decompress for $name$(<$($generic),*>)? {
                fn decompress(value: &[u8]) -> Result<$name$(<$($generic),*>)?, reth_db_api::DatabaseError> {
                    let (obj, _) = reth_codecs::Compact::from_compact(value, value.len());
                    Ok(obj)
                }
            }
        )+
    };
}

/// Implements [`reth_db_api::table::TableInfo`] for one or more table types that implement
/// [`reth_db_api::table::Table`] or [`reth_db_api::table::DupSort`].
///
/// This allows the table to be registered and introspected by the Reth database schema system.
///
/// # Example
/// ```ignore
/// impl_table_info!(BlockHeaders, LogEntries);
/// ```
macro_rules! impl_table_info {
    ($($table:ty),+ $(,)?) => {
        $(
            impl reth_db_api::table::TableInfo for $table
            where
                $table: reth_db_api::table::Table,
            {
                fn name(&self) -> &'static str {
                    <$table as reth_db_api::table::Table>::NAME
                }

                fn is_dupsort(&self) -> bool {
                    <$table as reth_db_api::table::Table>::DUPSORT
                }
            }
        )+
    };
}

/// Declares a struct representing a collection of tables and implements [`reth_db_api::TableSet`]
/// for it.
///
/// The resulting struct can be passed to Reth's `init_db_for::<_, YourTableSet>()`
/// to initialize only the specified tables.
///
/// # Example
/// ```ignore
/// impl_table_set!(LogStorageTables, BlockHeaders, LogEntries);
/// ```
#[macro_export]
macro_rules! impl_table_set {
    (
        $(#[$outer:meta])*
        $set_name:ident, $($table:ty),+ $(,)?
    ) => {
        #[allow(dead_code)]
        $(#[$outer])*
        pub(crate) struct $set_name;

        impl reth_db_api::TableSet for $set_name {
            fn tables() -> Box<dyn Iterator<Item = Box<dyn reth_db_api::table::TableInfo>>> {
                Box::new(vec![
                    $(
                        Box::new(<$table>::default()) as Box<dyn reth_db_api::table::TableInfo>
                    ),*
                ].into_iter())
            }
        }
    };
}

// Implement compression logic for all value types stored in tables
impl_compression_for_compact!(BlockHeader, LogEntry);

// Enable reflection for each table (name + dupsort metadata)
impl_table_info!(BlockHeaders, LogEntries);

// Define and register the full table set used by log storage
impl_table_set!(LogStorageTables, BlockHeaders, LogEntries);
