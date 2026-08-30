import { fetchEpg, getEpgForChannel, getCurrentProgram } from '$lib/api';
import type { EpgProgram } from '$lib/types';
import { formatError } from '$lib/utils/errors';

function createEpgStore() {
  let programs = $state<EpgProgram[]>([]);
  let currentProgram = $state<EpgProgram | null>(null);
  let loading = $state(false);
  let error = $state<string | null>(null);

  async function fetch(playlistId: string, epgUrl: string) {
    error = null;
    loading = true;
    try {
      await fetchEpg(playlistId, epgUrl);
    } catch (err) {
      error = formatError(err);
      throw err;
    } finally {
      loading = false;
    }
  }

  async function loadForChannel(channelId: string, start: string, end: string) {
    error = null;
    loading = true;
    try {
      programs = await getEpgForChannel(channelId, start, end);
    } catch (err) {
      error = formatError(err);
    } finally {
      loading = false;
    }
  }

  async function loadCurrent(channelId: string) {
    error = null;
    loading = true;
    try {
      currentProgram = await getCurrentProgram(channelId);
    } catch (err) {
      error = formatError(err);
    } finally {
      loading = false;
    }
  }

  const programsByChannel = $derived(() => {
    const map = new Map<string, EpgProgram[]>();
    for (const prog of programs) {
      const list = map.get(prog.channelId) ?? [];
      list.push(prog);
      map.set(prog.channelId, list);
    }
    return map;
  });

  return {
    get programs() { return programs; },
    get currentProgram() { return currentProgram; },
    get loading() { return loading; },
    get error() { return error; },
    get programsByChannel() { return programsByChannel(); },
    fetch,
    loadForChannel,
    loadCurrent,
  };
}

export const epgStore = createEpgStore();
