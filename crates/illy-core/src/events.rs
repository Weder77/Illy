//! Événements émis par le cœur vers l'interface.

use crate::model::{AccountId, AccountStatus, DraftId, DraftState, FolderId, MessageId};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case", rename_all_fields = "camelCase")]
pub enum CoreEvent {
    AccountStatus { account_id: AccountId, status: AccountStatus, error: Option<String> },
    FoldersChanged { account_id: AccountId },
    FolderChanged { account_id: AccountId, folder_id: FolderId },
    MessageUpdated { message_id: MessageId },
    SyncProgress { account_id: AccountId, folder_id: FolderId, fetched: u32, total: u32 },
    OutboxChanged { draft_id: DraftId, state: DraftState, send_at: Option<i64>, error: Option<String> },
    SettingsChanged { key: String },
    /// Liste, ordre, nom, couleur, signature ou serveurs des comptes modifiés.
    AccountsChanged,
}

pub trait EventSink: Send + Sync + 'static {
    fn emit(&self, event: CoreEvent);
}

/// Récepteur qui garde les événements en mémoire (tests).
#[derive(Default)]
pub struct RecordingSink(pub Mutex<Vec<CoreEvent>>);

impl RecordingSink {
    pub fn events(&self) -> Vec<CoreEvent> {
        self.0.lock().unwrap().clone()
    }
}

impl EventSink for RecordingSink {
    fn emit(&self, event: CoreEvent) {
        self.0.lock().unwrap().push(event);
    }
}

/// Récepteur qui ignore tout.
pub struct NullSink;

impl EventSink for NullSink {
    fn emit(&self, _event: CoreEvent) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_serialization() {
        let v = serde_json::to_value(CoreEvent::FolderChanged { account_id: 1, folder_id: 2 }).unwrap();
        assert_eq!(v, serde_json::json!({"type": "folder_changed", "accountId": 1, "folderId": 2}));
        assert_eq!(serde_json::to_value(CoreEvent::AccountsChanged).unwrap(), serde_json::json!({"type": "accounts_changed"}));
    }

    #[test]
    fn recording_sink_records() {
        let s = RecordingSink::default();
        s.emit(CoreEvent::FoldersChanged { account_id: 3 });
        assert_eq!(s.events(), vec![CoreEvent::FoldersChanged { account_id: 3 }]);
    }
}
