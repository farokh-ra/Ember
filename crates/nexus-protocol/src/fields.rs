//! Shared signed-field parsing helpers.

use nexus_crypto::Value;

use crate::error::ProtocolError;

pub(crate) fn map(value: &Value) -> Result<&[(Value, Value)], ProtocolError> {
    value.as_map().ok_or(ProtocolError::InvalidField(0))
}

pub(crate) fn required(value: &Value, key: u64) -> Result<&Value, ProtocolError> {
    value.get_key(key).ok_or(ProtocolError::MissingField(key))
}

pub(crate) fn unsigned(value: &Value, key: u64) -> Result<u64, ProtocolError> {
    required(value, key)?
        .as_unsigned()
        .ok_or(ProtocolError::InvalidField(key))
}

pub(crate) fn text(value: &Value, key: u64) -> Result<&str, ProtocolError> {
    required(value, key)?.as_text().ok_or(ProtocolError::InvalidField(key))
}

pub(crate) fn bytes(value: &Value, key: u64) -> Result<&[u8], ProtocolError> {
    required(value, key)?.as_bytes().ok_or(ProtocolError::InvalidField(key))
}

pub(crate) fn bytes32(value: &Value, key: u64) -> Result<[u8; 32], ProtocolError> {
    bytes(value, key)?
        .try_into()
        .map_err(|_| ProtocolError::InvalidField(key))
}

pub(crate) fn nullable_bytes32(value: &Value, key: u64) -> Result<Option<[u8; 32]>, ProtocolError> {
    let field = required(value, key)?;
    if field.is_null() {
        return Ok(None);
    }
    field
        .as_bytes()
        .ok_or(ProtocolError::InvalidField(key))?
        .try_into()
        .map(Some)
        .map_err(|_| ProtocolError::InvalidField(key))
}

pub(crate) fn array(value: &Value, key: u64) -> Result<&[Value], ProtocolError> {
    required(value, key)?.as_array().ok_or(ProtocolError::InvalidField(key))
}

pub(crate) fn nonnegative_keys(value: &Value) -> Result<Vec<u64>, ProtocolError> {
    let entries = map(value)?;
    let mut keys = Vec::with_capacity(entries.len());
    for (key, _) in entries {
        let key = key.as_unsigned().ok_or(ProtocolError::InvalidFieldKey)?;
        keys.push(key);
    }
    Ok(keys)
}
