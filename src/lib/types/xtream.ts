export type XtreamStreamType = 'live' | 'movie' | 'series' | 'radio';
export type XtreamContentType = 'live' | 'movie' | 'series';
export type XtreamCategoryType = 'live' | 'movie' | 'series' | 'radio';

export interface XtreamCredentials {
    serverUrl: string;
    username: string;
    password: string;
}

export interface XtreamCategory {
    id?: number;
    categoryId: string;
    categoryName: string;
    parentId: number;
    count?: number;
}

export interface XtreamStream {
    num: number;
    name: string;
    streamType: XtreamStreamType;
    streamId: number;
    streamIcon: string;
    added: string;
    categoryId: string;
    customSid: string;
    directSource: string;
    epgChannelId?: string;
    tvArchive?: number;
    tvArchiveDuration?: number;
    ratingImdb?: string;
    xtreamId?: number;
    type?: XtreamContentType;
    addedAt?: number;
    containerExtension?: string;
    rating?: string;
    year?: string;
    cover?: string;
    genre?: string;
    releaseDate?: string;
    streamUrl?: string;
    seriesId?: number;
    isSeries?: boolean;
}

export interface XtreamLiveStream extends XtreamStream {
    streamType: 'live';
    epgChannelId?: string;
    tvArchive: number;
    tvArchiveDuration: number;
}

export interface XtreamUserInfo {
    username: string;
    password: string;
    message?: string;
    auth: number;
    status: string;
    expDate?: string;
    maxConnections?: string;
    allowedOutputFormats?: string[];
}
