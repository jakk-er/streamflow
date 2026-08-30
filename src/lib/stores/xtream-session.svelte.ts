import { xtreamAuth, xtreamGetCategories, xtreamGetStreams } from '$lib/api';
import type { XtreamUserInfo, XtreamCategory, XtreamStream } from '$lib/types';
import { formatError } from '$lib/utils/errors';

function createXtreamSessionStore() {
  let userInfo = $state<XtreamUserInfo | null>(null);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let categories = $state<XtreamCategory[]>([]);
  let streams = $state<XtreamStream[]>([]);

  async function authenticate(playlistId: string) {
    error = null;
    loading = true;
    try {
      userInfo = await xtreamAuth(playlistId);
    } catch (err) {
      error = formatError(err);
      throw err;
    } finally {
      loading = false;
    }
  }

  async function loadCategories(playlistId: string, streamType: string) {
    error = null;
    loading = true;
    try {
      categories = await xtreamGetCategories(playlistId, streamType);
    } catch (err) {
      error = formatError(err);
    } finally {
      loading = false;
    }
  }

  async function loadStreams(playlistId: string, streamType: string, categoryId?: string) {
    error = null;
    loading = true;
    try {
      streams = await xtreamGetStreams(playlistId, streamType, categoryId);
    } catch (err) {
      error = formatError(err);
    } finally {
      loading = false;
    }
  }

  return {
    get userInfo() { return userInfo; },
    get loading() { return loading; },
    get error() { return error; },
    get categories() { return categories; },
    get streams() { return streams; },
    authenticate,
    loadCategories,
    loadStreams,
  };
}

export const xtreamSessionStore = createXtreamSessionStore();
