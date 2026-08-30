import { getSettings, updateSettings } from '$lib/api';
import type { AppSettings } from '$lib/types';
import { formatError } from '$lib/utils/errors';

function createSettingsStore() {
  let settings = $state<AppSettings | null>(null);
  let loading = $state(false);
  let error = $state<string | null>(null);

  async function load() {
    error = null;
    loading = true;
    try {
      settings = await getSettings();
    } catch (err) {
      error = formatError(err);
    } finally {
      loading = false;
    }
  }

  async function update(partial: Partial<AppSettings>) {
    error = null;
    loading = true;
    try {
      const merged = { ...settings, ...partial } as AppSettings;
      await updateSettings(merged);
      settings = merged;
    } catch (err) {
      error = formatError(err);
      throw err;
    } finally {
      loading = false;
    }
  }

  const theme = $derived(() => settings?.theme ?? 'DARK_THEME');
  const language = $derived(() => settings?.language ?? 'en');

  load();

  return {
    get settings() { return settings; },
    get loading() { return loading; },
    get error() { return error; },
    get theme() { return theme(); },
    get language() { return language(); },
    load,
    update,
  };
}

export const settingsStore = createSettingsStore();
