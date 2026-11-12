import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { relaunch } from '@tauri-apps/plugin-process';
import { check, type Update } from '@tauri-apps/plugin-updater';
import type { Api } from './index';
import type { CoreEvent } from './types';

/** Dernière mise à jour trouvée (l'objet du module sert ensuite à l'installer). */
let pendingUpdate: Update | null = null;

export const tauriApi: Api = {
  detectProvider: (email) => invoke('detect_provider', { email }),
  addAccount: (input, password) => invoke('add_account', { input, password }),
  updatePassword: (accountId, password) => invoke('update_password', { accountId, password }),
  listAccounts: () => invoke('list_accounts'),
  listFolders: (accountId) => invoke('list_folders', { accountId }),
  listMessages: (scope, offset, limit) => invoke('list_messages', { scope, offset, limit }),
  countMessages: (scope) => invoke('count_messages', { scope }),
  selectFolder: (folderId) => invoke('select_folder', { folderId }),
  openMessage: (id, showImages) => invoke('open_message', { id, showImages }),
  allowImagesFrom: (sender) => invoke('allow_images_from', { sender }),
  openAttachment: (id, partPath) => invoke('open_attachment', { id, partPath }),
  saveAttachment: (id, partPath) => invoke('save_attachment', { id, partPath }),
  openUrl: (url) => invoke('open_url', { url }),
  windowAction: async (action) => {
    const w = getCurrentWindow();
    if (action === 'minimize') await w.minimize();
    else if (action === 'toggleMaximize') await w.toggleMaximize();
    else await w.close();
  },
  onEvent: (cb) => subscription(listen<CoreEvent>('core-event', (e) => cb(e.payload))),
  searchContacts: (query) => invoke('search_contacts', { query }),
  createDraft: (kind, accountId, sourceMessageId) =>
    invoke('create_draft', { kind, accountId: accountId ?? null, sourceMessageId: sourceMessageId ?? null }),
  draftForMessage: (messageId) => invoke('draft_for_message', { messageId }),
  draftFromMessage: (messageId) => invoke('draft_from_message', { messageId }),
  getDraft: (draftId) => invoke('get_draft', { draftId }),
  updateDraft: (input) => invoke('update_draft', { input }),
  pickAttachments: (draftId) => invoke('pick_attachments', { draftId }),
  addAttachments: (draftId, paths) => invoke('add_attachments', { draftId, paths }),
  removeAttachment: (draftId, index) => invoke('remove_attachment', { draftId, index }),
  closeDraft: (draftId) => invoke('close_draft', { draftId }),
  discardDraft: (draftId) => invoke('discard_draft', { draftId }),
  sendDraft: (draftId) => invoke('send_draft', { draftId }),
  cancelSend: (draftId) => invoke('cancel_send', { draftId }),
  listOutbox: () => invoke('list_outbox'),
  getSettings: () => invoke('get_settings'),
  setSetting: (key, value) => invoke('set_setting', { key, value }),
  openComposeWindow: (draftId) => invoke('open_compose_window', { draftId }),
  reorderAccounts: (ids) => invoke('reorder_accounts', { ids }),
  updateAccount: (accountId, patch) => invoke('update_account', { accountId, patch }),
  updateServers: (accountId, imap, smtp) => invoke('update_servers', { accountId, imap, smtp }),
  removeAccount: (accountId) => invoke('remove_account', { accountId }),
  listImageAllowlist: () => invoke('list_image_allowlist'),
  removeFromImageAllowlist: (sender) => invoke('remove_from_image_allowlist', { sender }),
  storageUsage: () => invoke('storage_usage'),
  clearCache: () => invoke('clear_cache'),
  openLogsFolder: () => invoke('open_logs_folder'),
  appInfo: () => invoke('app_info'),
  checkUpdate: async () => {
    pendingUpdate = await check();
    return pendingUpdate ? { version: pendingUpdate.version, notes: pendingUpdate.body ?? null } : null;
  },
  downloadUpdate: async (onProgress) => {
    const update = pendingUpdate ?? (pendingUpdate = await check());
    if (!update) throw new Error('aucune mise à jour disponible');
    let total = 0;
    let done = 0;
    await update.download((e) => {
      if (e.event === 'Started') {
        total = e.data.contentLength ?? 0;
        onProgress(total ? 0 : null);
      } else if (e.event === 'Progress') {
        done += e.data.chunkLength;
        onProgress(total ? Math.min(100, Math.round((done / total) * 100)) : null);
      } else {
        onProgress(100);
      }
    });
  },
  applyUpdate: async () => {
    if (!pendingUpdate) throw new Error('aucune mise à jour téléchargée');
    // Sous Windows, l'installateur ferme Illy et le relance lui-même ; ailleurs, on relance.
    await pendingUpdate.install();
    await relaunch();
  },
  closeComposeWindows: () => invoke('close_compose_windows'),
  onWindowCloseRequested: (cb) =>
    subscription(
      getCurrentWindow().onCloseRequested(async (event) => {
        event.preventDefault();
        try {
          await cb();
        } finally {
          await getCurrentWindow().destroy();
        }
      }),
    ),
  onFileDrop: (cb) =>
    subscription(
      getCurrentWebview().onDragDropEvent((e) => {
        if (e.payload.type !== 'drop' || e.payload.paths.length === 0) return;
        const ratio = window.devicePixelRatio || 1;
        cb(e.payload.paths, { x: e.payload.position.x / ratio, y: e.payload.position.y / ratio });
      }),
    ),
};

/** Abonnement Tauri (promesse d'un désabonnement) vu comme une fonction de désabonnement immédiate. */
function subscription(p: Promise<() => void>): () => void {
  let off: (() => void) | null = null;
  let cancelled = false;
  void p.then((unlisten) => {
    if (cancelled) unlisten();
    else off = unlisten;
  });
  return () => {
    cancelled = true;
    off?.();
  };
}
