//! Versioned CBOR request, response, and event envelopes.

use nexus_crypto::{CborError, CborLimits, Value, decode};

use crate::error::{DomainCode, DomainError};

/// Envelope schema version.
pub const ENVELOPE_VERSION: u64 = 1;
/// Maximum request CBOR size.
pub const MAX_REQUEST_BYTES: usize = 1024 * 1024;
/// Maximum response CBOR size.
pub const MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;
/// Maximum event CBOR size.
pub const MAX_EVENT_BYTES: usize = 1024 * 1024;
/// Maximum CBOR nesting depth.
pub const MAX_CBOR_DEPTH: usize = 16;
/// Maximum entries in any CBOR map.
pub const MAX_CBOR_MAP_ENTRIES: usize = 256;
const MAX_CBOR_ITEMS: usize = 8_192;

const REQUEST_LIMITS: CborLimits = CborLimits::new(MAX_CBOR_DEPTH, MAX_CBOR_ITEMS, MAX_REQUEST_BYTES);
pub(crate) const RESPONSE_LIMITS: CborLimits = CborLimits::new(MAX_CBOR_DEPTH, MAX_CBOR_ITEMS, MAX_RESPONSE_BYTES);
const EVENT_LIMITS: CborLimits = CborLimits::new(MAX_CBOR_DEPTH, MAX_CBOR_ITEMS, MAX_EVENT_BYTES);

/// A validated request envelope.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Request {
    /// Method parameters. Missing `params` is normalized to an empty map.
    pub params: Value,
}

/// A decoded success or failure response.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Response {
    /// Successful method result.
    Success(Value),
    /// Domain failure.
    Failure(DomainError),
}

/// Envelope encoding or decoding failure.
#[derive(Debug, thiserror::Error)]
pub enum EnvelopeError {
    /// Input exceeded the envelope size limit.
    #[error("envelope is {actual} bytes, above the {max}-byte limit")]
    TooLarge {
        /// Observed size.
        actual: usize,
        /// Maximum size.
        max: usize,
    },
    /// The envelope was not a map.
    #[error("envelope must be a CBOR map")]
    NotMap,
    /// A top-level key was not text.
    #[error("envelope keys must be text")]
    NonTextKey,
    /// A required field was missing.
    #[error("envelope is missing field {0}")]
    Missing(&'static str),
    /// A field had the wrong type.
    #[error("envelope field {0} has the wrong type")]
    WrongType(&'static str),
    /// The version was not supported.
    #[error("envelope version {0} is not supported")]
    UnsupportedVersion(u64),
    /// A map exceeded the entry limit.
    #[error("CBOR map has {actual} entries, above the {max}-entry limit")]
    MapTooLarge {
        /// Observed entries.
        actual: usize,
        /// Maximum entries.
        max: usize,
    },
    /// Nesting exceeded the depth limit.
    #[error("CBOR nesting exceeds {0} levels")]
    TooDeep(usize),
    /// Canonical CBOR decoding failed.
    #[error("envelope CBOR is invalid: {0}")]
    Malformed(#[from] CborError),
}

impl Request {
    /// Decodes and validates a request envelope.
    pub fn decode(bytes: &[u8]) -> Result<Self, EnvelopeError> {
        if bytes.len() > MAX_REQUEST_BYTES {
            return Err(EnvelopeError::TooLarge {
                actual: bytes.len(),
                max: MAX_REQUEST_BYTES,
            });
        }
        let value = decode(bytes, REQUEST_LIMITS)?;
        validate_limits(&value, 0)?;
        let entries = value.as_map().ok_or(EnvelopeError::NotMap)?;
        require_text_keys(entries)?;

        let version = get(entries, "v")
            .ok_or(EnvelopeError::Missing("v"))?
            .as_unsigned()
            .ok_or(EnvelopeError::WrongType("v"))?;
        if version != ENVELOPE_VERSION {
            return Err(EnvelopeError::UnsupportedVersion(version));
        }

        let params = match get(entries, "params") {
            Some(value) => {
                if value.as_map().is_none() {
                    return Err(EnvelopeError::WrongType("params"));
                }
                value.clone()
            }
            None => Value::Map(Vec::new()),
        };
        Ok(Self { params })
    }
}

/// Encodes a successful response.
pub fn encode_success(result: Value) -> Result<Vec<u8>, EnvelopeError> {
    let bytes = Value::Map(vec![
        (Value::Text("v".to_owned()), Value::Unsigned(ENVELOPE_VERSION)),
        (Value::Text("ok".to_owned()), Value::Bool(true)),
        (Value::Text("result".to_owned()), result),
    ])
    .to_canonical_bytes_with_limits(RESPONSE_LIMITS)?;
    check_size(bytes, MAX_RESPONSE_BYTES)
}

/// Encodes a domain error response.
pub fn encode_failure(error: &DomainError) -> Result<Vec<u8>, EnvelopeError> {
    let bytes = Value::Map(vec![
        (Value::Text("v".to_owned()), Value::Unsigned(ENVELOPE_VERSION)),
        (Value::Text("ok".to_owned()), Value::Bool(false)),
        (Value::Text("error".to_owned()), error_value(error)),
    ])
    .to_canonical_bytes_with_limits(RESPONSE_LIMITS)?;
    check_size(bytes, MAX_RESPONSE_BYTES)
}

/// Decodes a response envelope. Used by native contract tests and diagnostics.
pub fn decode_response(bytes: &[u8]) -> Result<Response, EnvelopeError> {
    if bytes.len() > MAX_RESPONSE_BYTES {
        return Err(EnvelopeError::TooLarge {
            actual: bytes.len(),
            max: MAX_RESPONSE_BYTES,
        });
    }
    let value = decode(bytes, RESPONSE_LIMITS)?;
    validate_limits(&value, 0)?;
    let entries = value.as_map().ok_or(EnvelopeError::NotMap)?;
    require_text_keys(entries)?;
    let version = get(entries, "v")
        .ok_or(EnvelopeError::Missing("v"))?
        .as_unsigned()
        .ok_or(EnvelopeError::WrongType("v"))?;
    if version != ENVELOPE_VERSION {
        return Err(EnvelopeError::UnsupportedVersion(version));
    }
    let ok = get(entries, "ok")
        .ok_or(EnvelopeError::Missing("ok"))?
        .as_bool()
        .ok_or(EnvelopeError::WrongType("ok"))?;
    if ok {
        let result = get(entries, "result").ok_or(EnvelopeError::Missing("result"))?.clone();
        return Ok(Response::Success(result));
    }
    let error = get(entries, "error").ok_or(EnvelopeError::Missing("error"))?;
    let error_entries = error.as_map().ok_or(EnvelopeError::WrongType("error"))?;
    require_text_keys(error_entries)?;
    let code = get(error_entries, "code")
        .ok_or(EnvelopeError::Missing("code"))?
        .as_text()
        .ok_or(EnvelopeError::WrongType("code"))?;
    let code = DomainCode::parse(code).ok_or(EnvelopeError::WrongType("code"))?;
    let message = get(error_entries, "message")
        .ok_or(EnvelopeError::Missing("message"))?
        .as_text()
        .ok_or(EnvelopeError::WrongType("message"))?;
    let retryable = get(error_entries, "retryable")
        .ok_or(EnvelopeError::Missing("retryable"))?
        .as_bool()
        .ok_or(EnvelopeError::WrongType("retryable"))?;
    let details = get(error_entries, "details").cloned();
    let mut decoded = DomainError::new(code, message.to_owned());
    decoded.retryable = retryable;
    decoded.details = details;
    Ok(Response::Failure(decoded))
}

/// Builds the reusable CBOR error object used by responses and events.
#[must_use]
pub fn error_value(error: &DomainError) -> Value {
    let mut entries = vec![
        (
            Value::Text("code".to_owned()),
            Value::Text(error.code.as_str().to_owned()),
        ),
        (Value::Text("message".to_owned()), Value::Text(error.message.clone())),
        (Value::Text("retryable".to_owned()), Value::Bool(error.retryable)),
    ];
    if let Some(details) = &error.details {
        entries.push((Value::Text("details".to_owned()), details.clone()));
    }
    Value::Map(entries)
}

/// Encodes an event map after the queue has assigned its sequence and time.
pub(crate) fn encode_event(event: &crate::event::Event) -> Result<Vec<u8>, EnvelopeError> {
    let bytes = Value::Map(vec![
        (Value::Text("v".to_owned()), Value::Unsigned(ENVELOPE_VERSION)),
        (Value::Text("seq".to_owned()), Value::Unsigned(event.seq)),
        (Value::Text("ts".to_owned()), Value::Unsigned(event.timestamp_ms)),
        (Value::Text("type".to_owned()), Value::Text(event.event_type.clone())),
        (
            Value::Text("op_id".to_owned()),
            event.op_id.map_or(Value::Null, Value::Unsigned),
        ),
        (
            Value::Text("download_id".to_owned()),
            event.download_id.clone().map_or(Value::Null, Value::Text),
        ),
        (Value::Text("data".to_owned()), event.data.clone()),
    ])
    .to_canonical_bytes_with_limits(EVENT_LIMITS)?;
    size_check(bytes, MAX_EVENT_BYTES)
}

fn get<'a>(entries: &'a [(Value, Value)], key: &str) -> Option<&'a Value> {
    entries
        .iter()
        .find_map(|(candidate, value)| (candidate.as_text() == Some(key)).then_some(value))
}

fn require_text_keys(entries: &[(Value, Value)]) -> Result<(), EnvelopeError> {
    if entries.iter().all(|(key, _)| key.as_text().is_some()) {
        Ok(())
    } else {
        Err(EnvelopeError::NonTextKey)
    }
}

fn validate_limits(value: &Value, depth: usize) -> Result<(), EnvelopeError> {
    if depth > MAX_CBOR_DEPTH {
        return Err(EnvelopeError::TooDeep(MAX_CBOR_DEPTH));
    }
    match value {
        Value::Map(entries) => {
            if entries.len() > MAX_CBOR_MAP_ENTRIES {
                return Err(EnvelopeError::MapTooLarge {
                    actual: entries.len(),
                    max: MAX_CBOR_MAP_ENTRIES,
                });
            }
            for (key, value) in entries {
                validate_limits(key, depth + 1)?;
                validate_limits(value, depth + 1)?;
            }
        }
        Value::Array(items) => {
            for item in items {
                validate_limits(item, depth + 1)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn check_size(bytes: Vec<u8>, max: usize) -> Result<Vec<u8>, EnvelopeError> {
    size_check(bytes, max)
}

fn size_check(bytes: Vec<u8>, max: usize) -> Result<Vec<u8>, EnvelopeError> {
    if bytes.len() > max {
        return Err(EnvelopeError::TooLarge {
            actual: bytes.len(),
            max,
        });
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use nexus_crypto::Value;

    use super::{MAX_CBOR_MAP_ENTRIES, Request, Response, decode_response, encode_failure, encode_success};
    use crate::error::{DomainCode, DomainError};

    fn request_bytes(params: Value) -> Vec<u8> {
        Value::Map(vec![
            (Value::Text("v".to_owned()), Value::Unsigned(1)),
            (Value::Text("params".to_owned()), params),
        ])
        .to_canonical_bytes()
        .expect("request")
    }

    #[test]
    fn request_and_response_round_trip() {
        let request = Request::decode(&request_bytes(Value::Map(vec![(
            Value::Text("payload".to_owned()),
            Value::Text("hello".to_owned()),
        )])))
        .expect("request");
        assert_eq!(
            request.params,
            Value::Map(vec![(
                Value::Text("payload".to_owned()),
                Value::Text("hello".to_owned())
            )])
        );

        let response = encode_success(Value::Map(vec![(
            Value::Text("payload".to_owned()),
            Value::Text("hello".to_owned()),
        )]))
        .expect("response");
        assert_eq!(
            decode_response(&response).expect("decode"),
            Response::Success(Value::Map(vec![(
                Value::Text("payload".to_owned()),
                Value::Text("hello".to_owned())
            )]))
        );
    }

    #[test]
    fn failure_round_trips_without_details() {
        let bytes = encode_failure(&DomainError::new(
            DomainCode::QuotaExceeded,
            "Not enough managed storage for this file.",
        ))
        .expect("failure");
        let response = decode_response(&bytes).expect("decode");
        match response {
            Response::Failure(error) => {
                assert_eq!(error.code, DomainCode::QuotaExceeded);
                assert!(!error.retryable);
                assert!(error.details.is_none());
            }
            Response::Success(_) => panic!("expected failure"),
        }
    }

    #[test]
    fn rejects_non_canonical_versions_and_oversized_maps() {
        let bytes = Value::Map(vec![
            (Value::Text("v".to_owned()), Value::Unsigned(2)),
            (Value::Text("params".to_owned()), Value::Map(Vec::new())),
        ])
        .to_canonical_bytes()
        .expect("request");
        assert!(Request::decode(&bytes).is_err());

        let entries = (0..=MAX_CBOR_MAP_ENTRIES)
            .map(|index| {
                (
                    Value::Text(format!("k{index}")),
                    Value::Unsigned(u64::try_from(index).expect("index")),
                )
            })
            .collect();
        let bytes = Value::Map(vec![
            (Value::Text("v".to_owned()), Value::Unsigned(1)),
            (Value::Text("params".to_owned()), Value::Map(entries)),
        ])
        .to_canonical_bytes()
        .expect("request");
        assert!(Request::decode(&bytes).is_err());
    }
}
