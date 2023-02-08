//! Database connectors.

use crate::{extend::TomlTableExt, state::State, Map};
use sqlx::Error;
use std::{collections::HashMap, sync::LazyLock};
use toml::Table;

mod data_source;
mod serialize_row;

/// Supported connectors.
mod mssql_connector;
mod mysql_connector;
mod postgres_connector;
mod sqlite_connector;

pub use data_source::DataSource;

use data_source::DataSourcePool;
use serialize_row::SerializeRow;

/// Underlying trait of all data sources for implementors.
trait Connector {
    /// Creates a new data source with the configuration.
    fn new_data_source(config: &'static Table) -> DataSource;

    /// Executes the query and returns the total number of rows affected.
    async fn execute<const N: usize>(
        &self,
        sql: &str,
        params: Option<[&str; N]>,
    ) -> Result<u64, Error>;

    /// Executes the query in the table, and parses it as `Vec<Map>`.
    async fn query<const N: usize>(
        &self,
        sql: &str,
        params: Option<[&str; N]>,
    ) -> Result<Vec<Map>, Error>;

    /// Executes the query in the table, and parses it as a `Map`.
    async fn query_one<const N: usize>(
        &self,
        sql: &str,
        params: Option<[&str; N]>,
    ) -> Result<Option<Map>, Error>;
}

/// Global database connector.
#[derive(Debug, Clone, Copy, Default)]
pub struct GlobalConnector;

impl GlobalConnector {
    /// Gets the data source for the specific database service.
    #[inline]
    pub fn get(name: &'static str) -> Option<&'static DataSource> {
        GLOBAL_CONNECTOR.get(name)
    }
}

/// Global database connector.
static GLOBAL_CONNECTOR: LazyLock<HashMap<&'static str, DataSource>> = LazyLock::new(|| {
    let mut data_sources = HashMap::new();
    if let Some(connectors) = State::shared().config().get_array("connector") {
        for connector in connectors.iter().filter_map(|v| v.as_table()) {
            let database_type = connector.get_str("type").unwrap_or("unkown");
            let name = connector.get_str("name").unwrap_or(database_type);
            let data_source = DataSource::new_connector(database_type, connector)
                .unwrap_or_else(|err| panic!("failed to connect data source `{name}`: {err}"));
            data_sources.insert(name, data_source);
        }
    }
    data_sources
});
