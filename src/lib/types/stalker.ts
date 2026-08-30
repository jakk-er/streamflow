export type PortalMode = 'full' | 'simple';
export type StalkerContentType = 'itv' | 'radio' | 'vod' | 'series';
export type StalkerPortalAction = 'get_categories' | 'get_genres' | 'create_link' | 'get_ordered_list' | 'get_all_channels' | 'favorites' | 'handshake' | 'do_auth' | 'get_short_epg' | 'get_epg_info';
export type StalkerAuthFailure = 'login-required' | 'login-rejected' | 'device-conflict' | 'blocked' | 'auth-failed';

export interface StalkerSession {
    id: string;
    playlistId: string;
    token: string;
    watchdogTimeout?: number;
    expiresAt?: string;
    deviceId?: string;
    profileData?: unknown;
    createdAt: string;
    updatedAt: string;
}

export interface StalkerToken {
    token: string;
    identityFingerprint: string;
}

export interface StalkerProfile {
    status: number;
    watchdogTimeout: number;
    timeslot: number;
    msg?: string;
    blockMsg?: string;
}

export interface StalkerCredentials {
    username?: string;
    password?: string;
    deviceId1?: string;
    deviceId2?: string;
    signature1?: string;
    signature2?: string;
}

export interface StalkerDeviceInfo {
    macAddress: string;
    serialNumber?: string;
    deviceId1?: string;
    deviceId2?: string;
}

/** The durable bits of a Stalker session, persisted onto the playlist row. */
export interface StalkerSessionInfo {
    token: string;
    endpoint: string;
    fullPortal: boolean;
    watchdogTimeout: number;
    timeslot: number;
    notValid: boolean;
    loginCompleted: boolean;
    sessionFingerprint: string;
}

/**
 * Outcome of an authenticate/do_auth call. Login-required and refusal states
 * are not thrown as errors - the UI must branch on them, e.g. showing
 * `StalkerLoginForm` for `login-required`.
 */
export type StalkerAuthOutcome =
    | { kind: 'success'; session: StalkerSessionInfo }
    | { kind: 'loginRequired' }
    | { kind: 'loginRejected'; message: string }
    | { kind: 'deviceConflict'; message: string }
    | { kind: 'blocked'; message: string };

export interface StalkerCategory {
    id: string;
    title: string;
    alias?: string;
}

/** A raw catalog row from `get_ordered_list` (ITV/radio/VOD/series). */
export interface StalkerContentItem {
    id: string;
    name: string;
    cmd?: string;
    screenshotUri?: string;
    cover?: string;
    description?: string;
    actors?: string;
    director?: string;
    year?: string;
    ratingImdb?: string;
    categoryId?: string;
    isSeries: boolean;
    series?: string[];
    hasFiles?: number;
    useHttpTmpLink?: string;
    useLoadBalancing?: string;
    genresStr?: string;
}

export interface StalkerContentPage<T = StalkerContentItem> {
    data: T[];
    totalItems: number;
    maxPageItems: number;
    curPage: number;
    totalPages: number;
}

export interface StalkerSeasonItem {
    id: string;
    name: string;
    cmd?: string;
    series: string[];
    screenshotUri?: string;
    year?: string;
    ratingImdb?: string;
}

export interface StalkerChannel {
    id: string;
    name: string;
    cmd?: string;
    logo?: string;
    tvGenreId?: string;
    xmltvId?: string;
    useHttpTmpLink?: string;
    useLoadBalancing?: string;
    number?: number;
}

export interface StalkerEpgProgram {
    id: string;
    chId?: string;
    name: string;
    descr?: string;
    startTimestamp: number;
    stopTimestamp: number;
}

export type PortalActivityType = 'live' | 'movie' | 'series';
