export type FavoriteType = 'channel' | 'global';

export interface FavoriteChannel {
    id: string;
    channelId: string;
    playlistId: string;
    favoriteType: FavoriteType;
    createdAt: string;
    /** Denormalized from the channel at read time; absent if the channel was since deleted. */
    channelName?: string;
    channelLogo?: string;
}

export interface WatchHistoryItem {
    id: string;
    channelId?: string;
    playlistId?: string;
    itemType?: string;
    positionSeconds: number;
    totalSeconds: number;
    watchedAt: string;
    channelName?: string;
    channelLogo?: string;
}

export interface RecentlyViewedPlaylist {
    id: string;
    playlistId: string;
    channelId?: string;
    itemType?: string;
    createdAt: string;
}

export interface M3uFavoriteChannel {
    favoriteId: string;
    favoriteIndex: number;
    channel: import('./channel').Channel;
}
