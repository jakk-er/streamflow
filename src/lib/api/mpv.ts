import { invoke } from '@tauri-apps/api/core';
import type { EmbeddedMpvSession } from '$lib/types/player';

export type MpvCapability = { status: 'available' } | { status: 'unavailable'; reason: string };

/**
 * Windows-only embedded playback for MKV/HEVC VOD, which the native
 * `<video>` element can't handle (Chromium doesn't support Matroska).
 * Result is cached Rust-side, so safe to call before every attempt.
 */
export async function mpvCheckAvailable(): Promise<MpvCapability> {
  return await invoke<MpvCapability>('mpv_check_available');
}

/**
 * Starts an embedded mpv session. `bounds` (logical/CSS px, relative to the
 * webview viewport) should match the placeholder `<div>`'s
 * `getBoundingClientRect()` (see `EmbeddedMpvPlayer.svelte`).
 */
export async function mpvStartSession(
  url: string,
  title: string,
  bounds: { x: number; y: number; width: number; height: number },
  startPositionSeconds?: number
): Promise<string> {
  return await invoke<string>('mpv_start_session', {
    url,
    title,
    x: bounds.x,
    y: bounds.y,
    width: bounds.width,
    height: bounds.height,
    startPositionSeconds,
  });
}

/**
 * Repositions/resizes the native window to the full video area (never
 * shrunk for overlay UI - that would force mpv to rescale the picture), and
 * clips out `excludeRects` (control bar, close button) so the HTML
 * underneath still receives clicks - see `window::set_region` (Rust). Call
 * on every bounds or exclusion-rect change.
 */
export async function mpvSetBounds(
  sessionId: string,
  bounds: { x: number; y: number; width: number; height: number },
  excludeRects: { x: number; y: number; width: number; height: number }[] = []
): Promise<void> {
  await invoke<void>('mpv_set_bounds', { sessionId, x: bounds.x, y: bounds.y, width: bounds.width, height: bounds.height, excludeRects });
}

export async function mpvStopSession(sessionId: string): Promise<void> {
  await invoke<void>('mpv_stop_session', { sessionId });
}

export async function mpvGetSessionState(sessionId: string): Promise<EmbeddedMpvSession> {
  return await invoke<EmbeddedMpvSession>('mpv_get_session_state', { sessionId });
}

export async function mpvPlayPause(sessionId: string, paused: boolean): Promise<void> {
  await invoke<void>('mpv_play_pause', { sessionId, paused });
}

export async function mpvSeek(sessionId: string, positionSeconds: number): Promise<void> {
  await invoke<void>('mpv_seek', { sessionId, positionSeconds });
}

export async function mpvSetVolume(sessionId: string, volume: number): Promise<void> {
  await invoke<void>('mpv_set_volume', { sessionId, volume });
}

/** `brightness` is mpv's native -100..100 scale, not the UI's 50..150% slider. */
export async function mpvSetBrightness(sessionId: string, brightness: number): Promise<void> {
  await invoke<void>('mpv_set_brightness', { sessionId, brightness });
}
