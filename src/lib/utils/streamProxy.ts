import { getStreamProxyPort } from '$lib/api';
import { getPlaybackMediaExtensionFromUrl } from './playbackMediaExtension';

// The port is fixed for the app's lifetime (assigned once when the proxy
// server starts) - caching it avoids an IPC round trip on every single
// channel/movie/episode switch.
let cachedStreamProxyPort: number | null = null;
async function resolveStreamProxyPort(): Promise<number> {
  if (cachedStreamProxyPort === null) {
    cachedStreamProxyPort = await getStreamProxyPort();
  }
  return cachedStreamProxyPort;
}

/**
 * Routes a resolved playback URL through the local same-origin proxy
 * (`crate::stream_proxy`) instead of handing it to the player directly.
 *
 * hls.js/mpegts.js fetch segments via `fetch`/XHR, subject to normal browser
 * CORS - a desktop app isn't exempt. Stalker/Ministra CDNs and most Xtream
 * reseller panels don't send permissive CORS headers, so both need this
 * (mpeg-ts happens to work unproxied because mpegts.js's loader is less
 * CORS-strict, not because panels allow it). Every playback path (live,
 * VOD, episodes, any provider) routes through here now.
 *
 * `isVod` marks a bounded on-demand file (vs. an open-ended live feed) -
 * defaults to `false` so existing live call sites are unaffected.
 */
export async function wrapUrlThroughStreamProxy(
  playlistId: string,
  resolvedUrl: string,
  isVod = false
): Promise<{ url: string; extension?: string }> {
  const port = await resolveStreamProxyPort();
  // Read from the real (pre-proxy) URL - the proxied URL carries it nested
  // inside its `url` query value, so post-proxy detection would miss it.
  const extension = getPlaybackMediaExtensionFromUrl(resolvedUrl);
  const params = new URLSearchParams({ playlist_id: playlistId, url: resolvedUrl });
  if (isVod) params.set('vod', 'true');
  return { url: `http://127.0.0.1:${port}/stream?${params.toString()}`, extension };
}
