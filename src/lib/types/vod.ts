import type { StalkerContentItem } from './stalker';

export type StreamType = 'live' | 'movie' | 'series' | 'radio';

/** The two catalog kinds the local VOD cache tracks - matches the Rust
 * `VodContentType` enum's `rename_all = "lowercase"` wire shape exactly. */
export type VodContentType = 'movie' | 'series';

/** One row from the local VOD/series catalog cache (`vod_get_categories`/
 * `vod_get_items`) - list-level fields only. Full detail (plot/cast,
 * seasons/episodes) is fetched and cached lazily on first view. */
export interface VodCatalogItem {
    /** The provider's own id (Xtream `stream_id`/`series_id` as a string,
     * Stalker's own `id`) - not a locally-generated UUID. */
    id: string;
    contentType: VodContentType;
    categoryId?: string;
    name: string;
    cover?: string;
    rating?: string;
    genre?: string;
    releaseDate?: string;
    containerExtension?: string;
    /** Populated only for Stalker-sourced rows - Stalker has no id-based
     * lookup endpoint, so `stalkerGetVodInfo`/`stalkerGetSeriesInfo` need the
     * full original row back. */
    stalkerItem?: StalkerContentItem;
}

/** One live-fetched page from `vodGetItemsLive` (a Stalker category browse).
 * `items` matches `vodGetItems`'s shape, so downstream code doesn't care
 * whether a page came from cache or a live fetch. */
export interface VodLivePage {
    items: VodCatalogItem[];
    page: number;
    totalPages: number;
    totalItems: number;
}

/** One row from `vod_watch_progress` ("Continue Watching"). For a movie,
 * `episode*` are absent. For a series, `vodItemId` names the series itself
 * and `episode*` names the relevant episode - the one to resume
 * (`positionSeconds > 0`) or the next unwatched one (`positionSeconds === 0`). */
export interface VodWatchProgress {
    id: string;
    playlistId: string;
    contentType: VodContentType;
    vodItemId: string;
    episodeId?: string;
    seasonNumber?: number;
    episodeNumber?: number;
    episodeTitle?: string;
    positionSeconds: number;
    totalSeconds: number;
    title: string;
    cover?: string;
    updatedAt: string;
}

export interface EpisodeInfo {
    durationSecs?: number;
    rating?: number;
}

export interface SeasonEpisode {
    id: string;
    episodeNum?: number;
    title: string;
    season: number;
    containerExtension?: string;
    info?: EpisodeInfo;
    cover?: string;
    plot?: string;
    streamUrl?: string;
    directSource?: string;
    // Stalker-only: raw `cmd`/`series` index for re-resolving a fresh
    // playback URL right before play - see `VodDetails.cmd`'s comment.
    cmd?: string;
    seriesParam?: string;
}

export interface SeasonInfo {
    id?: number;
    name: string;
    seasonNumber: number;
    episodeCount?: number;
    airDate?: string;
    cover?: string;
}

export interface SeriesDetails {
    info: VodDetails;
    seasons: SeasonInfo[];
    episodes: Record<string, SeasonEpisode[]>;
}

export interface VodDetails {
    id: string;
    name: string;
    streamType: StreamType;
    containerExtension?: string;
    directSource?: string;
    seriesId?: number;
    seasonNumber?: number;
    episodeNumber?: number;
    cover?: string;
    streamUrl?: string;
    plot?: string;
    cast?: string;
    rating?: string;
    genre?: string;
    releaseDate?: string;
    tmdbId?: number;
    seasons?: SeasonInfo[];
    episodes?: Record<string, SeasonEpisode[]>;
    // Stalker-only: see `SeasonEpisode.cmd`. `stalkerContentType` is the
    // Stalker `type` ("vod"/"series") this was fetched under - needed to
    // resolve this item's (and its episodes') `cmd`, since a `type=vod` row
    // can itself be flagged as a series, independent of `streamType`.
    cmd?: string;
    useHttpTmpLink?: string;
    useLoadBalancing?: string;
    stalkerContentType?: string;
}

export interface VodItem {
    id: string;
    name: string;
    streamType: StreamType;
    containerExtension?: string;
    directSource?: string;
    seriesId?: number;
    seasonNumber?: number;
    episodeNumber?: number;
    cover?: string;
    streamIcon?: string;
    streamUrl?: string;
    plot?: string;
    cast?: string;
    rating?: string;
    genre?: string;
    releaseDate?: string;
    tmdbId?: number;
}

export interface VodSource {
    id: string;
    vodId: string;
    playlistId: string;
    streamId: number;
    name: string;
    streamType: StreamType;
    containerExtension?: string;
    directSource?: string;
}

export interface SeasonMarker {
    seasonNumber: number;
    title?: string;
}

export interface VodDetailsItem {
    readonly type: 'xtream' | 'stalker';
    data: unknown;
    playlistId: string;
    vodId?: number;
    cmd?: string;
}
