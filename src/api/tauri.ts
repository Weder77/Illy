import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
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
  onEvent: (cb) => {
    let off: (() => void) | null = null;
    let cancelled = false;
    listen<CoreEvent>('core-event', (e) => cb(e.payload)).then((unlisten) => {
      if (cancelled) unlisten();
      else off = unlisten;
    });
    return () => {
      cancelled = true;
      off?.();
    };
  },
};
