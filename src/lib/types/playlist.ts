export type PlaylistType = 'm3u' | 'xtream' | 'stalker';
export type PlaylistUpdateState = 'updated' | 'in_progress' | 'not_updated';
export type PlaylistUpdateStatus = 'updated' | 'failed' | 'skipped';

export interface StalkerAccountInfo {
    login?: string;
    expireDate?: number;
    tariffPlanName?: string;
    status?: number;
}

export interface ParsedPlaylistHeader {
    attrs: Record<string, string | undefined>;
    raw: string;
}

export interface ParsedPlaylistItem {
    name: string;
    tvg: {
        id: string;
        name: string;
        url: string;
        logo: string;
        rec: string;
    };
    group: {
        title: string;
    };
    http: {
        referrer: string;
        userAgent: string;
    };
    url?: string;
    raw: string;
    catchup?: {
        type?: string;
        source?: string;
        days?: string;
    };
    timeshift?: string;
    radio?: string;
    drm?: unknown; // TODO: verify type from channel-drm.interface.ts
}

export interface ParsedPlaylist {
    header: ParsedPlaylistHeader;
    items: ParsedPlaylistItem[];
}

export interface Playlist {
    _id: string;
    title: string;
    filename?: string;
    playlist?: unknown;
    importDate: string;
    lastUsage: string;
    favorites?: (string | unknown)[];
    items?: unknown[];
    header?: unknown;
    count: number;
    url?: string;
    userAgent?: string;
    referrer?: string;
    origin?: string;
    filePath?: string;
    epgUrls?: string[];
    detectedEpgUrls?: string[];
    manualEpgUrls?: string[];
    disabledEpgUrls?: string[];
    autoRefresh: boolean;
    updateDate?: number;
    updateState?: PlaylistUpdateState;
    position?: number;
    isTemporary?: boolean;
    serverUrl?: string;
    username?: string;
    password?: string;
    macAddress?: string;
    portalUrl?: string;
    recentlyViewed?: unknown[];
    isFullStalkerPortal?: boolean;
    serverTimezone?: string;
    stalkerToken?: string;
    stalkerSessionIdentity?: string;
    stalkerWatchdogTimeout?: number;
    stalkerTimeslot?: number;
    stalkerSerialNumber?: string;
    stalkerDeviceId1?: string;
    stalkerDeviceId2?: string;
    stalkerSignature1?: string;
    stalkerSignature2?: string;
    stalkerAccountInfo?: StalkerAccountInfo;
    hiddenGroupTitles?: string[];
    playlistType?: PlaylistType;
    stalkerLoginCompleted?: boolean;
    stalkerNotValid?: boolean;
    stalkerEndpoint?: string;
}

export interface PlaylistMeta {
    count: number;
    title: string;
    filename?: string;
    _id: string;
    url?: string;
    importDate: string;
    userAgent?: string;
    referrer?: string;
    origin?: string;
    filePath?: string;
    epgUrls?: string[];
    detectedEpgUrls?: string[];
    manualEpgUrls?: string[];
    disabledEpgUrls?: string[];
    updateDate?: number;
    updateState?: PlaylistUpdateState;
    position?: number;
    autoRefresh: boolean;
    favorites?: unknown[];
    serverUrl?: string;
    username?: string;
    password?: string;
    macAddress?: string;
    hiddenGroupTitles?: string[];
    portalUrl?: string;
    recentlyViewed?: unknown[];
    isFullStalkerPortal?: boolean;
    stalkerSerialNumber?: string;
    stalkerDeviceId1?: string;
    stalkerDeviceId2?: string;
    stalkerSignature1?: string;
    stalkerSignature2?: string;
}

export interface StalkerPlaylistSessionMetadata {
    stalkerToken: string;
    stalkerSessionIdentity?: string;
    stalkerWatchdogTimeout?: number;
    stalkerTimeslot?: number;
    stalkerAccountInfo?: Playlist['stalkerAccountInfo'];
}

export interface PlaylistMetaUpdate extends PlaylistMeta {
    stalkerSessionPatch?: StalkerPlaylistSessionMetadata | null;
}

export interface PlaylistAutoUpdate {
    playlists: Playlist[];
    outcomes: PlaylistUpdateOutcome[];
}

export interface PlaylistUpdateOutcome {
    playlistId: string;
    status: PlaylistUpdateStatus;
}
