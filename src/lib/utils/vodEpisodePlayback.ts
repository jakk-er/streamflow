import type { SeasonEpisode, SeriesDetails, StalkerContentType, VodResumeContext } from '$lib/types';
import { stalkerResolveVodEpisode } from '$lib/api';
import { wrapUrlThroughStreamProxy } from '$lib/utils/streamProxy';

/**
 * Resolves a playable, proxy-wrapped URL for one series episode - shared by
 * the user-click path (`SeasonTabs.svelte`) and auto-advance
 * (`VideoPlayer.svelte`) so they can't drift. An episode's `directSource`
 * was resolved via `create_link` when the series detail loaded and may
 * already be a dead temporary link by play time - re-resolve fresh from
 * `cmd`/`seriesParam` for Stalker. Returns `null` if nothing is playable.
 */
export async function resolveEpisodeUrl(
  playlistId: string,
  isStalker: boolean,
  episode: { directSource?: string; streamUrl?: string; cmd?: string; seriesParam?: string },
  stalkerContentType?: StalkerContentType
): Promise<{ url: string; extension?: string } | null> {
  let resolvedUrl = episode.directSource || episode.streamUrl || '';
  if (isStalker && episode.cmd) {
    resolvedUrl = await stalkerResolveVodEpisode(playlistId, stalkerContentType ?? 'series', episode.cmd, episode.seriesParam);
  }
  if (!resolvedUrl) return null;
  return await wrapUrlThroughStreamProxy(playlistId, resolvedUrl, true);
}

/**
 * Every episode after `currentEpisodeId` in play order, across remaining
 * seasons - autoplay/Continue Watching crosses season boundaries, so this
 * doesn't stop at season end. Seasons are walked in `detail.seasons`' array
 * order (matches `SeasonTabs.svelte`). Returns the full remaining queue, not
 * just the next episode - see `VodResumeContext.upcomingEpisodes`.
 */
export function findUpcomingEpisodes(detail: SeriesDetails, currentSeasonNumber: number, currentEpisodeId: string): SeasonEpisode[] {
  const result: SeasonEpisode[] = [];
  const currentSeasonEpisodes = detail.episodes[String(currentSeasonNumber)] ?? [];
  const indexInSeason = currentSeasonEpisodes.findIndex((e) => e.id === currentEpisodeId);
  if (indexInSeason !== -1) {
    result.push(...currentSeasonEpisodes.slice(indexInSeason + 1));
  }

  const seasonIndex = detail.seasons.findIndex((s) => s.seasonNumber === currentSeasonNumber);
  if (seasonIndex !== -1) {
    for (let i = seasonIndex + 1; i < detail.seasons.length; i++) {
      result.push(...(detail.episodes[String(detail.seasons[i].seasonNumber)] ?? []));
    }
  }
  return result;
}

/** Builds `VodResumeContext.upcomingEpisodes` from a list of raw
 * `SeasonEpisode`s - the shape `findUpcomingEpisodes` returns and the shape
 * the resume context needs are close but not identical (`episodeNum` vs
 * `episodeNumber`, `season` vs `seasonNumber`), so this is the one place
 * that reconciles them. */
export function toResumeEpisodeQueue(episodes: SeasonEpisode[]): NonNullable<VodResumeContext['upcomingEpisodes']> {
  return episodes.map((episode) => ({
    id: episode.id,
    seasonNumber: episode.season,
    episodeNumber: episode.episodeNum,
    title: episode.title,
    streamUrl: episode.streamUrl,
    directSource: episode.directSource,
    cmd: episode.cmd,
    seriesParam: episode.seriesParam,
  }));
}
