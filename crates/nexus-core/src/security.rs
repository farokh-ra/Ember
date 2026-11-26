//! Local blocking, abuse reports, and safety notices.
//!
//! Blocking and reporting are local policy decisions. Ember deliberately has no
//! central moderation authority: entries stay in the local store, are never
//! uploaded, and are never included in signed records. A blocked peer is
//! refused by the network layer and excluded from discovery and download
//! sources.

use nexus_crypto::Value;
use nexus_store::{AbuseCategory, AbuseReport, BlockReason, BlockedPeer, MAX_SECURITY_NOTE_BYTES};

use crate::engine::{Engine, MethodOutcome};
use crate::error::CoreError;

/// Default number of abuse reports returned by one listing.
const DEFAULT_REPORT_LIMIT: u64 = 50;
/// Largest page of abuse reports returned by one listing.
const MAX_REPORT_LIMIT: u64 = 200;

impl Engine {
    pub(crate) fn security_block_list(&self, _params: &Value) -> Result<Value, CoreError> {
        self.require_running()?;
        let peers = self.store.blocked_peers()?;
        Ok(Value::Map(vec![
            (
                Value::Text("count".to_owned()),
                Value::Unsigned(u64::try_from(peers.len()).unwrap_or(u64::MAX)),
            ),
            (Value::Text("cap".to_owned()), Value::Unsigned(block_cap(&self.store))),
            (
                Value::Text("peers".to_owned()),
                Value::Array(peers.iter().map(blocked_peer_value).collect()),
            ),
        ]))
    }

    pub(crate) fn security_block_add(&self, params: &Value) -> Result<MethodOutcome, CoreError> {
        self.require_running()?;
        let peer_id = required_text(params, "peer_id")?;
        let reason = match optional_text(params, "reason")? {
            Some(reason) => {
                BlockReason::parse(&reason).ok_or(CoreError::InvalidArgument("reason is not a block reason"))?
            }
            None => BlockReason::Manual,
        };
        let note = bounded_note(params)?;
        let blocked = self.store.block_peer(peer_id, reason, note.as_deref())?;
        self.refresh_network_blocklist();
        self.try_emit(super::event::PendingEvent::durable(
            "security.peer_blocked",
            blocked_peer_value(&blocked),
        ));
        Ok(MethodOutcome::Result(blocked_peer_value(&blocked)))
    }

    pub(crate) fn security_block_remove(&self, params: &Value) -> Result<MethodOutcome, CoreError> {
        self.require_running()?;
        let peer_id = required_text(params, "peer_id")?;
        let removed = self.store.unblock_peer(peer_id)?;
        if removed {
            self.refresh_network_blocklist();
            self.try_emit(super::event::PendingEvent::durable(
                "security.peer_unblocked",
                Value::Map(vec![(
                    Value::Text("peer_id".to_owned()),
                    Value::Text(peer_id.to_owned()),
                )]),
            ));
        }
        Ok(MethodOutcome::Result(Value::Map(vec![
            (Value::Text("peer_id".to_owned()), Value::Text(peer_id.to_owned())),
            (Value::Text("removed".to_owned()), Value::Bool(removed)),
        ])))
    }

    pub(crate) fn security_report_add(&self, params: &Value) -> Result<MethodOutcome, CoreError> {
        self.require_running()?;
        let peer_id = required_text(params, "peer_id")?;
        let category = match optional_text(params, "category")? {
            Some(category) => AbuseCategory::parse(&category)
                .ok_or(CoreError::InvalidArgument("category is not an abuse category"))?,
            None => AbuseCategory::Other,
        };
        let note = bounded_note(params)?;
        let report = self.store.record_abuse_report(peer_id, category, note.as_deref())?;
        Ok(MethodOutcome::Result(abuse_report_value(&report)))
    }

    pub(crate) fn security_report_list(&self, params: &Value) -> Result<Value, CoreError> {
        self.require_running()?;
        let limit = match map_get(params, "limit") {
            None | Some(Value::Null) => DEFAULT_REPORT_LIMIT,
            Some(value) => value
                .as_unsigned()
                .ok_or(CoreError::InvalidArgument("limit must be an unsigned integer"))?,
        };
        if !(1..=MAX_REPORT_LIMIT).contains(&limit) {
            return Err(CoreError::InvalidArgument("limit must be between 1 and 200"));
        }
        let reports = self.store.abuse_reports()?;
        let total = reports.len();
        let items: Vec<Value> = reports
            .iter()
            .take(usize::try_from(limit).unwrap_or(usize::MAX))
            .map(abuse_report_value)
            .collect();
        Ok(Value::Map(vec![
            (
                Value::Text("count".to_owned()),
                Value::Unsigned(u64::try_from(items.len()).unwrap_or(u64::MAX)),
            ),
            (
                Value::Text("total".to_owned()),
                Value::Unsigned(u64::try_from(total).unwrap_or(u64::MAX)),
            ),
            (Value::Text("local_only".to_owned()), Value::Bool(true)),
            (Value::Text("reports".to_owned()), Value::Array(items)),
        ]))
    }

    pub(crate) fn security_notices(&self) -> Result<Value, CoreError> {
        self.require_running()?;
        Ok(Value::Map(vec![
            (Value::Text("auto_execute".to_owned()), Value::Bool(false)),
            (Value::Text("previews".to_owned()), Value::Text("delegated".to_owned())),
            (Value::Text("notices".to_owned()), crate::safety::notices()),
        ]))
    }
}

fn bounded_note(params: &Value) -> Result<Option<String>, CoreError> {
    let Some(note) = optional_text(params, "note")? else {
        return Ok(None);
    };
    let note = note.trim();
    if note.is_empty() {
        return Ok(None);
    }
    if note.len() > MAX_SECURITY_NOTE_BYTES {
        return Err(CoreError::InvalidArgument("note must be at most 512 bytes"));
    }
    Ok(Some(note.to_owned()))
}

fn block_cap(store: &nexus_store::Store) -> u64 {
    u64::try_from(store.config().max_blocked_peers).unwrap_or(u64::MAX)
}

fn blocked_peer_value(blocked: &BlockedPeer) -> Value {
    Value::Map(vec![
        (Value::Text("peer_id".to_owned()), Value::Text(blocked.peer_id.clone())),
        (
            Value::Text("reason".to_owned()),
            Value::Text(blocked.reason.as_str().to_owned()),
        ),
        (
            Value::Text("note".to_owned()),
            blocked.note.clone().map_or(Value::Null, Value::Text),
        ),
        (
            Value::Text("blocked_at".to_owned()),
            Value::Unsigned(blocked.created_at),
        ),
        (
            Value::Text("updated_at".to_owned()),
            Value::Unsigned(blocked.updated_at),
        ),
        (Value::Text("local_only".to_owned()), Value::Bool(true)),
    ])
}

fn abuse_report_value(report: &AbuseReport) -> Value {
    Value::Map(vec![
        (Value::Text("report_id".to_owned()), Value::Unsigned(report.report_id)),
        (Value::Text("peer_id".to_owned()), Value::Text(report.peer_id.clone())),
        (
            Value::Text("category".to_owned()),
            Value::Text(report.category.as_str().to_owned()),
        ),
        (
            Value::Text("note".to_owned()),
            report.note.clone().map_or(Value::Null, Value::Text),
        ),
        (Value::Text("created_at".to_owned()), Value::Unsigned(report.created_at)),
        (Value::Text("local_only".to_owned()), Value::Bool(true)),
    ])
}

fn map_get<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    let Value::Map(entries) = value else {
        return None;
    };
    entries
        .iter()
        .find_map(|(entry_key, entry_value)| (entry_key.as_text() == Some(key)).then_some(entry_value))
}

fn optional_text(value: &Value, key: &str) -> Result<Option<String>, CoreError> {
    match map_get(value, key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Text(text)) => Ok(Some(text.clone())),
        Some(_) => Err(CoreError::InvalidArgument("expected a text value")),
    }
}

fn required_text<'a>(value: &'a Value, key: &'static str) -> Result<&'a str, CoreError> {
    match map_get(value, key) {
        Some(Value::Text(text)) => Ok(text.as_str()),
        Some(_) => Err(CoreError::InvalidArgument("peer_id must be text")),
        None => Err(CoreError::InvalidArgument("peer_id is required")),
    }
}

#[cfg(test)]
mod tests {
    use super::{abuse_report_value, blocked_peer_value};
    use nexus_crypto::Value;
    use nexus_store::{AbuseCategory, AbuseReport, BlockReason, BlockedPeer};

    fn text(value: &Value, key: &str) -> Option<String> {
        let Value::Map(entries) = value else {
            return None;
        };
        entries.iter().find_map(|(entry_key, entry_value)| {
            if entry_key.as_text() == Some(key) {
                entry_value.as_text().map(str::to_owned)
            } else {
                None
            }
        })
    }

    #[test]
    fn blocked_values_mark_records_local_only() {
        let value = blocked_peer_value(&BlockedPeer {
            peer_id: "12D3KooWpeer".to_owned(),
            reason: BlockReason::Spam,
            note: Some("repeat messages".to_owned()),
            created_at: 1,
            updated_at: 2,
        });
        assert_eq!(text(&value, "reason").as_deref(), Some("spam"));
        assert_eq!(text(&value, "local_only"), None);
        let Value::Map(entries) = &value else {
            panic!("expected a map");
        };
        assert!(
            entries
                .iter()
                .any(|(key, value)| key.as_text() == Some("local_only") && matches!(value, Value::Bool(true)))
        );
    }

    #[test]
    fn abuse_values_have_a_report_identifier() {
        let value = abuse_report_value(&AbuseReport {
            report_id: 7,
            peer_id: "12D3KooWpeer".to_owned(),
            category: AbuseCategory::Harassment,
            note: None,
            created_at: 3,
        });
        assert_eq!(text(&value, "category").as_deref(), Some("harassment"));
        assert!(text(&value, "note").is_none());
        assert!(matches!(
            value,
            Value::Map(entries) if entries.iter().any(|(key, _)| key.as_text() == Some("report_id"))
        ));
    }
}
