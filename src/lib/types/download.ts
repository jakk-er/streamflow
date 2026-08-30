export type DownloadStatus = 'pending' | 'downloading' | 'paused' | 'completed' | 'failed' | 'canceled';

export interface DownloadMetadataPerson {
    tmdbPersonId?: number;
    name: string;
    role?: string;
    profileUrl?: string;
}

export interface DownloadEpisodeMetadata {
    title?: string;
    plot?: string;
    stillUrl?: string;
    seasonNumber: number;
    episodeNumber: number;
}

export interface DownloadMetadataSnapshot {
    version: 1;
    language: string;
    mediaKind: 'movie' | 'series';
    title: string;
    originalTitle?: string;
    plot?: string;
    releaseDate?: string;
    year?: number;
    durationMinutes?: number;
    genres?: string[];
    rating?: number;
    status?: string;
    posterUrl?: string;
    backdropUrl?: string;
    tmdbId?: number;
    providerCategoryId?: string;
    cast?: DownloadMetadataPerson[];
    creators?: DownloadMetadataPerson[];
    episode?: DownloadEpisodeMetadata;
    enrichedAt?: string;
}

export interface DownloadMetadata {
    id: string;
    url: string;
    filePath: string;
    totalBytes?: number;
    downloadedBytes: number;
    status: DownloadStatus;
    createdAt: string;
}

export interface DownloadProgress {
    downloadId: string;
    downloadedBytes: number;
    totalBytes?: number;
    status: DownloadStatus;
    errorMessage?: string;
    updatedAt: string;
}
