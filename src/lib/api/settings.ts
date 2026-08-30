import { invoke } from '@tauri-apps/api/core';
import type { AppSettings } from '$lib/types';

/**
 * Get the current application settings from the database.
 * @returns The AppSettings object
 */
export async function getSettings(): Promise<AppSettings> {
  return await invoke<AppSettings>('get_settings');
}

/**
 * Update the application settings in the database.
 * @param settings - The AppSettings object to save
 */
export async function updateSettings(settings: AppSettings): Promise<void> {
  await invoke<void>('update_settings', { settings });
}
