use crate::{
    state::{NAMESPACE_PREFIX, SHARED_STATE},
    Column, ConnectionPool, Map, Model, Mutation, Query, Validation,
};
use futures::TryStreamExt;
use serde::de::DeserializeOwned;
use serde_json::json;
use sqlx::{Error, Row};

/// Model schema.
pub trait Schema: 'static + Send + Sync + Model {
    /// Type name as a str.
    const TYPE_NAME: &'static str;
    /// Primary key name as a str.
    const PRIMARY_KEY_NAME: &'static str = "id";
    /// Reader name.
    const READER_NAME: &'static str = "main";
    /// Writer name.
    const WRITER_NAME: &'static str = "main";

    /// Returns a reference to the columns.
    fn columns() -> &'static [Column<'static>];

    /// Returns the primary key value as a `String`.
    fn primary_key(&self) -> String;

    /// Initializes model reader.
    async fn init_reader() -> Option<&'static ConnectionPool>;

    /// Initializes model writer.
    async fn init_writer() -> Option<&'static ConnectionPool>;

    /// Returns the model name.
    #[inline]
    fn model_name() -> &'static str {
        Self::TYPE_NAME
    }

    /// Returns the model namespace.
    #[inline]
    fn model_namespace() -> &'static str {
        [*NAMESPACE_PREFIX, Self::TYPE_NAME].join(":").leak()
    }

    /// Returns the table name.
    #[inline]
    fn table_name() -> &'static str {
        [*NAMESPACE_PREFIX, Self::TYPE_NAME]
            .join("_")
            .replace(':', "_")
            .leak()
    }

    /// Gets a column for the field.
    #[inline]
    fn get_column(key: &str) -> Option<&Column<'static>> {
        Self::columns().iter().find(|c| c.name() == key)
    }

    /// Gets model reader.
    #[inline]
    fn get_reader() -> Option<&'static ConnectionPool> {
        SHARED_STATE.get_pool(Self::READER_NAME)
    }

    /// Gets model writer.
    #[inline]
    fn get_writer() -> Option<&'static ConnectionPool> {
        SHARED_STATE.get_pool(Self::WRITER_NAME)
    }

    /// Creates table for the model.
    async fn create_table() -> Result<u64, Error> {
        let pool = Self::get_writer().ok_or(Error::PoolClosed)?.pool();
        let table_name = Self::table_name();
        let primary_key_name = Self::PRIMARY_KEY_NAME;
        let mut columns = Vec::new();
        for col in Self::columns() {
            let name = col.name();
            let postgres_type = col.postgres_type();
            let mut column = format!("{name} {postgres_type}");
            if let Some(value) = col.default_value() {
                column = column + " DEFAULT " + &col.format_postgres_value(value);
            } else if col.is_not_null() {
                column += " NOT NULL";
            }
            columns.push(column);
        }
        let sql = format!(
            "
                CREATE TABLE IF NOT EXISTS {0} (
                    {1},
                    CONSTRAINT {0}_pkey PRIMARY KEY ({2})
                );
            ",
            table_name,
            columns.join(",\n"),
            primary_key_name
        );
        let query_result = sqlx::query(&sql).execute(pool).await?;
        Ok(query_result.rows_affected())
    }

    /// Creates indexes for the model.
    async fn create_indexes() -> Result<u64, Error> {
        let pool = Self::get_writer().ok_or(Error::PoolClosed)?.pool();
        let table_name = Self::table_name();
        let mut text_search_languages = Vec::new();
        let mut text_search_columns = Vec::new();
        let mut rows = 0;
        for col in Self::columns() {
            if let Some(index_type) = col.index_type() {
                let column_name = col.name();
                if index_type.starts_with("text") {
                    let language = index_type.strip_prefix("text:").unwrap_or("english");
                    let column = format!("coalesce({column_name}, '')");
                    text_search_languages.push(language);
                    text_search_columns.push((language, column));
                } else {
                    let sort_order = if index_type == "btree" { " DESC" } else { "" };
                    let sql = format!(
                        "
                            CREATE INDEX CONCURRENTLY IF NOT EXISTS {table_name}_{column_name}_index
                            ON {table_name} USING {index_type}({column_name}{sort_order});
                        "
                    );
                    rows = sqlx::query(&sql)
                        .execute(pool)
                        .await?
                        .rows_affected()
                        .max(rows);
                }
            }
        }
        for language in text_search_languages {
            let column = text_search_columns
                .iter()
                .filter_map(|col| (col.0 == language).then_some(col.1.as_str()))
                .intersperse(" || ' ' || ")
                .collect::<String>();
            let text_search = format!("to_tsvector('{language}', {column})");
            let sql = format!(
                "
                    CREATE INDEX CONCURRENTLY IF NOT EXISTS {table_name}_text_search_{language}_index
                    ON {table_name} USING gin({text_search});
                "
            );
            rows = sqlx::query(&sql)
                .execute(pool)
                .await?
                .rows_affected()
                .max(rows);
        }
        Ok(rows)
    }

    /// Inserts the model into the table.
    async fn insert(self) -> Result<u64, Error> {
        let pool = Self::init_writer().await.ok_or(Error::PoolClosed)?.pool();
        let table_name = Self::table_name();
        let map = self.into_map();
        let mut keys = Vec::new();
        let mut values = Vec::new();
        for col in Self::columns() {
            let key = col.name();
            let value = col.encode_postgres_value(map.get(key));
            keys.push(key);
            values.push(value);
        }
        let sql = format!(
            "INSERT INTO {0} ({1}) VALUES ({2});",
            table_name,
            keys.join(","),
            values.join(",")
        );
        let query_result = sqlx::query(&sql).execute(pool).await?;
        Ok(query_result.rows_affected())
    }

    /// Inserts many models into the table.
    async fn insert_many(models: Vec<Self>) -> Result<u64, Error> {
        let pool = Self::init_writer().await.ok_or(Error::PoolClosed)?.pool();
        let table_name = Self::table_name();
        let mut keys = Vec::new();
        let mut values = Vec::new();
        for model in models.into_iter() {
            let map = model.into_map();
            let mut entries = Vec::new();
            for col in Self::columns() {
                let key = col.name();
                let value = col.encode_postgres_value(map.get(key));
                keys.push(key);
                entries.push(value);
            }
            values.push(format!("({})", entries.join(",")));
        }
        let sql = format!(
            "INSERT INTO {0} ({1}) VALUES {2};",
            table_name,
            keys.join(","),
            values.join(",")
        );
        let query_result = sqlx::query(&sql).execute(pool).await?;
        Ok(query_result.rows_affected())
    }

    /// Updates the model in the table.
    async fn update(self) -> Result<u64, Error> {
        let pool = Self::init_writer().await.ok_or(Error::PoolClosed)?.pool();
        let table_name = Self::table_name();
        let primary_key_name = Self::PRIMARY_KEY_NAME;
        let primary_key = self.primary_key();
        let map = self.into_map();
        let mut mutations = Vec::new();
        for col in Self::columns() {
            let key = col.name();
            if key != primary_key_name {
                let value = col.encode_postgres_value(map.get(key));
                mutations.push(format!("{key} = {value}"));
            }
        }
        let sql = format!(
            "UPDATE {0} SET {1} WHERE {2} = '{3}';",
            table_name,
            mutations.join(","),
            primary_key_name,
            primary_key
        );
        let query_result = sqlx::query(&sql).execute(pool).await?;
        Ok(query_result.rows_affected())
    }

    /// Updates at most one model selected by the query in the table.
    async fn update_one(query: Query, mutation: Mutation) -> Result<u64, Error> {
        let pool = Self::init_writer().await.ok_or(Error::PoolClosed)?.pool();
        let table_name = Self::table_name();
        let primary_key_name = Self::PRIMARY_KEY_NAME;
        let filter = query.format_filter::<Self>();
        let sort = query.format_sort();
        let update = mutation.format_update::<Self>();
        let sql = format!(
            "
                UPDATE {table_name} {update} WHERE {primary_key_name} IN
                (SELECT {primary_key_name} FROM {table_name} {filter} {sort} LIMIT 1);
            "
        );
        let query_result = sqlx::query(&sql).execute(pool).await?;
        Ok(query_result.rows_affected())
    }

    /// Updates many models selected by the query in the table.
    async fn update_many(query: Query, mutation: Mutation) -> Result<u64, Error> {
        let pool = Self::init_writer().await.ok_or(Error::PoolClosed)?.pool();
        let table_name = Self::table_name();
        let filter = query.format_filter::<Self>();
        let update = mutation.format_update::<Self>();
        let sql = format!("UPDATE {table_name} {update} {filter};");
        let query_result = sqlx::query(&sql).execute(pool).await?;
        Ok(query_result.rows_affected())
    }

    /// Updates or inserts the model into the table.
    async fn upsert(self) -> Result<u64, Error> {
        let pool = Self::init_writer().await.ok_or(Error::PoolClosed)?.pool();
        let table_name = Self::table_name();
        let primary_key_name = Self::PRIMARY_KEY_NAME;
        let map = self.into_map();
        let mut keys = Vec::new();
        let mut values = Vec::new();
        let mut mutations = Vec::new();
        for col in Self::columns() {
            let key = col.name();
            let value = col.encode_postgres_value(map.get(key));
            if key != primary_key_name {
                mutations.push(format!("{key} = {value}"));
            }
            keys.push(key);
            values.push(value);
        }
        let sql = format!(
            "
                INSERT INTO {0} ({1}) VALUES ({2})
                ON CONFLICT ({3}) DO UPDATE SET {4};
            ",
            table_name,
            keys.join(","),
            values.join(","),
            primary_key_name,
            mutations.join(",")
        );
        let query_result = sqlx::query(&sql).execute(pool).await?;
        Ok(query_result.rows_affected())
    }

    /// Deletes the model in the table.
    async fn delete(self) -> Result<u64, Error> {
        let pool = Self::init_writer().await.ok_or(Error::PoolClosed)?.pool();
        let table_name = Self::table_name();
        let primary_key_name = Self::PRIMARY_KEY_NAME;
        let primary_key = self.primary_key();
        let sql = format!("DELETE FROM {table_name} WHERE {primary_key_name} = '{primary_key}';");
        let query_result = sqlx::query(&sql).execute(pool).await?;
        Ok(query_result.rows_affected())
    }

    /// Deletes at most one model selected by the query in the table.
    async fn delete_one(query: Query) -> Result<u64, Error> {
        let pool = Self::init_writer().await.ok_or(Error::PoolClosed)?.pool();
        let table_name = Self::table_name();
        let primary_key_name = Self::PRIMARY_KEY_NAME;
        let filter = query.format_filter::<Self>();
        let sort = query.format_sort();
        let sql = format!(
            "
                DELETE FROM {table_name} WHERE {primary_key_name} IN
                (SELECT {primary_key_name} FROM {table_name} {filter} {sort} LIMIT 1);
            "
        );
        let query_result = sqlx::query(&sql).execute(pool).await?;
        Ok(query_result.rows_affected())
    }

    /// Deletes many models selected by the query in the table.
    async fn delete_many(query: Query) -> Result<u64, Error> {
        let pool = Self::init_writer().await.ok_or(Error::PoolClosed)?.pool();
        let table_name = Self::table_name();
        let filter = query.format_filter::<Self>();
        let sql = format!("DELETE FROM {table_name} {filter};");
        let query_result = sqlx::query(&sql).execute(pool).await?;
        Ok(query_result.rows_affected())
    }

    /// Finds models selected by the query in the table, and parses it as `Vec<Map>`.
    async fn find(query: Query) -> Result<Vec<Map>, Error> {
        let pool = Self::init_reader().await.ok_or(Error::PoolClosed)?.pool();
        let table_name = Self::table_name();
        let fields = query.fields();
        let projection = query.format_fields();
        let filter = query.format_filter::<Self>();
        let sort = query.format_sort();
        let pagination = query.format_pagination();
        let sql = format!("SELECT {projection} FROM {table_name} {filter} {sort} {pagination};");
        let mut rows = sqlx::query(&sql).fetch(pool);
        let mut data = Vec::new();
        if fields.is_empty() {
            let columns = Self::columns();
            let capacity = columns.len();
            while let Some(row) = rows.try_next().await? {
                let mut map = Map::with_capacity(capacity);
                for col in columns {
                    let value = col.decode_postgres_row(&row)?;
                    map.insert(col.name().to_string(), value);
                }
                data.push(map);
            }
        } else {
            while let Some(row) = rows.try_next().await? {
                let map = Column::parse_postgres_row(&row)?;
                data.push(map);
            }
        }
        Ok(data)
    }

    /// Finds models selected by the query in the table, and parses it as `Vec<T>`.
    async fn find_as<T: DeserializeOwned>(query: Query) -> Result<Vec<T>, Error> {
        let data = Self::find(query).await?;
        serde_json::from_value(data.into()).map_err(|err| Error::Decode(Box::new(err)))
    }

    /// Finds one model selected by the query in the table, and parses it as a `Map`.
    async fn find_one(query: Query) -> Result<Option<Map>, Error> {
        let pool = Self::init_reader().await.ok_or(Error::PoolClosed)?.pool();
        let table_name = Self::table_name();
        let fields = query.fields();
        let projection = query.format_fields();
        let filter = query.format_filter::<Self>();
        let sort = query.format_sort();
        let sql = format!("SELECT {projection} FROM {table_name} {filter} {sort} LIMIT 1;");
        let data = match sqlx::query(&sql).fetch_optional(pool).await? {
            Some(row) => {
                if fields.is_empty() {
                    let columns = Self::columns();
                    let mut map = Map::with_capacity(columns.len());
                    for col in columns {
                        let value = col.decode_postgres_row(&row)?;
                        map.insert(col.name().to_string(), value);
                    }
                    Some(map)
                } else {
                    let map = Column::parse_postgres_row(&row)?;
                    Some(map)
                }
            }
            None => None,
        };
        Ok(data)
    }

    /// Finds one model selected by the query in the table, and parses it as an instance of type `T`.
    async fn find_one_as<T: DeserializeOwned>(query: Query) -> Result<Option<T>, Error> {
        match Self::find_one(query).await? {
            Some(data) => {
                serde_json::from_value(data.into()).map_err(|err| Error::Decode(Box::new(err)))
            }
            None => Ok(None),
        }
    }

    /// Fetches the associated data for `Vec<Map>` using a merged select on the primary key,
    /// which solves the `N+1` problem.
    async fn fetch(
        mut query: Query,
        data: &mut Vec<Map>,
        columns: &[String],
    ) -> Result<u64, Error> {
        let pool = Self::init_reader().await.ok_or(Error::PoolClosed)?.pool();
        let table_name = Self::table_name();
        let primary_key_name = Self::PRIMARY_KEY_NAME;
        let mut values: Vec<String> = Vec::new();
        for row in data.iter() {
            for col in columns {
                if let Some(mut vec) = Validation::parse_array(row.get(col)) {
                    values.append(&mut vec);
                }
            }
        }
        if !values.is_empty() {
            let mut primary_key_filter = Map::new();
            primary_key_filter.insert(
                primary_key_name.to_string(),
                json!({
                    "$in": values,
                }),
            );
            query.append_filter(&mut primary_key_filter);
        }

        let fields = query.fields();
        let projection = query.format_fields();
        let filter = query.format_filter::<Self>();
        let sql = format!("SELECT {projection} FROM {table_name} {filter};");
        let mut rows = sqlx::query(&sql).fetch(pool);
        let mut associations = Map::new();
        if fields.is_empty() {
            let columns = Self::columns();
            let capacity = columns.len();
            while let Some(row) = rows.try_next().await? {
                let primary_key_value = row.try_get_unchecked::<String, _>(primary_key_name)?;
                let mut map = Map::with_capacity(capacity);
                for col in columns {
                    let value = col.decode_postgres_row(&row)?;
                    map.insert(col.name().to_string(), value);
                }
                associations.insert(primary_key_value, map.into());
            }
        } else {
            while let Some(row) = rows.try_next().await? {
                let primary_key_value = row.try_get_unchecked::<String, _>(primary_key_name)?;
                let map = Column::parse_postgres_row(&row)?;
                associations.insert(primary_key_value, map.into());
            }
        }
        for row in data {
            for col in columns {
                if let Some(value) = row.get_mut(col) {
                    if let Some(value) = value.as_str() {
                        if let Some(value) = associations.get(value) {
                            row.insert(col.to_string(), value.clone());
                        }
                    } else if let Some(entries) = value.as_array_mut() {
                        for entry in entries {
                            if let Some(value) = entry.as_str() {
                                if let Some(value) = associations.get(value) {
                                    *entry = value.clone();
                                }
                            }
                        }
                    }
                }
            }
        }
        u64::try_from(associations.len()).map_err(|err| Error::Decode(Box::new(err)))
    }

    /// Fetches the associated data for `Map` using a merged select on the primary key,
    /// which solves the `N+1` problem.
    async fn fetch_one(mut query: Query, data: &mut Map, columns: &[String]) -> Result<u64, Error> {
        let pool = Self::init_reader().await.ok_or(Error::PoolClosed)?.pool();
        let table_name = Self::table_name();
        let primary_key_name = Self::PRIMARY_KEY_NAME;
        let mut values: Vec<String> = Vec::new();
        for col in columns {
            if let Some(mut vec) = Validation::parse_array(data.get(col)) {
                values.append(&mut vec);
            }
        }
        if !values.is_empty() {
            let mut primary_key_filter = Map::new();
            primary_key_filter.insert(
                primary_key_name.to_string(),
                json!({
                    "$in": values,
                }),
            );
            query.append_filter(&mut primary_key_filter);
        }

        let fields = query.fields();
        let projection = query.format_fields();
        let filter = query.format_filter::<Self>();
        let sql = format!("SELECT {projection} FROM {table_name} {filter};");
        let mut rows = sqlx::query(&sql).fetch(pool);
        let mut associations = Map::new();
        if fields.is_empty() {
            let columns = Self::columns();
            let capacity = columns.len();
            while let Some(row) = rows.try_next().await? {
                let primary_key_value = row.try_get_unchecked::<String, _>(primary_key_name)?;
                let mut map = Map::with_capacity(capacity);
                for col in columns {
                    let value = col.decode_postgres_row(&row)?;
                    map.insert(col.name().to_string(), value);
                }
                associations.insert(primary_key_value, map.into());
            }
        } else {
            while let Some(row) = rows.try_next().await? {
                let primary_key_value = row.try_get_unchecked::<String, _>(primary_key_name)?;
                let map = Column::parse_postgres_row(&row)?;
                associations.insert(primary_key_value, map.into());
            }
        }
        for col in columns {
            if let Some(value) = data.get_mut(col) {
                if let Some(value) = value.as_str() {
                    if let Some(value) = associations.get(value) {
                        data.insert(col.to_string(), value.clone());
                    }
                } else if let Some(entries) = value.as_array_mut() {
                    for entry in entries {
                        if let Some(value) = entry.as_str() {
                            if let Some(value) = associations.get(value) {
                                *entry = value.clone();
                            }
                        }
                    }
                }
            }
        }
        u64::try_from(associations.len()).map_err(|err| Error::Decode(Box::new(err)))
    }

    /// Executes the query in the table, and returns the total number of rows affected.
    async fn execute(sql: &str, params: Option<&[String]>) -> Result<u64, Error> {
        let pool = Self::init_reader().await.ok_or(Error::PoolClosed)?.pool();
        let mut query = sqlx::query(sql);
        if let Some(params) = params {
            for param in params {
                query = query.bind(param);
            }
        }
        let query_result = query.execute(pool).await?;
        Ok(query_result.rows_affected())
    }

    /// Executes the query in the table, and parses it as `Vec<Map>`.
    async fn query(sql: &str, params: Option<&[String]>) -> Result<Vec<Map>, Error> {
        let pool = Self::init_reader().await.ok_or(Error::PoolClosed)?.pool();
        let mut query = sqlx::query(sql);
        if let Some(params) = params {
            for param in params {
                query = query.bind(param);
            }
        }
        let mut rows = query.fetch(pool);
        let mut data = Vec::new();
        while let Some(row) = rows.try_next().await? {
            let map = Column::parse_postgres_row(&row)?;
            data.push(map);
        }
        Ok(data)
    }

    /// Executes the query in the table, and parses it as `Vec<T>`.
    async fn query_as<T: DeserializeOwned>(
        sql: &str,
        params: Option<&[String]>,
    ) -> Result<Vec<T>, Error> {
        let data = Self::query(sql, params).await?;
        serde_json::from_value(data.into()).map_err(|err| Error::Decode(Box::new(err)))
    }

    /// Executes the query in the table, and parses it as a `Map`.
    async fn query_one(sql: &str, params: Option<&[String]>) -> Result<Option<Map>, Error> {
        let pool = Self::init_reader().await.ok_or(Error::PoolClosed)?.pool();
        let mut query = sqlx::query(sql);
        if let Some(params) = params {
            for param in params {
                query = query.bind(param);
            }
        }
        let data = match query.fetch_optional(pool).await? {
            Some(row) => {
                let map = Column::parse_postgres_row(&row)?;
                Some(map)
            }
            None => None,
        };
        Ok(data)
    }

    /// Executes the query in the table, and parses it as an instance of type `T`.
    async fn query_one_as<T: DeserializeOwned>(
        sql: &str,
        params: Option<&[String]>,
    ) -> Result<Option<T>, Error> {
        match Self::query_one(sql, params).await? {
            Some(data) => {
                serde_json::from_value(data.into()).map_err(|err| Error::Decode(Box::new(err)))
            }
            None => Ok(None),
        }
    }

    /// Finds one model selected by the primary key in the table, and parses it as `Self`.
    async fn try_get_model(primary_key: &str) -> Result<Self, Error> {
        let pool = Self::init_reader().await.ok_or(Error::PoolClosed)?.pool();
        let table_name = Self::table_name();
        let primary_key_name = Self::PRIMARY_KEY_NAME;
        let sql = format!(
            "SELECT * FROM {0} WHERE {1} = {2};",
            table_name,
            primary_key_name,
            Column::format_postgres_string(primary_key)
        );
        match sqlx::query(&sql).fetch_optional(pool).await? {
            Some(row) => {
                let map = Column::parse_postgres_row(&row)?;
                serde_json::from_value(map.into()).map_err(|err| Error::Decode(Box::new(err)))
            }
            None => Err(Error::RowNotFound),
        }
    }
}
