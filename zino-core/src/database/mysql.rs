use super::{query::QueryExt, DatabaseDriver, DatabaseRow};
use crate::{
    datetime::DateTime,
    model::{Column, DecodeRow, EncodeColumn, Query},
    request::Validation,
    Map, Record, SharedString,
};
use apache_avro::types::Value as AvroValue;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use serde_json::Value as JsonValue;
use sqlx::{Column as _, Error, Row, TypeInfo};
use std::borrow::Cow;

impl<'c> EncodeColumn<DatabaseDriver> for Column<'c> {
    fn column_type(&self) -> &str {
        let type_name = self.type_name();
        match type_name {
            "bool" => "BOOLEAN",
            "u64" | "usize" => "BIGINT UNSIGNED",
            "i64" | "isize" => "BIGINT",
            "u32" => "INT UNSIGNED",
            "i32" => "INT",
            "u16" => "SMALLINT UNSIGNED",
            "i16" => "SMALLINT",
            "u8" => "TINYINT UNSIGNED",
            "i8" => "TINYINT",
            "f64" => "DOUBLE",
            "f32" => "FLOAT",
            "String" => {
                if self.default_value().or(self.index_type()).is_some() {
                    "VARCHAR(255)"
                } else {
                    "TEXT"
                }
            }
            "DateTime" => "TIMESTAMP(6)",
            "NaiveDateTime" => "DATETIME(6)",
            "NaiveDate" | "Date" => "DATE",
            "NaiveTime" | "Time" => "TIME",
            "Uuid" | "Option<Uuid>" => "VARCHAR(36)",
            "Vec<u8>" => "BLOB",
            "Vec<String>" => "JSON",
            "Vec<Uuid>" => "JSON",
            "Map" => "JSON",
            _ => type_name,
        }
    }

    fn encode_value<'a>(&self, value: Option<&'a JsonValue>) -> Cow<'a, str> {
        if let Some(value) = value {
            match value {
                JsonValue::Null => "NULL".into(),
                JsonValue::Bool(value) => {
                    let value = if *value { "TRUE" } else { "FALSE" };
                    value.into()
                }
                JsonValue::Number(value) => value.to_string().into(),
                JsonValue::String(value) => {
                    if value.is_empty() {
                        if let Some(value) = self.default_value() {
                            self.format_value(value).into_owned().into()
                        } else {
                            "''".into()
                        }
                    } else if value == "null" {
                        "NULL".into()
                    } else {
                        self.format_value(value)
                    }
                }
                JsonValue::Array(value) => {
                    let values = value
                        .iter()
                        .map(|v| match v {
                            JsonValue::String(v) => Query::escape_string(v),
                            _ => self.encode_value(Some(v)).into_owned(),
                        })
                        .collect::<Vec<_>>();
                    format!(r#"json_array({})"#, values.join(",")).into()
                }
                JsonValue::Object(_) => format!("'{value}'").into(),
            }
        } else if self.default_value().is_some() {
            "DEFAULT".into()
        } else {
            "NULL".into()
        }
    }

    fn format_value<'a>(&self, value: &'a str) -> Cow<'a, str> {
        match self.type_name() {
            "bool" => {
                let value = if value == "true" { "TRUE" } else { "FALSE" };
                value.into()
            }
            "u64" | "u32" | "u16" | "u8" | "usize" => {
                if value.parse::<u64>().is_ok() {
                    value.into()
                } else {
                    "NULL".into()
                }
            }
            "i64" | "i32" | "i16" | "i8" | "isize" => {
                if value.parse::<i64>().is_ok() {
                    value.into()
                } else {
                    "NULL".into()
                }
            }
            "f64" | "f32" => {
                if value.parse::<f64>().is_ok() {
                    value.into()
                } else {
                    "NULL".into()
                }
            }
            "String" | "Uuid" | "Option<Uuid>" => Query::escape_string(value).into(),
            "DateTime" | "NaiveDateTime" => match value {
                "epoch" => "from_unixtime(0)".into(),
                "now" => "current_timestamp(6)".into(),
                "today" => "curdate()".into(),
                "tomorrow" => "curdate() + INTERVAL 1 DAY".into(),
                "yesterday" => "curdate() - INTERVAL 1 DAY".into(),
                _ => Query::escape_string(value).into(),
            },
            "Date" | "NaiveDate" => match value {
                "epoch" => "'1970-01-01'".into(),
                "today" => "curdate()".into(),
                "tomorrow" => "curdate() + INTERVAL 1 DAY".into(),
                "yesterday" => "curdate() - INTERVAL 1 DAY".into(),
                _ => Query::escape_string(value).into(),
            },
            "Time" | "NaiveTime" => match value {
                "now" => "curtime()".into(),
                "midnight" => "'00:00:00'".into(),
                _ => Query::escape_string(value).into(),
            },
            "Vec<u8>" => format!("'value'").into(),
            "Vec<String>" | "Vec<Uuid>" => {
                if value.contains(',') {
                    let values = value
                        .split(',')
                        .map(Query::escape_string)
                        .collect::<Vec<_>>();
                    format!(r#"json_array({})"#, values.join(",")).into()
                } else {
                    let value = Query::escape_string(value);
                    format!(r#"json_array({value})"#).into()
                }
            }
            "Map" => {
                let value = Query::escape_string(value);
                format!("'{value}'").into()
            }
            _ => "NULL".into(),
        }
    }

    fn format_filter(&self, field: &str, value: &serde_json::Value) -> String {
        let type_name = self.type_name();
        if let Some(filter) = value.as_object() {
            if type_name == "Map" {
                let field = Query::format_field(field);
                let value = self.encode_value(Some(value));
                // `json_overlaps()` was added in MySQL 8.0.17.
                return format!(r#"json_overlaps({field}, {value})"#);
            } else {
                let mut conditions = Vec::with_capacity(filter.len());
                for (name, value) in filter {
                    let operator = match name.as_str() {
                        "$eq" => "=",
                        "$ne" => "<>",
                        "$lt" => "<",
                        "$lte" => "<=",
                        "$gt" => ">",
                        "$gte" => ">=",
                        "$in" => "IN",
                        "$nin" => "NOT IN",
                        _ => "=",
                    };
                    if operator == "IN" || operator == "NOT IN" {
                        if let Some(value) = value.as_array() && !value.is_empty() {
                            let field = Query::format_field(field);
                            let value = value
                                .iter()
                                .map(|v| self.encode_value(Some(v)))
                                .collect::<Vec<_>>()
                                .join(",");
                            let condition = format!(r#"{field} {operator} ({value})"#);
                            conditions.push(condition);
                        }
                    } else {
                        let field = Query::format_field(field);
                        let value = self.encode_value(Some(value));
                        let condition = format!(r#"{field} {operator} {value}"#);
                        conditions.push(condition);
                    }
                }
                if conditions.is_empty() {
                    return String::new();
                } else {
                    return format!("({})", conditions.join(" AND "));
                }
            }
        }
        match type_name {
            "bool" => {
                let field = Query::format_field(field);
                let value = self.encode_value(Some(value));
                if value == "TRUE" {
                    format!(r#"{field} IS TRUE"#)
                } else {
                    format!(r#"{field} IS NOT TRUE"#)
                }
            }
            "u64" | "i64" | "u32" | "i32" | "u16" | "i16" | "u8" | "i8" | "usize" | "isize"
            | "f64" | "f32" | "DateTime" | "Date" | "Time" | "NaiveDateTime" | "NaiveDate"
            | "NaiveTime" => {
                let field = Query::format_field(field);
                if let Some(value) = value.as_str() {
                    if let Some((min_value, max_value)) = value.split_once(',') {
                        let min_value = self.format_value(min_value);
                        let max_value = self.format_value(max_value);
                        format!(r#"{field} >= {min_value} AND {field} < {max_value}"#)
                    } else {
                        let index = value.find(|ch| !"<>=".contains(ch)).unwrap_or(0);
                        if index > 0 {
                            let (operator, value) = value.split_at(index);
                            let value = self.format_value(value);
                            format!(r#"{field} {operator} {value}"#)
                        } else {
                            let value = self.format_value(value);
                            format!(r#"{field} = {value}"#)
                        }
                    }
                } else {
                    let value = self.encode_value(Some(value));
                    format!(r#"{field} = {value}"#)
                }
            }
            "String" => {
                let field = Query::format_field(field);
                if let Some(value) = value.as_str() {
                    if value == "null" {
                        // either NULL or empty
                        format!(r#"({field} = '') IS NOT FALSE"#)
                    } else if value == "notnull" {
                        format!(r#"({field} = '') IS FALSE"#)
                    } else {
                        let index = value.find(|ch| !"!~*".contains(ch)).unwrap_or(0);
                        if index > 0 {
                            let (operator, value) = value.split_at(index);
                            let value = Query::escape_string(value);
                            format!(r#"{field} {operator} {value}"#)
                        } else {
                            let value = Query::escape_string(value);
                            format!(r#"{field} = {value}"#)
                        }
                    }
                } else {
                    let value = self.encode_value(Some(value));
                    format!(r#"{field} = {value}"#)
                }
            }
            "Uuid" | "Option<Uuid>" => {
                let field = Query::format_field(field);
                if let Some(value) = value.as_str() {
                    if value == "null" {
                        format!(r#"{field} IS NULL"#)
                    } else if value == "notnull" {
                        format!(r#"{field} IS NOT NULL"#)
                    } else if value.contains(',') {
                        let value = value
                            .split(',')
                            .map(Query::escape_string)
                            .collect::<Vec<_>>()
                            .join(",");
                        format!(r#"{field} IN ({value})"#)
                    } else {
                        let value = Query::escape_string(value);
                        format!(r#"{field} = {value}"#)
                    }
                } else {
                    let value = self.encode_value(Some(value));
                    format!(r#"{field} = {value}"#)
                }
            }
            "Vec<String>" | "Vec<Uuid>" => {
                let field = Query::format_field(field);
                if let Some(value) = value.as_str() {
                    if value.contains(';') {
                        if value.contains(',') {
                            value
                                .split(',')
                                .map(|v| {
                                    let s = v.replace(';', ",");
                                    let value = self.format_value(&s);
                                    format!(r#"json_overlaps({field}, {value})"#)
                                })
                                .collect::<Vec<_>>()
                                .join(" OR ")
                        } else {
                            value
                                .split(';')
                                .map(|v| {
                                    let value = self.format_value(v);
                                    format!(r#"json_overlaps({field}, {value})"#)
                                })
                                .collect::<Vec<_>>()
                                .join(" AND ")
                        }
                    } else {
                        let value = self.format_value(value);
                        format!(r#"json_overlaps({field}, {value})"#)
                    }
                } else {
                    let value = self.encode_value(Some(value));
                    format!(r#"json_overlaps({field}, {value})"#)
                }
            }
            "Map" => {
                let field = Query::format_field(field);
                let value = self.encode_value(Some(value));
                format!(r#"json_overlaps({field}, {value})"#)
            }
            _ => {
                let field = Query::format_field(field);
                let value = self.encode_value(Some(value));
                format!(r#"{field} = {value}"#)
            }
        }
    }
}

impl DecodeRow<DatabaseRow> for Map {
    type Error = Error;

    fn decode_row(row: &DatabaseRow) -> Result<Self, Self::Error> {
        let columns = row.columns();
        let mut map = Map::with_capacity(columns.len());
        for col in columns {
            let field = col.name();
            let index = col.ordinal();
            let value = match col.type_info().name() {
                "BOOLEAN" => row.try_get_unchecked::<bool, _>(index)?.into(),
                "TINYINT" => row.try_get_unchecked::<i8, _>(index)?.into(),
                "TINYINT UNSIGNED" => row.try_get_unchecked::<u8, _>(index)?.into(),
                "SMALLINT" => row.try_get_unchecked::<i16, _>(index)?.into(),
                "SMALLINT UNSIGNED" => row.try_get_unchecked::<u16, _>(index)?.into(),
                "INT" => row.try_get_unchecked::<i32, _>(index)?.into(),
                "INT UNSIGNED" => row.try_get_unchecked::<u32, _>(index)?.into(),
                "BIGINT" => row.try_get_unchecked::<i64, _>(index)?.into(),
                "BIGINT UNSIGNED" => row.try_get_unchecked::<u64, _>(index)?.into(),
                "FLOAT" => row.try_get_unchecked::<f32, _>(index)?.into(),
                "DOUBLE" => row.try_get_unchecked::<f64, _>(index)?.into(),
                "TEXT" | "VARCHAR" | "CHAR" => row.try_get_unchecked::<String, _>(index)?.into(),
                "TIMESTAMP" => row.try_get_unchecked::<DateTime, _>(index)?.into(),
                "DATETIME" => row
                    .try_get_unchecked::<NaiveDateTime, _>(index)?
                    .to_string()
                    .into(),
                "DATE" => row
                    .try_get_unchecked::<NaiveDate, _>(index)?
                    .to_string()
                    .into(),
                "TIME" => row
                    .try_get_unchecked::<NaiveTime, _>(index)?
                    .to_string()
                    .into(),
                "BLOB" | "VARBINARY" | "BINARY" => {
                    row.try_get_unchecked::<Vec<u8>, _>(index)?.into()
                }
                "JSON" => row.try_get_unchecked::<JsonValue, _>(index)?,
                _ => JsonValue::Null,
            };
            map.insert(field.to_owned(), value);
        }
        Ok(map)
    }
}

impl DecodeRow<DatabaseRow> for Record {
    type Error = Error;

    fn decode_row(row: &DatabaseRow) -> Result<Self, Self::Error> {
        let columns = row.columns();
        let mut record = Record::with_capacity(columns.len());
        for col in columns {
            let field = col.name();
            let index = col.ordinal();
            let value = match col.type_info().name() {
                "BOOLEAN" => row.try_get_unchecked::<bool, _>(index)?.into(),
                "INT" | "INT UNSIGNED" => row.try_get_unchecked::<i32, _>(index)?.into(),
                "BIGINT" | "BIGINT UNSIGNED" => row.try_get_unchecked::<i64, _>(index)?.into(),
                "FLOAT" => row.try_get_unchecked::<f32, _>(index)?.into(),
                "DOUBLE" => row.try_get_unchecked::<f64, _>(index)?.into(),
                "TEXT" | "VARCHAR" | "CHAR" => row.try_get_unchecked::<String, _>(index)?.into(),
                "TIMESTAMP" => row.try_get_unchecked::<DateTime, _>(index)?.into(),
                "DATETIME" => row
                    .try_get_unchecked::<NaiveDateTime, _>(index)?
                    .to_string()
                    .into(),
                "DATE" => row
                    .try_get_unchecked::<NaiveDate, _>(index)?
                    .to_string()
                    .into(),
                "TIME" => row
                    .try_get_unchecked::<NaiveTime, _>(index)?
                    .to_string()
                    .into(),
                "BLOB" | "VARBINARY" | "BINARY" => {
                    row.try_get_unchecked::<Vec<u8>, _>(index)?.into()
                }
                "JSON" => row.try_get_unchecked::<JsonValue, _>(index)?.into(),
                _ => AvroValue::Null,
            };
            record.push((field.to_owned(), value));
        }
        Ok(record)
    }
}

impl QueryExt<DatabaseDriver> for Query {
    #[inline]
    fn placeholder(_n: usize) -> SharedString {
        "?".into()
    }

    #[inline]
    fn query_fields(&self) -> &[String] {
        self.fields()
    }

    #[inline]
    fn query_filters(&self) -> &Map {
        self.filters()
    }

    #[inline]
    fn query_order(&self) -> (&str, bool) {
        self.sort_order()
    }

    fn format_pagination(&self) -> String {
        let (sort_by, _) = self.sort_order();
        if self.filters().contains_key(sort_by) {
            format!("LIMIT {}", self.limit())
        } else {
            format!("LIMIT {}, {}", self.offset(), self.limit())
        }
    }

    fn format_field(field: &str) -> Cow<'_, str> {
        if field.contains('.') {
            field
                .split('.')
                .map(|s| format!("`{s}`"))
                .collect::<Vec<_>>()
                .join(".")
                .into()
        } else {
            format!("`{field}`").into()
        }
    }

    fn parse_text_search(filter: &Map) -> Option<String> {
        let fields = Validation::parse_str_array(filter.get("$fields"))?;
        Validation::parse_string(filter.get("$search")).map(|search| {
            let fields = fields.join(",");
            let search = Query::escape_string(search.as_ref());
            format!("match({fields}) against({search})")
        })
    }
}