//! Canonical CBOR for signed Ember records.
//!
//! The profile is deliberately smaller than RFC 8949 CBOR:
//!
//! - definite lengths only;
//! - shortest integer encodings only;
//! - no tags, floats, undefined values, or indefinite items;
//! - map keys are scalar values, ordered by the bytewise lexicographic order
//!   of their canonical encodings;
//! - duplicate map keys are rejected;
//! - text must be valid UTF-8;
//! - nesting depth, item count, and input length are bounded.
//!
//! [`decode`] parses a value and rejects anything whose canonical re-encoding
//! differs from the input. Shorter, non-canonical encodings are never accepted.

use std::collections::BTreeSet;

use crate::error::CborError;

/// Allocation and recursion bounds for canonical CBOR.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CborLimits {
    /// Maximum nesting depth. The top-level value is depth zero.
    pub max_depth: usize,
    /// Maximum number of parsed or encoded values, including map keys.
    pub max_items: usize,
    /// Maximum input or output length in bytes.
    pub max_input: usize,
}

impl CborLimits {
    /// Creates a bounded profile.
    #[must_use]
    pub const fn new(max_depth: usize, max_items: usize, max_input: usize) -> Self {
        Self {
            max_depth,
            max_items,
            max_input,
        }
    }

    /// Limits for a signed record body or envelope.
    #[must_use]
    pub const fn signed_record() -> Self {
        Self::new(16, 512, 65_536)
    }
}

impl Default for CborLimits {
    fn default() -> Self {
        Self::new(16, 4_096, 1_048_576)
    }
}

/// A value in the canonical CBOR profile.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Value {
    /// Unsigned integer (major type 0).
    Unsigned(u64),
    /// Negative integer (major type 1).
    Negative(i64),
    /// Byte string (major type 2).
    Bytes(Vec<u8>),
    /// UTF-8 text string (major type 3).
    Text(String),
    /// Array (major type 4).
    Array(Vec<Value>),
    /// Map (major type 5).
    Map(Vec<(Value, Value)>),
    /// Boolean.
    Bool(bool),
    /// Null.
    Null,
}

impl Value {
    /// Returns the unsigned integer value, if this is a non-negative integer.
    #[must_use]
    pub fn as_unsigned(&self) -> Option<u64> {
        match self {
            Self::Unsigned(value) => Some(*value),
            _ => None,
        }
    }

    /// Returns the integer value when it fits a signed 64-bit integer.
    #[must_use]
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Unsigned(value) => i64::try_from(*value).ok(),
            Self::Negative(value) => Some(*value),
            _ => None,
        }
    }

    /// Returns the text value.
    #[must_use]
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(value) => Some(value),
            _ => None,
        }
    }

    /// Returns the byte string value.
    #[must_use]
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Bytes(value) => Some(value),
            _ => None,
        }
    }

    /// Returns the array value.
    #[must_use]
    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Self::Array(value) => Some(value),
            _ => None,
        }
    }

    /// Returns the map entries.
    #[must_use]
    pub fn as_map(&self) -> Option<&[(Value, Value)]> {
        match self {
            Self::Map(value) => Some(value),
            _ => None,
        }
    }

    /// Returns the boolean value.
    #[must_use]
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(value) => Some(*value),
            _ => None,
        }
    }

    /// Returns true when this is the CBOR null value.
    #[must_use]
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    /// Looks up a non-negative integer key in a map.
    #[must_use]
    pub fn get_key(&self, key: u64) -> Option<&Value> {
        let Self::Map(entries) = self else {
            return None;
        };
        entries
            .iter()
            .find_map(|(candidate, value)| (candidate.as_unsigned() == Some(key)).then_some(value))
    }

    /// Encodes the value canonically with the supplied bounds.
    pub fn to_canonical_bytes_with_limits(&self, limits: CborLimits) -> Result<Vec<u8>, CborError> {
        let mut encoder = Encoder {
            limits,
            items: 0,
            output: Vec::new(),
        };
        encoder.value(self, 0)?;
        Ok(encoder.output)
    }

    /// Encodes the value canonically with default bounds.
    pub fn to_canonical_bytes(&self) -> Result<Vec<u8>, CborError> {
        self.to_canonical_bytes_with_limits(CborLimits::default())
    }
}

impl From<u64> for Value {
    fn from(value: u64) -> Self {
        Self::Unsigned(value)
    }
}

impl From<i64> for Value {
    fn from(value: i64) -> Self {
        if value >= 0 {
            Self::Unsigned(value.unsigned_abs())
        } else {
            Self::Negative(value)
        }
    }
}

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Self::Text(value.to_owned())
    }
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl From<Vec<u8>> for Value {
    fn from(value: Vec<u8>) -> Self {
        Self::Bytes(value)
    }
}

/// Decodes one canonical CBOR value and rejects trailing or non-canonical bytes.
pub fn decode(input: &[u8], limits: CborLimits) -> Result<Value, CborError> {
    if input.len() > limits.max_input {
        return Err(CborError::InputTooLarge { max: limits.max_input });
    }

    let mut decoder = Decoder {
        input,
        position: 0,
        limits,
        items: 0,
    };
    let value = decoder.value(0)?;
    if decoder.position != input.len() {
        return Err(CborError::TrailingBytes);
    }

    let canonical = value.to_canonical_bytes_with_limits(limits)?;
    if canonical != input {
        return Err(CborError::NonCanonical);
    }
    Ok(value)
}

/// Decodes one value using the signed-record bounds.
pub fn decode_signed_record(input: &[u8]) -> Result<Value, CborError> {
    decode(input, CborLimits::signed_record())
}

struct Encoder {
    limits: CborLimits,
    items: usize,
    output: Vec<u8>,
}

impl Encoder {
    fn value(&mut self, value: &Value, depth: usize) -> Result<(), CborError> {
        if depth > self.limits.max_depth {
            return Err(CborError::DepthExceeded {
                max: self.limits.max_depth,
            });
        }
        self.items = self.items.saturating_add(1);
        if self.items > self.limits.max_items {
            return Err(CborError::ItemsExceeded {
                max: self.limits.max_items,
            });
        }

        match value {
            Value::Unsigned(number) => write_type_and_length(&mut self.output, 0, *number),
            Value::Negative(number) => {
                if *number >= 0 {
                    return Err(CborError::IntegerOutOfRange);
                }
                let encoded = (-1_i128 - i128::from(*number)) as u64;
                write_type_and_length(&mut self.output, 1, encoded);
            }
            Value::Bytes(bytes) => {
                write_type_and_length(
                    &mut self.output,
                    2,
                    u64::try_from(bytes.len()).map_err(|_| CborError::InputTooLarge {
                        max: self.limits.max_input,
                    })?,
                );
                self.output.extend_from_slice(bytes);
            }
            Value::Text(text) => {
                write_type_and_length(
                    &mut self.output,
                    3,
                    u64::try_from(text.len()).map_err(|_| CborError::InputTooLarge {
                        max: self.limits.max_input,
                    })?,
                );
                self.output.extend_from_slice(text.as_bytes());
            }
            Value::Array(items) => {
                write_type_and_length(
                    &mut self.output,
                    4,
                    u64::try_from(items.len()).map_err(|_| CborError::ItemsExceeded {
                        max: self.limits.max_items,
                    })?,
                );
                for item in items {
                    self.value(item, depth + 1)?;
                }
            }
            Value::Map(entries) => {
                let mut encoded_entries = Vec::with_capacity(entries.len());
                for (key, value) in entries {
                    if matches!(key, Value::Array(_) | Value::Map(_)) {
                        return Err(CborError::UnsupportedMapKey);
                    }
                    let key_bytes = {
                        let mut key_encoder = Encoder {
                            limits: self.limits,
                            items: self.items,
                            output: Vec::new(),
                        };
                        key_encoder.value(key, depth + 1)?;
                        self.items = key_encoder.items;
                        key_encoder.output
                    };
                    let value_bytes = {
                        let mut value_encoder = Encoder {
                            limits: self.limits,
                            items: self.items,
                            output: Vec::new(),
                        };
                        value_encoder.value(value, depth + 1)?;
                        self.items = value_encoder.items;
                        value_encoder.output
                    };
                    encoded_entries.push((key_bytes, value_bytes));
                }
                encoded_entries.sort_by(|left, right| left.0.cmp(&right.0));
                for pair in encoded_entries.windows(2) {
                    if pair[0].0 == pair[1].0 {
                        return Err(CborError::DuplicateKey);
                    }
                }
                write_type_and_length(
                    &mut self.output,
                    5,
                    u64::try_from(encoded_entries.len()).map_err(|_| CborError::ItemsExceeded {
                        max: self.limits.max_items,
                    })?,
                );
                for (key, value) in encoded_entries {
                    self.output.extend_from_slice(&key);
                    self.output.extend_from_slice(&value);
                }
            }
            Value::Bool(false) => self.output.push(0xf4),
            Value::Bool(true) => self.output.push(0xf5),
            Value::Null => self.output.push(0xf6),
        }

        if self.output.len() > self.limits.max_input {
            return Err(CborError::InputTooLarge {
                max: self.limits.max_input,
            });
        }
        Ok(())
    }
}

fn write_type_and_length(output: &mut Vec<u8>, major: u8, length: u64) {
    let prefix = major << 5;
    if length < 24 {
        output.push(prefix | u8::try_from(length).unwrap_or(23));
    } else if let Ok(length) = u8::try_from(length) {
        output.push(prefix | 24);
        output.push(length);
    } else if let Ok(length) = u16::try_from(length) {
        output.push(prefix | 25);
        output.extend_from_slice(&length.to_be_bytes());
    } else if let Ok(length) = u32::try_from(length) {
        output.push(prefix | 26);
        output.extend_from_slice(&length.to_be_bytes());
    } else {
        output.push(prefix | 27);
        output.extend_from_slice(&length.to_be_bytes());
    }
}

struct Decoder<'input> {
    input: &'input [u8],
    position: usize,
    limits: CborLimits,
    items: usize,
}

impl Decoder<'_> {
    fn value(&mut self, depth: usize) -> Result<Value, CborError> {
        if depth > self.limits.max_depth {
            return Err(CborError::DepthExceeded {
                max: self.limits.max_depth,
            });
        }
        self.items = self.items.saturating_add(1);
        if self.items > self.limits.max_items {
            return Err(CborError::ItemsExceeded {
                max: self.limits.max_items,
            });
        }

        let initial = self.read_byte()?;
        let major = initial >> 5;
        let additional = initial & 0x1f;

        match major {
            0 => Ok(Value::Unsigned(self.read_length(additional)?)),
            1 => {
                let encoded = self.read_length(additional)?;
                if encoded > i64::MAX as u64 {
                    return Err(CborError::IntegerOutOfRange);
                }
                Ok(Value::Negative(-1 - encoded as i64))
            }
            2 => {
                let length = self.read_length(additional)?;
                let bytes = self.read_exact(length)?;
                Ok(Value::Bytes(bytes.to_vec()))
            }
            3 => {
                let length = self.read_length(additional)?;
                let bytes = self.read_exact(length)?;
                let text = std::str::from_utf8(bytes)
                    .map_err(|_| CborError::InvalidUtf8)?
                    .to_owned();
                Ok(Value::Text(text))
            }
            4 => {
                let length = self.read_length(additional)?;
                if length > u64::try_from(self.remaining()).unwrap_or(u64::MAX) {
                    return Err(CborError::UnexpectedEnd);
                }
                let length = usize::try_from(length).map_err(|_| CborError::IntegerOutOfRange)?;
                let mut items = Vec::with_capacity(length.min(self.limits.max_items));
                for _ in 0..length {
                    items.push(self.value(depth + 1)?);
                }
                Ok(Value::Array(items))
            }
            5 => {
                let length = self.read_length(additional)?;
                let remaining = u64::try_from(self.remaining()).unwrap_or(u64::MAX);
                if length > remaining / 2 {
                    return Err(CborError::UnexpectedEnd);
                }
                let length = usize::try_from(length).map_err(|_| CborError::IntegerOutOfRange)?;
                let mut entries = Vec::with_capacity(length.min(self.limits.max_items));
                let mut seen = BTreeSet::new();
                for _ in 0..length {
                    let key = self.value(depth + 1)?;
                    if matches!(key, Value::Array(_) | Value::Map(_)) {
                        return Err(CborError::UnsupportedMapKey);
                    }
                    let key_bytes = key.to_canonical_bytes_with_limits(self.limits)?;
                    if !seen.insert(key_bytes) {
                        return Err(CborError::DuplicateKey);
                    }
                    let value = self.value(depth + 1)?;
                    entries.push((key, value));
                }
                Ok(Value::Map(entries))
            }
            6 => Err(CborError::UnsupportedType("tag")),
            7 => match additional {
                20 => Ok(Value::Bool(false)),
                21 => Ok(Value::Bool(true)),
                22 => Ok(Value::Null),
                23 => Err(CborError::UnsupportedType("undefined")),
                24..=27 => Err(CborError::UnsupportedType("simple value or float")),
                31 => Err(CborError::IndefiniteLength),
                _ => Err(CborError::UnsupportedType("simple value")),
            },
            _ => Err(CborError::UnsupportedType("reserved major type")),
        }
    }

    fn read_length(&mut self, additional: u8) -> Result<u64, CborError> {
        match additional {
            0..=23 => Ok(u64::from(additional)),
            24 => {
                let value = u64::from(self.read_byte()?);
                if value < 24 {
                    return Err(CborError::NonShortestInteger);
                }
                Ok(value)
            }
            25 => {
                let value = u64::from(u16::from_be_bytes(self.read_array()?));
                if value <= u64::from(u8::MAX) {
                    return Err(CborError::NonShortestInteger);
                }
                Ok(value)
            }
            26 => {
                let value = u64::from(u32::from_be_bytes(self.read_array()?));
                if value <= u64::from(u16::MAX) {
                    return Err(CborError::NonShortestInteger);
                }
                Ok(value)
            }
            27 => {
                let value = u64::from_be_bytes(self.read_array()?);
                if value <= u64::from(u32::MAX) {
                    return Err(CborError::NonShortestInteger);
                }
                Ok(value)
            }
            28..=30 => Err(CborError::UnsupportedType("reserved additional information")),
            31 => Err(CborError::IndefiniteLength),
            _ => Err(CborError::UnsupportedType("invalid additional information")),
        }
    }

    fn read_byte(&mut self) -> Result<u8, CborError> {
        let byte = *self.input.get(self.position).ok_or(CborError::UnexpectedEnd)?;
        self.position += 1;
        Ok(byte)
    }

    fn read_array<const N: usize>(&mut self) -> Result<[u8; N], CborError> {
        let end = self.position.checked_add(N).ok_or(CborError::UnexpectedEnd)?;
        let bytes = self.input.get(self.position..end).ok_or(CborError::UnexpectedEnd)?;
        let array = bytes.try_into().map_err(|_| CborError::UnexpectedEnd)?;
        self.position = end;
        Ok(array)
    }

    fn read_exact(&mut self, length: u64) -> Result<&[u8], CborError> {
        let length = usize::try_from(length).map_err(|_| CborError::IntegerOutOfRange)?;
        let end = self.position.checked_add(length).ok_or(CborError::UnexpectedEnd)?;
        let bytes = self.input.get(self.position..end).ok_or(CborError::UnexpectedEnd)?;
        self.position = end;
        Ok(bytes)
    }

    fn remaining(&self) -> usize {
        self.input.len().saturating_sub(self.position)
    }
}

#[cfg(test)]
mod tests {
    use super::{CborError, CborLimits, Value, decode, decode_signed_record};

    fn unsigned(value: u64) -> Value {
        Value::Unsigned(value)
    }

    #[test]
    fn encodes_rfc_8949_integer_vectors() {
        let vectors: &[(u64, &[u8])] = &[
            (0, &[0x00]),
            (1, &[0x01]),
            (10, &[0x0a]),
            (23, &[0x17]),
            (24, &[0x18, 0x18]),
            (25, &[0x18, 0x19]),
            (100, &[0x18, 0x64]),
            (255, &[0x18, 0xff]),
            (256, &[0x19, 0x01, 0x00]),
            (1_000, &[0x19, 0x03, 0xe8]),
            (65_535, &[0x19, 0xff, 0xff]),
            (65_536, &[0x1a, 0x00, 0x01, 0x00, 0x00]),
            (1_000_000, &[0x1a, 0x00, 0x0f, 0x42, 0x40]),
            (4_294_967_296, &[0x1b, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00]),
        ];
        for (number, encoded) in vectors {
            let value = unsigned(*number);
            assert_eq!(value.to_canonical_bytes().expect("encode"), *encoded);
            assert_eq!(decode(encoded, CborLimits::default()).expect("decode"), value);
        }
    }

    #[test]
    fn encodes_negative_integers_and_scalars() {
        assert_eq!(Value::Negative(-1).to_canonical_bytes().expect("encode"), vec![0x20]);
        assert_eq!(Value::Negative(-10).to_canonical_bytes().expect("encode"), vec![0x29]);
        assert_eq!(
            Value::Negative(-100).to_canonical_bytes().expect("encode"),
            vec![0x38, 0x63]
        );
        assert_eq!(
            Value::Text("IETF".to_owned()).to_canonical_bytes().expect("encode"),
            b"\x64IETF".to_vec()
        );
        assert_eq!(
            Value::Bytes(vec![1, 2, 3, 4]).to_canonical_bytes().expect("encode"),
            vec![0x44, 1, 2, 3, 4]
        );
        assert_eq!(
            Value::Array(vec![unsigned(1), unsigned(2), unsigned(3)])
                .to_canonical_bytes()
                .expect("encode"),
            vec![0x83, 0x01, 0x02, 0x03]
        );
        assert_eq!(
            Value::Map(vec![
                (Value::Text("b".to_owned()), unsigned(2)),
                (Value::Text("a".to_owned()), unsigned(1)),
            ])
            .to_canonical_bytes()
            .expect("encode"),
            vec![0xa2, 0x61, b'a', 0x01, 0x61, b'b', 0x02]
        );
        assert_eq!(
            Value::Map(vec![(unsigned(10), Value::Bool(true)), (unsigned(1), Value::Null),])
                .to_canonical_bytes()
                .expect("encode"),
            vec![0xa2, 0x01, 0xf6, 0x0a, 0xf5]
        );
    }

    #[test]
    fn rejects_non_canonical_and_unsupported_encodings() {
        assert_eq!(
            decode(&[0x18, 0x17], CborLimits::default()),
            Err(CborError::NonShortestInteger)
        );
        assert_eq!(
            decode(&[0xa2, 0x61, b'b', 0x02, 0x61, b'a', 0x01], CborLimits::default()),
            Err(CborError::NonCanonical)
        );
        assert_eq!(
            decode(&[0xa2, 0x01, 0x01, 0x01, 0x02], CborLimits::default()),
            Err(CborError::DuplicateKey)
        );
        assert_eq!(
            decode(&[0x7f, 0x61, b'a', 0xff], CborLimits::default()),
            Err(CborError::IndefiniteLength)
        );
        assert_eq!(
            decode(&[0xc0, 0x01], CborLimits::default()),
            Err(CborError::UnsupportedType("tag"))
        );
        assert_eq!(
            decode(&[0xf7], CborLimits::default()),
            Err(CborError::UnsupportedType("undefined"))
        );
        assert_eq!(
            decode(&[0xfb, 0, 0, 0, 0, 0, 0, 0, 0], CborLimits::default()),
            Err(CborError::UnsupportedType("simple value or float"))
        );
        assert_eq!(
            decode(&[0x61, 0xff], CborLimits::default()),
            Err(CborError::InvalidUtf8)
        );
        assert_eq!(
            decode(&[0x00, 0x01], CborLimits::default()),
            Err(CborError::TrailingBytes)
        );
    }

    #[test]
    fn bounds_depth_items_and_input() {
        let mut nested = vec![0x81; 17];
        nested.push(0x00);
        assert_eq!(
            decode(&nested, CborLimits::new(16, 100, 100)),
            Err(CborError::DepthExceeded { max: 16 })
        );

        let limits = CborLimits::new(4, 2, 100);
        assert_eq!(
            decode(&[0x82, 0x01, 0x02], limits),
            Err(CborError::ItemsExceeded { max: 2 })
        );
        assert_eq!(
            decode(&[0x00], CborLimits::new(4, 10, 0)),
            Err(CborError::InputTooLarge { max: 0 })
        );
    }

    #[test]
    fn signed_record_limits_match_the_record_cap() {
        assert_eq!(CborLimits::signed_record().max_input, 65_536);
        assert_eq!(decode_signed_record(&[0x01]).expect("small value"), Value::Unsigned(1));
    }
}
