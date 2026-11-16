//! Bounded CBOR bodies for the Stage 09 transport-check protocol.
//!
//! `/nexus/test/1` is a diagnostic-only request/response protocol used to prove
//! that two authenticated peers can exchange a bounded message. Product record,
//! manifest, and chunk protocols keep their own IDs and schemas in later
//! stages.

use nexus_crypto::{CborLimits, Value, decode};

use crate::error::ProtocolError;

/// Wire version for the transport-check messages.
pub const TEST_WIRE_VERSION: u64 = 1;
/// Maximum request or response payload.
pub const MAX_TEST_PAYLOAD_BYTES: usize = 4 * 1024;
/// Maximum encoded request or response size.
pub const MAX_TEST_MESSAGE_BYTES: usize = 8 * 1024;
/// Maximum length of a typed error code in a response.
pub const MAX_TEST_ERROR_BYTES: usize = 64;

const LIMITS: CborLimits = CborLimits::new(4, 16, MAX_TEST_MESSAGE_BYTES);

/// A bounded transport-check request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TestRequest {
    /// Opaque payload that the peer echoes back.
    pub payload: Vec<u8>,
}

/// A bounded transport-check response.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TestResponse {
    /// Echoed payload when the request succeeded.
    pub payload: Option<Vec<u8>>,
    /// Stable error code when the request was rejected.
    pub error: Option<String>,
}

/// Encodes a transport-check request as canonical CBOR.
pub fn encode_test_request(payload: &[u8]) -> Result<Vec<u8>, ProtocolError> {
    if payload.len() > MAX_TEST_PAYLOAD_BYTES {
        return Err(ProtocolError::InvalidWireMessage);
    }
    let value = Value::Map(vec![
        (Value::Text("v".to_owned()), Value::Unsigned(TEST_WIRE_VERSION)),
        (Value::Text("payload".to_owned()), Value::Bytes(payload.to_vec())),
    ]);
    encode(value)
}

/// Decodes a transport-check request, rejecting unknown fields and versions.
pub fn decode_test_request(bytes: &[u8]) -> Result<TestRequest, ProtocolError> {
    let entries = decode_map(bytes)?;
    let mut version = None;
    let mut payload = None;
    for (key, value) in entries {
        match key.as_text() {
            Some("v") => version = Some(value.as_unsigned().ok_or(ProtocolError::InvalidWireMessage)?),
            Some("payload") => {
                payload = Some(value.as_bytes().ok_or(ProtocolError::InvalidWireMessage)?.to_vec());
            }
            _ => return Err(ProtocolError::InvalidWireMessage),
        }
    }
    if version != Some(TEST_WIRE_VERSION) {
        return Err(ProtocolError::InvalidWireMessage);
    }
    let payload = payload.ok_or(ProtocolError::InvalidWireMessage)?;
    if payload.len() > MAX_TEST_PAYLOAD_BYTES {
        return Err(ProtocolError::InvalidWireMessage);
    }
    Ok(TestRequest { payload })
}

/// Encodes a transport-check response as canonical CBOR.
pub fn encode_test_response(response: &TestResponse) -> Result<Vec<u8>, ProtocolError> {
    let mut entries = vec![(Value::Text("v".to_owned()), Value::Unsigned(TEST_WIRE_VERSION))];
    match (&response.payload, &response.error) {
        (Some(payload), None) => {
            if payload.len() > MAX_TEST_PAYLOAD_BYTES {
                return Err(ProtocolError::InvalidWireMessage);
            }
            entries.push((Value::Text("payload".to_owned()), Value::Bytes(payload.clone())));
        }
        (None, Some(error)) => {
            if error.is_empty() || error.len() > MAX_TEST_ERROR_BYTES || !error.is_ascii() {
                return Err(ProtocolError::InvalidWireMessage);
            }
            entries.push((Value::Text("error".to_owned()), Value::Text(error.clone())));
        }
        _ => return Err(ProtocolError::InvalidWireMessage),
    }
    encode(Value::Map(entries))
}

/// Decodes a transport-check response, rejecting unknown fields and versions.
pub fn decode_test_response(bytes: &[u8]) -> Result<TestResponse, ProtocolError> {
    let entries = decode_map(bytes)?;
    let mut version = None;
    let mut payload = None;
    let mut error = None;
    for (key, value) in entries {
        match key.as_text() {
            Some("v") => version = Some(value.as_unsigned().ok_or(ProtocolError::InvalidWireMessage)?),
            Some("payload") => {
                payload = Some(value.as_bytes().ok_or(ProtocolError::InvalidWireMessage)?.to_vec());
            }
            Some("error") => {
                error = Some(value.as_text().ok_or(ProtocolError::InvalidWireMessage)?.to_owned());
            }
            _ => return Err(ProtocolError::InvalidWireMessage),
        }
    }
    if version != Some(TEST_WIRE_VERSION) {
        return Err(ProtocolError::InvalidWireMessage);
    }
    match (payload, error) {
        (Some(payload), None) if payload.len() <= MAX_TEST_PAYLOAD_BYTES => Ok(TestResponse {
            payload: Some(payload),
            error: None,
        }),
        (None, Some(error)) if !error.is_empty() && error.len() <= MAX_TEST_ERROR_BYTES && error.is_ascii() => {
            Ok(TestResponse {
                payload: None,
                error: Some(error),
            })
        }
        _ => Err(ProtocolError::InvalidWireMessage),
    }
}

fn encode(value: Value) -> Result<Vec<u8>, ProtocolError> {
    let bytes = value.to_canonical_bytes_with_limits(LIMITS)?;
    if bytes.len() > MAX_TEST_MESSAGE_BYTES {
        return Err(ProtocolError::InvalidWireMessage);
    }
    Ok(bytes)
}

fn decode_map(bytes: &[u8]) -> Result<Vec<(Value, Value)>, ProtocolError> {
    if bytes.is_empty() || bytes.len() > MAX_TEST_MESSAGE_BYTES {
        return Err(ProtocolError::InvalidWireMessage);
    }
    match decode(bytes, LIMITS)? {
        Value::Map(entries) => Ok(entries),
        _ => Err(ProtocolError::InvalidWireMessage),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_TEST_ERROR_BYTES, MAX_TEST_MESSAGE_BYTES, MAX_TEST_PAYLOAD_BYTES, TestRequest, TestResponse,
        decode_test_request, decode_test_response, encode_test_request, encode_test_response,
    };

    #[test]
    fn request_round_trips_within_bounds() {
        let request = TestRequest {
            payload: b"ember-stage-09".to_vec(),
        };
        let bytes = encode_test_request(&request.payload).expect("encode");
        assert!(bytes.len() < MAX_TEST_MESSAGE_BYTES);
        assert_eq!(decode_test_request(&bytes).expect("decode"), request);

        let empty = encode_test_request(&[]).expect("empty encode");
        assert_eq!(
            decode_test_request(&empty).expect("empty decode").payload,
            Vec::<u8>::new()
        );
    }

    #[test]
    fn response_round_trips_success_and_error() {
        let success = TestResponse {
            payload: Some(b"pong".to_vec()),
            error: None,
        };
        let bytes = encode_test_response(&success).expect("encode");
        assert_eq!(decode_test_response(&bytes).expect("decode"), success);

        let failure = TestResponse {
            payload: None,
            error: Some("rate_limited".to_owned()),
        };
        let bytes = encode_test_response(&failure).expect("encode");
        assert_eq!(decode_test_response(&bytes).expect("decode"), failure);
    }

    #[test]
    fn rejects_oversized_payloads_before_encoding() {
        assert!(encode_test_request(&vec![1_u8; MAX_TEST_PAYLOAD_BYTES + 1]).is_err());
        assert!(
            encode_test_response(&TestResponse {
                payload: Some(vec![1_u8; MAX_TEST_PAYLOAD_BYTES + 1]),
                error: None,
            })
            .is_err()
        );
        assert!(
            encode_test_response(&TestResponse {
                payload: None,
                error: Some("x".repeat(MAX_TEST_ERROR_BYTES + 1)),
            })
            .is_err()
        );
    }

    #[test]
    fn rejects_unknown_versions_fields_and_shapes() {
        let request = encode_test_request(b"payload").expect("encode");

        let mut wrong_version = request.clone();
        let index = wrong_version.iter().position(|byte| *byte == 1).expect("version byte");
        wrong_version[index] = 2;
        assert!(decode_test_request(&wrong_version).is_err());

        let unknown_field = nexus_crypto::Value::Map(vec![
            (
                nexus_crypto::Value::Text("v".to_owned()),
                nexus_crypto::Value::Unsigned(1),
            ),
            (
                nexus_crypto::Value::Text("payload".to_owned()),
                nexus_crypto::Value::Bytes(b"x".to_vec()),
            ),
            (
                nexus_crypto::Value::Text("extra".to_owned()),
                nexus_crypto::Value::Bool(true),
            ),
        ])
        .to_canonical_bytes()
        .expect("encode unknown field");
        assert!(decode_test_request(&unknown_field).is_err());

        assert!(
            decode_test_response(
                &nexus_crypto::Value::Map(vec![
                    (
                        nexus_crypto::Value::Text("v".to_owned()),
                        nexus_crypto::Value::Unsigned(1),
                    ),
                    (
                        nexus_crypto::Value::Text("payload".to_owned()),
                        nexus_crypto::Value::Bytes(b"x".to_vec()),
                    ),
                    (
                        nexus_crypto::Value::Text("error".to_owned()),
                        nexus_crypto::Value::Text("both".to_owned()),
                    ),
                ])
                .to_canonical_bytes()
                .expect("encode both"),
            )
            .is_err()
        );
    }

    #[test]
    fn rejects_truncated_and_oversized_messages() {
        assert!(decode_test_request(&[]).is_err());
        assert!(decode_test_request(&vec![0_u8; MAX_TEST_MESSAGE_BYTES + 1]).is_err());
        assert!(
            decode_test_response(
                &nexus_crypto::Value::Map(vec![(
                    nexus_crypto::Value::Text("v".to_owned()),
                    nexus_crypto::Value::Unsigned(1),
                )])
                .to_canonical_bytes()
                .expect("encode")
            )
            .is_err()
        );
    }
}
