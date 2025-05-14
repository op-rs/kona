use reth_db_api::table::TableInfo;
use reth_db_api::TableSet;
use crate::log::models::LogEntries;

#[derive(Debug, Clone, Copy)]
pub struct LogStorageTables;

impl TableSet for LogStorageTables {
    fn tables() -> Box<dyn Iterator<Item = Box<dyn TableInfo>>> {
        Box::new(vec![
            Box::new(LogEntries) as Box<dyn TableInfo>,
        ].into_iter())
    }
}