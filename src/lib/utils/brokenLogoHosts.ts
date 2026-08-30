/**
 * Hosts hardcoded by specific Stalker portals into logo URLs that are dead
 * at the DNS level (e.g. `billing.tv4k.me`). `onerror` on `<img>` already
 * falls back gracefully, but the browser still logs the failed request
 * before `onerror` runs - skipping `src` entirely avoids that. Hand-confirmed
 * list only; add a host once it's actually been seen failing, not preemptively.
 */
const KNOWN_BAD_LOGO_HOSTS = new Set(['billing.tv4k.me']);

export function isKnownBadLogoUrl(url: string | undefined | null): boolean {
  if (!url) return false;
  try {
    return KNOWN_BAD_LOGO_HOSTS.has(new URL(url).hostname);
  } catch {
    return false;
  }
}
