//! Synchro d'un dossier : nouveaux mails (récents d'abord), suppressions, drapeaux.

use super::{diff_uids, uidvalidity_changed, WorkerConfig};
use crate::error::Result;
use crate::events::{CoreEvent, EventSink};
use crate::imap::{fetch_flags, fetch_headers, search_all_uids, select, uid_set, Caps, ImapSession};
use crate::model::{AccountId, FolderId};
use crate::store::{now_ts, FolderSyncState, Store};

/// UID restant à récupérer pour un dossier (du plus récent au plus ancien).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FolderBacklog {
    pub folder_id: FolderId,
    pub pending_desc: Vec<u32>,
}

/// Récupération d'un lot d'UID (abstraite pour pouvoir tester le découpage).
pub trait ChunkFetcher<T> {
    fn fetch(&mut self, chunk: &[u32]) -> impl std::future::Future<Output = Result<Vec<T>>> + Send;
}

struct HeaderFetcher<'a> {
    s: &'a mut ImapSession,
    with_structure: bool,
}

impl ChunkFetcher<crate::store::NewHeader> for HeaderFetcher<'_> {
    fn fetch(&mut self, chunk: &[u32]) -> impl std::future::Future<Output = Result<Vec<crate::store::NewHeader>>> + Send {
        fetch_headers(self.s, chunk, self.with_structure)
    }
}

/// Exécute `fetcher` sur un lot d'UID ; si le serveur refuse (NO), coupe le lot en deux
/// pour isoler les mails fautifs, qui sont ignorés. Les erreurs fatales remontent.
pub async fn bisect_fetch<T: Send>(uids: &[u32], fetcher: &mut (impl ChunkFetcher<T> + Send)) -> Result<Vec<T>> {
    let mut out = Vec::new();
    let mut stack: Vec<Vec<u32>> = vec![uids.to_vec()];
    while let Some(chunk) = stack.pop() {
        if chunk.is_empty() {
            continue;
        }
        match fetcher.fetch(&chunk).await {
            Ok(v) => out.extend(v),
            Err(crate::CoreError::Imap(m)) if chunk.len() > 1 => {
                tracing::debug!("lot refusé ({m}), découpage");
                let (a, b) = chunk.split_at(chunk.len() / 2);
                stack.push(b.to_vec());
                stack.push(a.to_vec());
            }
            Err(crate::CoreError::Imap(m)) => tracing::warn!(uid = chunk[0], "mail ignoré, refusé par le serveur : {m}"),
            Err(e) => return Err(e),
        }
    }
    Ok(out)
}

/// Intervalle entre deux relectures complètes des drapeaux sans CONDSTORE.
const FULL_FLAGS_EVERY_SECS: i64 = 3600;

#[allow(clippy::too_many_arguments)]
pub async fn sync_folder(
    s: &mut ImapSession,
    caps: &Caps,
    store: &Store,
    events: &dyn EventSink,
    account: AccountId,
    folder: FolderId,
    path: &str,
    cfg: &WorkerConfig,
    with_structure: bool,
) -> Result<FolderBacklog> {
    let sel = select(s, path, caps.condstore).await?;
    let mut st = store.folder_state(folder)?;
    let mut changed = false;
    if uidvalidity_changed(st.uidvalidity, sel.uidvalidity) {
        tracing::info!(folder, "UIDVALIDITY changé : resynchro complète du dossier");
        store.clear_folder(folder)?;
        st = FolderSyncState::default();
        changed = true;
    }

    let server = search_all_uids(s).await?;
    let known = store.known_uids(folder)?;
    let diff = diff_uids(&server, &known);
    if !diff.vanished.is_empty() {
        changed |= store.delete_uids(folder, &diff.vanished)? > 0;
    }

    // Drapeaux des mails déjà connus (les nouveaux arrivent avec leurs drapeaux).
    let vanished: std::collections::HashSet<u32> = diff.vanished.iter().copied().collect();
    let still_known: Vec<u32> = known.into_iter().filter(|u| !vanished.contains(u)).collect();
    let now = now_ts();
    let mut full_flags_done = false;
    let flags = match (caps.condstore, st.highest_modseq, sel.highest_modseq) {
        (true, Some(stored), Some(_)) if !still_known.is_empty() => fetch_flags(s, "1:*", Some(stored)).await?,
        _ if still_known.is_empty() => vec![],
        _ => {
            let stale = st.last_sync_at.map(|t| now - t >= FULL_FLAGS_EVERY_SECS).unwrap_or(true);
            let window: &[u32] = if stale {
                full_flags_done = true;
                &still_known
            } else {
                &still_known[still_known.len().saturating_sub(cfg.flags_recent_window)..]
            };
            fetch_flags(s, &uid_set(window), None).await?
        }
    };
    if !flags.is_empty() {
        changed |= !store.apply_flags(folder, &flags)?.is_empty();
    }

    let batch = (cfg.header_batch as usize).min(diff.new_desc.len());
    let (first, rest) = diff.new_desc.split_at(batch);
    if !first.is_empty() {
        let headers = bisect_fetch(first, &mut HeaderFetcher { s, with_structure }).await?;
        changed |= store.upsert_headers(account, folder, &headers)? > 0;
        events.emit(CoreEvent::SyncProgress {
            account_id: account,
            folder_id: folder,
            fetched: first.len() as u32,
            total: diff.new_desc.len() as u32,
        });
    }

    // `last_sync_at` = dernière relecture complète des drapeaux.
    let full = full_flags_done || st.last_sync_at.is_none() || caps.condstore;
    store.set_folder_state(
        folder,
        &FolderSyncState {
            uidvalidity: Some(sel.uidvalidity),
            uidnext: sel.uidnext,
            highest_modseq: sel.highest_modseq,
            last_sync_at: if full { Some(now) } else { st.last_sync_at },
        },
    )?;
    if changed {
        events.emit(CoreEvent::FolderChanged { account_id: account, folder_id: folder });
    }
    Ok(FolderBacklog { folder_id: folder, pending_desc: rest.to_vec() })
}

/// Récupère le lot suivant d'en-têtes anciens d'un dossier.
#[allow(clippy::too_many_arguments)]
pub async fn backfill_step(
    s: &mut ImapSession,
    store: &Store,
    events: &dyn EventSink,
    account: AccountId,
    path: &str,
    backlog: &mut FolderBacklog,
    cfg: &WorkerConfig,
    with_structure: bool,
) -> Result<()> {
    if backlog.pending_desc.is_empty() {
        return Ok(());
    }
    let n = (cfg.header_batch as usize).min(backlog.pending_desc.len());
    select(s, path, false).await?;
    // Le lot n'est retiré du backlog qu'en cas de succès (sinon il est réessayé).
    let batch = backlog.pending_desc[..n].to_vec();
    let headers = bisect_fetch(&batch, &mut HeaderFetcher { s, with_structure }).await?;
    backlog.pending_desc.drain(..n);
    if store.upsert_headers(account, backlog.folder_id, &headers)? > 0 {
        events.emit(CoreEvent::FolderChanged { account_id: account, folder_id: backlog.folder_id });
    }
    events.emit(CoreEvent::SyncProgress {
        account_id: account,
        folder_id: backlog.folder_id,
        fetched: n as u32,
        total: (n + backlog.pending_desc.len()) as u32,
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::CoreError;

    struct Fake {
        calls: usize,
        fatal: bool,
    }

    impl ChunkFetcher<u32> for Fake {
        fn fetch(&mut self, chunk: &[u32]) -> impl std::future::Future<Output = Result<Vec<u32>>> + Send {
            self.calls += 1;
            let r = if self.fatal {
                Err(CoreError::Network("down".into()))
            } else if chunk.contains(&7) || chunk.contains(&13) {
                Err(CoreError::Imap("Some messages could not be FETCHed".into()))
            } else {
                Ok(chunk.to_vec())
            };
            async move { r }
        }
    }

    #[tokio::test]
    async fn bisect_isolates_refused_uids() {
        let uids: Vec<u32> = (1..=20).collect();
        let mut fake = Fake { calls: 0, fatal: false };
        let got = bisect_fetch(&uids, &mut fake).await.unwrap();
        let calls = fake.calls;
        let mut got = got;
        got.sort();
        let expected: Vec<u32> = (1..=20).filter(|u| *u != 7 && *u != 13).collect();
        assert_eq!(got, expected);
        assert!(calls < 20, "bisection, not one by one: {calls} calls");
    }

    #[tokio::test]
    async fn bisect_propagates_fatal_errors() {
        let r = bisect_fetch(&[1, 2, 3], &mut Fake { calls: 0, fatal: true }).await;
        assert!(matches!(r, Err(CoreError::Network(_))));
    }
}
