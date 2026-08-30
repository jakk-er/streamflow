/**
 * Media-extension detection for playback URLs (ported from iptvnator). The
 * extension picks the player engine (mpegts.js for `.ts`, hls.js for `.m3u8`,
 * else native `<video>`); Stalker portals often encode it in a query param
 * (`extension=ts`) rather than the path, so a plain suffix check misses it.
 */

const EXPLICIT_MEDIA_EXTENSION_QUERY_KEYS = ['extension', 'ext'];
const DECLARED_MEDIA_FORMAT_QUERY_KEYS = ['format', 'container'];

const DECLARED_MEDIA_EXTENSION_ALIASES = new Map([
  ['hls', 'm3u8'],
  ['mpegts', 'ts'],
  ['mpeg-ts', 'ts'],
]);

const DECLARED_MEDIA_EXTENSIONS = new Set([
  'avi', 'asf', 'divx', 'flv', 'm2ts', 'm4v', 'mkv', 'mov', 'mpeg', 'mpg', 'rm', 'rmvb', 'ts', 'vob', 'wmv',
  'aac', 'flac', 'm3u', 'm3u8', 'm4s', 'mp3', 'mp4', 'mpd', 'oga', 'ogg', 'ogv', 'webm',
]);

const NON_MEDIA_URL_EXTENSIONS = new Set(['asp', 'aspx', 'cgi', 'jsp', 'mpv', 'php', 'pl']);

function normalizeExtensionToken(value: string | undefined): string {
  return (value?.trim().toLowerCase() ?? '').replace(/^\.+/, '');
}

function getExtensionFromUrl(url: string): string {
  try {
    const parsed = new URL(url, 'http://iptv.local');
    const path = parsed.pathname;
    const lastDot = path.lastIndexOf('.');
    const lastSlash = path.lastIndexOf('/');
    if (lastDot > lastSlash && lastDot !== -1) {
      return path.slice(lastDot + 1);
    }
    return '';
  } catch {
    return '';
  }
}

function normalizeDeclaredMediaExtension(raw: string | null): string {
  const token = normalizeExtensionToken(raw ?? undefined);
  if (!token) return '';
  const aliased = DECLARED_MEDIA_EXTENSION_ALIASES.get(token);
  if (aliased) return aliased;
  return DECLARED_MEDIA_EXTENSIONS.has(token) ? token : '';
}

function getMediaExtensionFromQuery(url: string, queryKeys: readonly string[]): string {
  try {
    const parsed = new URL(url, 'http://iptv.local');
    for (const key of queryKeys) {
      const declared = normalizeDeclaredMediaExtension(parsed.searchParams.get(key));
      if (declared) return declared;
    }
    return '';
  } catch {
    return '';
  }
}

/**
 * Priority: explicit `extension`/`ext` query param (Stalker's convention),
 * then a recognized path extension, then `format`/`container` query param.
 * A non-media path extension is treated as "no extension", not misdetected.
 */
export function getPlaybackMediaExtensionFromUrl(url: string): string {
  const explicitQueryExtension = getMediaExtensionFromQuery(url, EXPLICIT_MEDIA_EXTENSION_QUERY_KEYS);
  if (explicitQueryExtension) {
    return explicitQueryExtension;
  }

  const pathExtension = normalizeExtensionToken(getExtensionFromUrl(url));
  if (DECLARED_MEDIA_EXTENSIONS.has(pathExtension)) {
    return pathExtension;
  }

  const formatQueryExtension = getMediaExtensionFromQuery(url, DECLARED_MEDIA_FORMAT_QUERY_KEYS);
  if (formatQueryExtension) {
    return formatQueryExtension;
  }

  if (NON_MEDIA_URL_EXTENSIONS.has(pathExtension)) {
    return '';
  }

  return pathExtension;
}
