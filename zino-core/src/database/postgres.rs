use super::{query::QueryExt, DatabaseDriver, DatabaseRow};
use crate::{
    datetime::DateTime,
    model::{Column, DecodeRow, EncodeColumn, Query},
    request::Validation,
    Map, Record, SharedString, Uuid,
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
            "u64" | "i64" | "usize" | "isize" => "BIGINT",
            "u32" | "i32" => "INT",
            "u16" | "i16" | "u8" | "i8" => "SMALLINT",
            "f64" => "DOUBLE PRECISION",
            "f32" => "REAL",
            "String" => "TEXT",
            "DateTime" => "TIMESTAMPTZ",
            "NaiveDateTime" => "TIMESTAMP",
            "NaiveDate" | "Date" => "DATE",
            "NaiveTime" | "Time" => "TIME",
            "Uuid" | "Option<Uuid>" => "UUID",
            "Vec<u8>" => "BYTEA",
            "Vec<String>" => "TEXT[]",
            "Vec<Uuid>" => "UUID[]",
            "Map" => "JSONB",
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
                    format!("ARRAY[{}]::{}", values.join(","), self.column_type()).into()
                }
                JsonValue::Object(_) => format!("'{}'::{}", value, self.column_type()).into(),
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
                "epoch" => "'epoch'".into(),
                "now" => "now()".into(),
                "today" => "date_trunc('day', now())".into(),
                "tomorrow" => "date_trunc('day', now()) + '1 day'::INTERVAL".into(),
                "yesterday" => "date_trunc('day', now()) - '1 day'::INTERVAL".into(),
                _ => Query::escape_string(value).into(),
            },
            "Date" | "NaiveDate" => match value {
                "epoch" => "'epoch'".into(),
                "today" => "curdate()".into(),
                "tomorrow" => "curdate() + INTERVAL 1 DAY".into(),
                "yesterday" => "curdate() - INTERVAL 1 DAY".into(),
                _ => Query::escape_string(value).into(),
            },
            "Time" | "NaiveTime" => match value {
                "now" => "curtime()".into(),
                "midnight" => "'allballs'".into(),
                _ => Query::escape_string(value).into(),
            },
            "Vec<u8>" => format!(r"'\x{value}'").into(),
            "Vec<String>" | "Vec<Uuid>" => {
                let column_type = self.column_type();
                if value.contains(',') {
                    let values = value
                        .split(',')
                        .map(Query::escape_string)
                        .collect::<Vec<_>>();
                    format!("ARRAY[{}]::{}", values.join(","), column_type).into()
                } else {
                    let value = Query::escape_string(value);
                    format!("ARRAY[{value}]::{column_type}").into()
                }
            }
            "Map" => {
                let value = Query::escape_string(value);
                format!("{value}::jsonb").into()
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
                return format!(r#"{field} @> {value}"#);
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
                        "$all" => "@>",
                        "$size" => "array_length",
                        _ => "=",
                    };
                    if operator == "array_length" {
                        let field = Query::format_field(field);
                        let value = self.encode_value(Some(value));
                        let condition = format!(r#"array_length({field}, 1) = {value}"#);
                        conditions.push(condition);
                    } else if operator == "IN" || operator == "NOT IN" {
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
                                    format!(r#"{field} @> {value}"#)
                                })
                                .collect::<Vec<_>>()
                                .join(" OR ")
                        } else {
                            let s = value.replace(';', ",");
                            let value = self.format_value(&s);
                            format!(r#"{field} @> {value}"#)
                        }
                    } else {
                        let value = self.format_value(value);
                        format!(r#"{field} && {value}"#)
                    }
                } else {
                    let value = self.encode_value(Some(value));
                    format!(r#"{field} && {value}"#)
                }
            }
            "Map" => {
                let field = Query::format_field(field);
                if let Some(value) = value.as_str() {
                    // JSON path operator is supported in Postgres 12+
                    let value = Query::escape_string(value);
                    format!(r#"{field} @? {value}"#)
                } else {
                    let value = self.encode_value(Some(value));
                    format!(r#"{field} @> {value}"#)
                }
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
                "BOOL" => row.try_get_unchecked::<bool, _>(index)?.into(),
                "INT2" => row.try_get_unchecked::<i16, _>(index)?.into(),
                "INT4" => row.try_get_unchecked::<i32, _>(index)?.into(),
                "INT8" => row.try_get_unchecked::<i64, _>(index)?.into(),
                "FLOAT4" => row.try_get_unchecked::<f32, _>(index)?.into(),
                "FLOAT8" => row.try_get_unchecked::<f64, _>(index)?.into(),
                "TEXT" | "VARCHAR" | "CHAR" => row.try_get_unchecked::<String, _>(index)?.into(),
                "TIMESTAMPTZ" => row.try_get_unchecked::<DateTime, _>(index)?.into(),
                "TIMESTAMP" => row
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
                "UUID" => row.try_get_unchecked::<Uuid, _>(index)?.to_string().into(),
                "BYTEA" => row.try_get_unchecked::<Vec<u8>, _>(index)?.into(),
                "TEXT[]" => row.try_get_unchecked::<Vec<String>, _>(index)?.into(),
                "UUID[]" => {
                    let values = row.try_get_unchecked::<Vec<Uuid>, _>(index)?;
                    values
                        .iter()
                        .map(|v| v.to_string())
                        .collect::<Vec<_>>()
                        .into()
                }
                "JSONB" | "JSON" => row.try_get_unchecked::<JsonValue, _>(index)?,
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
                "BOOL" => row.try_get_unchecked::<bool, _>(index)?.into(),
                "INT4" => row.try_get_unchecked::<i32, _>(index)?.into(),
                "INT8" => row.try_get_unchecked::<i64, _>(index)?.into(),
                "FLOAT4" => row.try_get_unchecked::<f32, _>(index)?.into(),
                "FLOAT8" => row.try_get_unchecked::<f64, _>(index)?.into(),
                "TEXT" | "VARCHAR" | "CHAR" => row.try_get_unchecked::<String, _>(index)?.into(),
                "TIMESTAMPTZ" => row.try_get_unchecked::<DateTime, _>(index)?.into(),
                "TIMESTAMP" => row
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
                // deserialize Avro Uuid value wasn't supported in 0.14.0
                "UUID" => row.try_get_unchecked::<Uuid, _>(index)?.to_string().into(),
                "BYTEA" => row.try_get_unchecked::<Vec<u8>, _>(index)?.into(),
                "TEXT[]" => {
                    let values = row.try_get_unchecked::<Vec<String>, _>(index)?;
                    let vec = values
                        .into_iter()
                        .map(AvroValue::String)
                        .collect::<Vec<_>>();
                    AvroValue::Array(vec)
                }
                "UUID[]" => {
                    // deserialize Avro Uuid value wasn't supported in 0.14.0
                    let values = row.try_get_unchecked::<Vec<Uuid>, _>(index)?;
                    let vec = values
                        .into_iter()
                        .map(|u| AvroValue::String(u.to_string()))
                        .collect::<Vec<_>>();
                    AvroValue::Array(vec)
                }
                "JSONB" | "JSON" => row.try_get_unchecked::<JsonValue, _>(index)?.into(),
                _ => AvroValue::Null,
            };
            record.push((field.to_owned(), value));
        }
        Ok(record)
    }
}

impl QueryExt<DatabaseDriver> for Query {
    #[inline]
    fn placeholder(n: usize) -> SharedString {
        if n == 1 {
            "$1".into()
        } else {
            format!("${n}").into()
        }
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
            format!("LIMIT {} OFFSET {}", self.limit(), self.offset())
        }
    }

    fn format_field(field: &str) -> Cow<'_, str> {
        if field.contains('.') {
            field
                .split('.')
                .map(|s| format!(r#""{s}""#))
                .collect::<Vec<_>>()
                .join(".")
                .into()
        } else {
            format!(r#""{field}""#).into()
        }
    }

    fn parse_text_search(filter: &Map) -> Option<String> {
        let fields = Validation::parse_str_array(filter.get("$fields"))?;
        Validation::parse_string(filter.get("$search")).map(|search| {
            let text = fields.join(" || ' ' || ");
            let lang = Validation::parse_string(filter.get("$language"))
                .unwrap_or_else(|| "english".into());
            format!("to_tsvector('{lang}', {text}) @@ websearch_to_tsquery('{lang}', '{search}')")
        })
    }
}
