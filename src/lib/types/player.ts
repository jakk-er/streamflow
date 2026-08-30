export type PlaybackContentType = 'vod' | 'episode';
export type EmbeddedMpvSessionStatus = 'idle' | 'loading' | 'playing' | 'paused' | 'ended' | 'error' | 'closed';

/** Set on `playerStore.play(...)`'s optional trailing param for VOD playback
 * only - its presence is what gates resume tracking in `VideoPlayer.svelte`.
 * Live TV / external playback never set it, so tracking never activates there. */
export interface VodResumeContext {
    playlistId: string;
    contentType: 'movie' | 'series';
    /** The movie's own id, or the SERIES' id for an episode (never an
     * episode id itself) - matches `vod_watch_progress`'s own key. */
    vodItemId: string;
    title: string;
    cover?: string;
    episode?: {
        id: string;
        seasonNumber: number;
        episodeNumber?: number;
        title: string;
    };
    /** Seek-to-on-load, for a resumed title. */
    startPositionSeconds?: number;
    /** Every episode after the current one, in play order, across remaining
     * seasons - precomputed by `findUpcomingEpisodes` so `VideoPlayer.svelte`
     * can autoplay through without needing the full `SeriesDetails`. On
     * completion it pops the next entry and passes the rest along as the new
     * resume context. Empty/absent for a movie or a series with nothing left. */
    upcomingEpisodes?: {
        id: string;
        seasonNumber: number;
        episodeNumber?: number;
        title: string;
        streamUrl?: string;
        directSource?: string;
        cmd?: string;
        /** Stalker's `series=` param - which episode inside a season's shared
         * `cmd` template `create_link` should resolve. Without it `create_link`
         * falls back to a `type` the portal doesn't recognize (see content.rs). */
        seriesParam?: string;
    }[];
}

export interface PlaybackPosition {
    id: string;
    contentXtreamId: number;
    contentType: PlaybackContentType;
    seriesXtreamId?: number;
    seasonNumber?: number;
    episodeNumber?: number;
    positionSeconds: number;
    totalSeconds?: number;
    playlistId?: string;
    updatedAt?: string;
    sessionId?: string;
}

export interface EmbeddedMpvSession {
    id: string;
    title: string;
    streamUrl: string;
    status: EmbeddedMpvSessionStatus;
    positionSeconds: number;
    durationSeconds: number | null;
    volume: number;
    startedAt: string;
    updatedAt: string;
    error?: string | null;
}
