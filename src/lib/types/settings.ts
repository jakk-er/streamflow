export type Language = 'ar' | 'ary' | 'en' | 'ko' | 'ru' | 'de' | 'es' | 'zh' | 'zhtw' | 'fr' | 'it' | 'tr' | 'ja' | 'nl' | 'by' | 'pl' | 'pt' | 'el' | 'hu';
export type Theme = 'DARK_THEME' | 'LIGHT_THEME' | 'SYSTEM_THEME';
// 'videojs'/'artplayer' removed - never a real dependency or selectable
// value, so no settings blob could contain one. 'embedded-mpv' stays even
// though the engine now activates by file extension, not this preference -
// it WAS once selectable, so an old settings blob can still have it stored;
// dropping the enum variant would fail to deserialize that blob and reset
// the user's entire settings.
export type VideoPlayer = 'html5' | 'embedded-mpv' | 'mpv' | 'vlc';
export type CoverSize = 'small' | 'medium' | 'large';
export type SortChannelsBy = 'default' | 'name-az' | 'name-za';
export type StartupBehavior = 'first-view' | 'restore-last-view';

export interface AppSettings {
    language: Language;
    theme: Theme;
    showEpg: boolean;
    epgSource?: string[];
    enableAnalytics: boolean;
    videoPlayer: VideoPlayer;
    mpvPath?: string;
    vlcPath?: string;
    hideCategoryNames: boolean;
    showChannelNumber: boolean;
    trackWatchHistory: boolean;
    sortChannelsBy: SortChannelsBy;
    coverSize: CoverSize;
    /** User-chosen VOD grid column count - `undefined` (never set) falls
     * back to `VodGrid.svelte`'s own default. Always clamped there against
     * the actual window width before being applied. */
    vodGridColumns?: number;
    /** Playlist to auto-activate on launch; `undefined` = no auto-selection.
     * Applied in `+layout.svelte`'s `onMount`, set/cleared in `PlaylistManager.svelte`. */
    defaultPlaylistId?: string;
}
