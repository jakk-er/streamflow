import { stalkerAuth, stalkerDoAuth, stalkerWatchdogPing, stalkerGetChannels, stalkerSyncEpg } from '$lib/api';
import type { StalkerAuthOutcome } from '$lib/types';
import { formatError } from '$lib/utils/errors';

function describeOutcome(outcome: StalkerAuthOutcome): string {
  switch (outcome.kind) {
    case 'loginRejected':
    case 'deviceConflict':
    case 'blocked':
      return outcome.message;
    case 'loginRequired':
      return 'This portal requires a username and password.';
    default:
      return '';
  }
}

/**
 * Owns the Stalker auth lifecycle: authenticate, the `do_auth` login-required
 * follow-up, and the watchdog keep-alive. Once a session exists it kicks off
 * a background channel + EPG sync so live TV is populated automatically;
 * category/content/EPG browsing itself lives in `vodStore`/`channelStore`/`epgStore`.
 */
function createStalkerSessionStore() {
  let outcome = $state<StalkerAuthOutcome | null>(null);
  let loading = $state(false);
  let error = $state<string | null>(null);

  let watchdogInterval: number | null = null;
  let watchdogTimeslotTimeout: number | null = null;
  let watchdogPlaylistId: string | null = null;

  async function afterSuccess(playlistId: string, watchdogTimeout: number, timeslot: number) {
    setupWatchdog(playlistId, watchdogTimeout, timeslot);
    // Fire-and-forget from the caller's perspective, but sequenced not
    // concurrent: some portals tolerate only ~1 connection at a time (4
    // concurrent requests dropped ~1/3 of the time on a real portal;
    // sequential never failed). Channel sync can itself fall back to a
    // paginated crawl on failure, so racing EPG against it compounds the
    // portal's load. Neither failure is fatal - the user can still browse,
    // and a manual refresh can retry.
    try {
      await stalkerGetChannels(playlistId);
    } catch (err) {
      console.error('[stalkerSessionStore] Channel sync failed:', err);
    }
    stalkerSyncEpg(playlistId).catch((err) => {
      console.error('[stalkerSessionStore] EPG sync failed:', err);
    });
  }

  async function authenticate(playlistId: string, username?: string, password?: string) {
    error = null;
    loading = true;
    try {
      const result = await stalkerAuth(playlistId, username, password);
      outcome = result;
      if (result.kind === 'success') {
        afterSuccess(playlistId, result.session.watchdogTimeout, result.session.timeslot);
      } else if (result.kind !== 'loginRequired') {
        error = describeOutcome(result);
      }
      return result;
    } catch (err) {
      error = formatError(err);
      console.error('[stalkerSessionStore] Authenticate failed:', error);
      throw err;
    } finally {
      loading = false;
    }
  }

  async function completeLogin(playlistId: string, username: string, password: string) {
    error = null;
    loading = true;
    try {
      const result = await stalkerDoAuth(playlistId, username, password);
      outcome = result;
      if (result.kind === 'success') {
        afterSuccess(playlistId, result.session.watchdogTimeout, result.session.timeslot);
      } else {
        error = describeOutcome(result);
      }
      return result;
    } catch (err) {
      error = formatError(err);
      console.error('[stalkerSessionStore] Login failed:', error);
      throw err;
    } finally {
      loading = false;
    }
  }

  /**
   * Matches the reference client's watchdog sequencing: an immediate,
   * unawaited `init=1` ping fires the moment the watchdog activates, then
   * periodic `init=0` pings start after a `timeslot`-second jitter delay
   * (or immediately if the portal didn't advertise one). Without the
   * immediate ping the session wouldn't show "online" in the portal's admin
   * panel until the first full period elapsed.
   */
  function setupWatchdog(playlistId: string, watchdogTimeoutSeconds: number, timeslotSeconds: number) {
    stopWatchdog();
    watchdogPlaylistId = playlistId;
    const periodMs = Math.max(30, watchdogTimeoutSeconds || 120) * 1000;
    const timeslotMs = Math.max(0, Math.min(timeslotSeconds || 0, periodMs / 1000 - 1)) * 1000;

    stalkerWatchdogPing(playlistId, true).catch(() => {
      // Non-fatal by design: a missed ping only affects the portal's
      // admin-panel "online" reporting, never the session itself.
    });

    const startInterval = () => {
      watchdogTimeslotTimeout = null;
      watchdogInterval = window.setInterval(() => {
        if (watchdogPlaylistId) {
          stalkerWatchdogPing(watchdogPlaylistId, false).catch(() => {});
        }
      }, periodMs);
    };

    if (timeslotMs > 0) {
      watchdogTimeslotTimeout = window.setTimeout(startInterval, timeslotMs);
    } else {
      startInterval();
    }
  }

  function stopWatchdog() {
    if (watchdogInterval !== null) {
      clearInterval(watchdogInterval);
      watchdogInterval = null;
    }
    if (watchdogTimeslotTimeout !== null) {
      clearTimeout(watchdogTimeslotTimeout);
      watchdogTimeslotTimeout = null;
    }
    watchdogPlaylistId = null;
  }

  return {
    get outcome() { return outcome; },
    get loading() { return loading; },
    get error() { return error; },
    get loginRequired() { return outcome?.kind === 'loginRequired'; },
    authenticate,
    completeLogin,
    stopWatchdog,
  };
}

export const stalkerSessionStore = createStalkerSessionStore();
