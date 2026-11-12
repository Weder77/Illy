//! Types partagés entre le cœur et l'interface (sérialisés en camelCase).

use serde::{Deserialize, Serialize};

pub type AccountId = i64;
pub type FolderId = i64;
pub type MessageId = i64;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    Gmail,
    Yahoo,
    Ovh,
    Custom,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TlsMode {
    Implicit,
    StartTls,
    Plain,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub tls: TlsMode,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NewAccount {
    pub email: String,
    pub display_name: String,
    pub provider: Provider,
    pub imap: ServerConfig,
    pub smtp: ServerConfig,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AccountStatus {
    Ok,
    Syncing,
    AuthError,
    Offline,
    Error,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: AccountId,
    pub email: String,
    pub display_name: String,
    pub provider: Provider,
    pub imap: ServerConfig,
    pub smtp: ServerConfig,
    pub color: String,
    pub sort_order: i64,
    pub status: AccountStatus,
    pub last_error: Option<String>,
    /// Signature HTML (nettoyée), ajoutée aux nouveaux mails et réponses.
    pub signature_html: String,
}

/// Espace disque utilisé par Illy, en octets.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct StorageUsage {
    pub database: u64,
    pub drafts: u64,
    pub temporary: u64,
}

/// Modification partielle d'un compte (Paramètres → Comptes).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct AccountPatch {
    pub display_name: Option<String>,
    pub color: Option<String>,
    pub signature_html: Option<String>,
}

/// L'ordre de déclaration est l'ordre d'affichage.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum FolderRole {
    Inbox,
    Drafts,
    Sent,
    Spam,
    Trash,
    Custom,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Folder {
    pub id: FolderId,
    pub account_id: AccountId,
    pub imap_path: String,
    pub display_name: String,
    pub role: FolderRole,
    pub unread_count: u32,
    pub total_count: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Address {
    pub name: Option<String>,
    pub email: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MessageSummary {
    pub id: MessageId,
    pub account_id: AccountId,
    pub folder_id: FolderId,
    pub from: Address,
    pub subject: String,
    pub snippet: String,
    pub date: i64,
    pub seen: bool,
    pub has_attachments: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    pub part_path: String,
    pub filename: String,
    pub mime_type: String,
    pub size: u64,
    pub can_open: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(tag = "kind", content = "content", rename_all = "snake_case")]
pub enum MessageBody {
    Html(String),
    Text(String),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MessageDetail {
    pub summary: MessageSummary,
    pub to: Vec<Address>,
    pub cc: Vec<Address>,
    pub body: MessageBody,
    pub attachments: Vec<Attachment>,
    pub images_blocked: bool,
    /// Images distantes autorisées explicitement (ce mail ou cet expéditeur) : seul critère d'ouverture de la CSP.
    pub remote_images_allowed: bool,
    pub degraded: bool,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(tag = "kind", content = "folderId", rename_all = "snake_case")]
pub enum ListScope {
    Folder(FolderId),
    UnifiedInbox,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderGuess {
    pub provider: Option<Provider>,
    pub imap: Option<ServerConfig>,
    pub smtp: Option<ServerConfig>,
}

pub type DraftId = i64;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DraftKind {
    New,
    Reply,
    ReplyAll,
    Forward,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DraftState {
    Editing,
    Queued,
    Sending,
    Sent,
    Failed,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", rename_all_fields = "camelCase")]
pub enum AttachmentSource {
    File { path: String },
    Part { message_id: MessageId, part_path: String },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DraftAttachment {
    pub filename: String,
    pub mime_type: String,
    pub size: u64,
    pub source: AttachmentSource,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Draft {
    pub id: DraftId,
    pub account_id: AccountId,
    pub kind: DraftKind,
    pub source_message_id: Option<MessageId>,
    pub to: Vec<Address>,
    pub cc: Vec<Address>,
    pub bcc: Vec<Address>,
    pub subject: String,
    pub body_html: String,
    pub attachments: Vec<DraftAttachment>,
    pub state: DraftState,
    pub last_error: Option<String>,
    pub send_at: Option<i64>,
    pub remote_uid: Option<u32>,
    pub updated_at: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DraftInput {
    pub id: DraftId,
    pub account_id: AccountId,
    pub to: Vec<Address>,
    pub cc: Vec<Address>,
    pub bcc: Vec<Address>,
    pub subject: String,
    pub body_html: String,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SendTicket {
    pub send_at: i64,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum RemoteImages {
    #[default]
    Block,
    Always,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    #[default]
    Dark,
    Light,
    System,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Accent {
    #[default]
    Blue,
    Violet,
    Green,
    Orange,
    Pink,
    Teal,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TextSize {
    Small,
    #[default]
    Normal,
    Large,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Density {
    #[default]
    Comfortable,
    Compact,
}

/// Réglages de l'application (table `settings` ; une clé absente vaut sa valeur par défaut).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub undo_send_seconds: u32,
    pub remote_images: RemoteImages,
    pub theme: Theme,
    pub accent: Accent,
    pub text_size: TextSize,
    pub density: Density,
    pub show_snippet: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            undo_send_seconds: 10,
            remote_images: RemoteImages::Block,
            theme: Theme::Dark,
            accent: Accent::Blue,
            text_size: TextSize::Normal,
            density: Density::Comfortable,
            show_snippet: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_scope_serialization() {
        assert_eq!(
            serde_json::to_value(ListScope::Folder(3)).unwrap(),
            serde_json::json!({"kind": "folder", "folderId": 3})
        );
        assert_eq!(
            serde_json::to_value(ListScope::UnifiedInbox).unwrap(),
            serde_json::json!({"kind": "unified_inbox"})
        );
        let back: ListScope = serde_json::from_value(serde_json::json!({"kind": "folder", "folderId": 3})).unwrap();
        assert_eq!(back, ListScope::Folder(3));
    }

    #[test]
    fn body_serialization() {
        assert_eq!(
            serde_json::to_value(MessageBody::Html("<p>x</p>".into())).unwrap(),
            serde_json::json!({"kind": "html", "content": "<p>x</p>"})
        );
    }
}
