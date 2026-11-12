import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import type { Api } from './index';
import type { CoreEvent } from './types';

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
