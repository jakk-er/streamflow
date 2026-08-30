export type TmdbMediaType = 'movie' | 'tv' | 'person';
export type TmdbSeriesStatus = 'returning' | 'planned' | 'in-production' | 'ended' | 'canceled' | 'pilot';

export interface TmdbSettings {
    enabled: boolean;
    apiKey?: string;
}

export interface TmdbEnrichedCastMember {
    name: string;
    character?: string;
    profileUrl: string | null;
    tmdbPersonId?: number;
}

export interface TmdbRecommendation {
    tmdbId: number;
    title: string;
    year: number | null;
    posterUrl: string | null;
}

export interface TmdbCacheStats {
    entries: number;
    bytes: number;
}

export interface TmdbCacheEntry {
    mediaType: TmdbMediaType;
    lookupKey: string;
    language: string;
    tmdbId: number | null;
    payload: string | null;
    fetchedAt?: string;
}

export interface TmdbMovie {
    id: string;
    tmdbId?: number;
    title: string;
    overview?: string;
    posterPath?: string;
    backdropPath?: string;
    voteAverage?: number;
    releaseDate?: string;
    genreIds: number[];
    mediaType: TmdbMediaType;
}

export interface TmdbTvShow {
    id: string;
    tmdbId?: number;
    title: string;
    overview?: string;
    posterPath?: string;
    backdropPath?: string;
    voteAverage?: number;
    releaseDate?: string;
    genreIds: number[];
    mediaType: TmdbMediaType;
    status?: TmdbSeriesStatus;
}

export interface TmdbCast {
    id: string;
    tmdbPersonId?: number;
    name: string;
    character?: string;
    profileUrl?: string;
    mediaId: string;
    mediaType: TmdbMediaType;
}

export interface TmdbCrew {
    id: string;
    tmdbPersonId?: number;
    name: string;
    job?: string;
    department?: string;
    profileUrl?: string;
    mediaId: string;
    mediaType: TmdbMediaType;
}

export interface TmdbSearchResult {
    id: string;
    tmdbId: number;
    title: string;
    overview?: string;
    posterPath?: string;
    backdropPath?: string;
    voteAverage?: number;
    releaseDate?: string;
    genreIds: number[];
    mediaType: TmdbMediaType;
}
