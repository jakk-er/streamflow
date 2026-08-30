export interface ChannelGroup {
    title: string;
}

export interface ChannelTvg {
    id: string;
    name: string;
    url: string;
    logo: string;
    rec: string;
}

export interface ChannelHttp {
    referrer: string;
    userAgent: string;
    origin: string;
}

export interface ChannelCatchup {
    type?: string;
    source?: string;
    days?: string;
}

export interface ChannelDrm {
    type?: string;
    licenseUrl?: string;
    headers?: Record<string, string>;
    data?: string;
}

export interface Channel {
    id: string;
    url: string;
    name: string;
    group: ChannelGroup;
    tvg: ChannelTvg;
    epgParams?: string;
    timeshift?: string;
    catchup?: ChannelCatchup;
    http: ChannelHttp;
    radio: string;
    drm?: ChannelDrm;
    raw?: string;
    channelNumber?: number;
}
