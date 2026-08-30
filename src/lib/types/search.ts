export type GlobalSearchResultSource = 'xtream' | 'm3u';
export type GlobalSearchContentType = 'live' | 'movie' | 'series';

export interface GlobalSearchPaginationOptions {
    limit?: number;
    offset?: number;
}

export interface GlobalSearchBaseResult {
    sourceType: GlobalSearchResultSource;
    contentType: GlobalSearchContentType;
    id: number | string;
    categoryId: number | string;
    title: string;
    rating: string | null;
    added: string | null;
    posterUrl: string | null;
    xtreamId: number;
    type: GlobalSearchContentType;
    playlistId: string;
    playlistName: string;
}

export interface XtreamGlobalSearchResult extends GlobalSearchBaseResult {
    sourceType: 'xtream';
    description?: string;
    backdropUrl?: string | null;
    epgChannelId?: string | null;
    tvArchive?: number | null;
    tvArchiveDuration?: number | null;
    directSource?: string | null;
    addedAt?: string;
    viewedAt?: string;
    position?: number | null;
}

export interface M3uGlobalSearchResult extends GlobalSearchBaseResult {
    sourceType: 'm3u';
    contentType: 'live';
    type: 'live';
    channelId: string;
    streamUrl: string;
    groupTitle: string;
    radio: string;
    channel: import('./channel').Channel;
}

export type GlobalSearchResult = XtreamGlobalSearchResult | M3uGlobalSearchResult;
