export interface EpgDisplayName {
    lang: string;
    value: string;
}

export interface EpgIcon {
    src: string;
    width?: number;
    height?: number;
}

export interface EpgChannel {
    id: string;
    displayName: EpgDisplayName[];
    icon: EpgIcon[];
    url: string[];
}

export interface EpgProgram {
    id: string;
    channelId: string;
    start: string;
    stop: string;
    title: string;
    description?: string;
    category?: string;
    icon?: string;
}

export interface EpgItem {
    id: string;
    epgId: string;
    title: string;
    lang: string;
    start: string;
    end: string;
    stop: string;
    description: string;
    channelId: string;
    startTimestamp: string;
    stopTimestamp: string;
}
