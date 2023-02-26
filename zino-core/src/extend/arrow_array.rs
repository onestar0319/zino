use crate::BoxError;
use apache_avro::{types::Value, Days, Duration, Millis, Months};
use datafusion::arrow::{
    array::{self, Array, ArrayAccessor, FixedSizeBinaryArray, FixedSizeListArray, StringArray},
    datatypes::{
        DataType, Date32Type, Date64Type, DurationMicrosecondType, DurationMillisecondType,
        DurationNanosecondType, DurationSecondType, Float32Type, Float64Type, Int16Type, Int32Type,
        Int64Type, Int8Type, IntervalDayTimeType, IntervalUnit, Time32MillisecondType,
        Time32SecondType, Time64MicrosecondType, Time64NanosecondType, TimeUnit,
        TimestampMicrosecondType, TimestampMillisecondType, TimestampNanosecondType,
        TimestampSecondType, UInt16Type, UInt32Type, UInt64Type, UInt8Type,
    },
};
use std::collections::HashMap;

/// Extension trait for [`dyn Array`](datafusion::arrow::array::Array).
pub trait ArrowArrayExt {
    /// Parses an Avro value at the index.
    fn parse_avro_value(&self, index: usize) -> Result<Value, BoxError>;
}

impl ArrowArrayExt for dyn Array {
    fn parse_avro_value(&self, index: usize) -> Result<Value, BoxError> {
        if self.is_null(index) {
            return Ok(Value::Null);
        }
        let value = match self.data_type() {
            DataType::Null => Value::Null,
            DataType::Boolean => {
                let value = array::as_boolean_array(self).value(index);
                Value::Boolean(value)
            }
            DataType::Int8 => {
                let value = array::as_primitive_array::<Int8Type>(self).value(index);
                Value::Int(value.into())
            }
            DataType::Int16 => {
                let value = array::as_primitive_array::<Int16Type>(self).value(index);
                Value::Int(value.into())
            }
            DataType::Int32 => {
                let value = array::as_primitive_array::<Int32Type>(self).value(index);
                Value::Int(value)
            }
            DataType::Int64 => {
                let value = array::as_primitive_array::<Int64Type>(self).value(index);
                Value::Long(value)
            }
            DataType::UInt8 => {
                let value = array::as_primitive_array::<UInt8Type>(self).value(index);
                Value::Int(value.into())
            }
            DataType::UInt16 => {
                let value = array::as_primitive_array::<UInt16Type>(self).value(index);
                Value::Int(value.into())
            }
            DataType::UInt32 => {
                let value = array::as_primitive_array::<UInt32Type>(self).value(index);
                Value::Int(value.try_into()?)
            }
            DataType::UInt64 => {
                let value = array::as_primitive_array::<UInt64Type>(self).value(index);
                Value::Long(value.try_into()?)
            }
            DataType::Float32 => {
                let value = array::as_primitive_array::<Float32Type>(self).value(index);
                Value::Float(value)
            }
            DataType::Float64 => {
                let value = array::as_primitive_array::<Float64Type>(self).value(index);
                Value::Double(value)
            }
            DataType::Utf8 => {
                let value = array::as_string_array(self).value(index);
                Value::String(value.to_owned())
            }
            DataType::LargeUtf8 => {
                let value = array::as_largestring_array(self).value(index);
                Value::String(value.to_owned())
            }
            DataType::Date32 => {
                let value = array::as_primitive_array::<Date32Type>(self).value(index);
                Value::Date(value)
            }
            DataType::Date64 => {
                let value = array::as_primitive_array::<Date64Type>(self).value(index);
                Value::TimestampMillis(value)
            }
            DataType::Time32(TimeUnit::Second) => {
                let value = array::as_primitive_array::<Time32SecondType>(self).value(index);
                Value::TimeMillis(value * 1000)
            }
            DataType::Time32(TimeUnit::Millisecond) => {
                let value = array::as_primitive_array::<Time32MillisecondType>(self).value(index);
                Value::TimeMillis(value)
            }
            DataType::Time64(TimeUnit::Microsecond) => {
                let value = array::as_primitive_array::<Time64MicrosecondType>(self).value(index);
                Value::TimeMicros(value)
            }
            DataType::Time64(TimeUnit::Nanosecond) => {
                let value = array::as_primitive_array::<Time64NanosecondType>(self).value(index);
                Value::TimeMicros(value / 1000)
            }
            DataType::Timestamp(TimeUnit::Second, None) => {
                let value = array::as_primitive_array::<TimestampSecondType>(self).value(index);
                Value::TimestampMillis(value * 1000)
            }
            DataType::Timestamp(TimeUnit::Millisecond, None) => {
                let value =
                    array::as_primitive_array::<TimestampMillisecondType>(self).value(index);
                Value::TimestampMillis(value)
            }
            DataType::Timestamp(TimeUnit::Microsecond, None) => {
                let value =
                    array::as_primitive_array::<TimestampMicrosecondType>(self).value(index);
                Value::TimestampMicros(value)
            }
            DataType::Timestamp(TimeUnit::Nanosecond, None) => {
                let value = array::as_primitive_array::<TimestampNanosecondType>(self).value(index);
                Value::TimestampMicros(value / 1000)
            }
            DataType::Duration(TimeUnit::Second) => {
                let value = array::as_primitive_array::<DurationSecondType>(self).value(index);
                Value::Duration(Duration::new(
                    Months::new(0),
                    Days::new(0),
                    Millis::new((value * 1000).try_into()?),
                ))
            }
            DataType::Duration(TimeUnit::Millisecond) => {
                let value = array::as_primitive_array::<DurationMillisecondType>(self).value(index);
                Value::Duration(Duration::new(
                    Months::new(0),
                    Days::new(0),
                    Millis::new(value.try_into()?),
                ))
            }
            DataType::Duration(TimeUnit::Microsecond) => {
                let value = array::as_primitive_array::<DurationMicrosecondType>(self).value(index);
                Value::Duration(Duration::new(
                    Months::new(0),
                    Days::new(0),
                    Millis::new((value / 1000).try_into()?),
                ))
            }
            DataType::Duration(TimeUnit::Nanosecond) => {
                let value = array::as_primitive_array::<DurationNanosecondType>(self).value(index);
                Value::Duration(Duration::new(
                    Months::new(0),
                    Days::new(0),
                    Millis::new((value / 1000000).try_into()?),
                ))
            }
            DataType::Interval(IntervalUnit::DayTime) => {
                let value = array::as_primitive_array::<IntervalDayTimeType>(self).value(index);
                let (days, millis) = IntervalDayTimeType::to_parts(value);
                Value::Duration(Duration::new(
                    Months::new(0),
                    Days::new(days.try_into()?),
                    Millis::new(millis.try_into()?),
                ))
            }
            DataType::Binary => {
                let value = array::as_generic_binary_array::<i32>(self).value(index);
                Value::Bytes(value.to_vec())
            }
            DataType::LargeBinary => {
                let value = array::as_generic_binary_array::<i64>(self).value(index);
                Value::Bytes(value.to_vec())
            }
            DataType::FixedSizeBinary(_size) => {
                let fixed_size_array = array::downcast_array::<FixedSizeBinaryArray>(self);
                let value = fixed_size_array.value(index).to_vec();
                Value::Fixed(value.len(), value)
            }
            DataType::List(_field) => {
                let array = array::as_list_array(self).value(index);
                let array_length = array.len();
                let mut values = Vec::with_capacity(array_length);
                for i in 0..array_length {
                    let value = array.parse_avro_value(i)?;
                    values.push(value);
                }
                Value::Array(values)
            }
            DataType::LargeList(_field) => {
                let array = array::as_large_list_array(self).value(index);
                let array_length = array.len();
                let mut values = Vec::with_capacity(array_length);
                for i in 0..array_length {
                    let value = array.parse_avro_value(i)?;
                    values.push(value);
                }
                Value::Array(values)
            }
            DataType::FixedSizeList(_field, _size) => {
                let array = array::downcast_array::<FixedSizeListArray>(self).value(index);
                let array_length = array.len();
                let mut values = Vec::with_capacity(array_length);
                for i in 0..array_length {
                    let value = array.parse_avro_value(i)?;
                    values.push(value);
                }
                Value::Array(values)
            }
            DataType::Map(_field, _sorted) => {
                let map_array = array::as_map_array(self);
                let keys = map_array.keys();
                let values = map_array.value(index);
                let num_keys = keys.len();
                let mut hashmap = HashMap::with_capacity(num_keys);
                for i in 0..num_keys {
                    if let Value::String(key) = keys.parse_avro_value(i)? {
                        let value = values.parse_avro_value(i)?;
                        hashmap.insert(key, value);
                    } else {
                        let key_type = map_array.key_type();
                        return Err(format!("Avro map does not support `{key_type}` keys ").into());
                    }
                }
                Value::Map(hashmap)
            }
            DataType::Struct(_fields) => {
                let struct_array = array::as_struct_array(self);
                let column_names = struct_array.column_names();
                let columns = struct_array.columns();
                let num_columns = struct_array.num_columns();
                let mut hashmap = HashMap::with_capacity(num_columns);
                for i in 0..num_columns {
                    let key = column_names[i].to_owned();
                    let value = columns[i].parse_avro_value(index)?;
                    hashmap.insert(key, value);
                }
                Value::Map(hashmap)
            }
            DataType::Union(_fields, types, _mode) => {
                let union_array = array::as_union_array(self);
                let type_id = union_array.type_id(index);
                let position = types.iter().position(|&ty| type_id == ty).ok_or_else(|| {
                    let type_names = union_array.type_names();
                    format!("invalid slot `{type_id}` for the union types `{type_names:?}`")
                })?;
                let value = union_array.value(index).parse_avro_value(0)?;
                Value::Union(position.try_into()?, Box::new(value))
            }
            DataType::Dictionary(key_type, value_type)
                if key_type == &Box::new(DataType::UInt32)
                    && value_type == &Box::new(DataType::Utf8) =>
            {
                let dictionary_array = array::as_dictionary_array::<UInt32Type>(self);
                let string_array = dictionary_array
                    .downcast_dict::<StringArray>()
                    .ok_or_else(|| "fail to downcast the dictionary to string array")?;
                let value = string_array.value(index);
                let position = dictionary_array
                    .lookup_key(value)
                    .ok_or_else(|| format!("value `{value}` is not in the dictionary"))?;
                Value::Enum(position.try_into()?, value.to_owned())
            }
            data_type => {
                return Err(format!(
                    "conversion of the `{data_type}` value to an Avro value is unsupported"
                )
                .into())
            }
        };
        Ok(value)
    }
}
