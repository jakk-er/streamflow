const BROWSE_LAYOUT_KEY = 'streamflow:live-browse-layout';

export type BrowseLayout = 'list' | 'categories';

function readStoredBrowseLayout(): BrowseLayout {
  try {
    return localStorage.getItem(BROWSE_LAYOUT_KEY) === 'categories' ? 'categories' : 'list';
  } catch {
    // Private browsing / no storage access - just default to the current layout.
    return 'list';
  }
}

function createUiPrefsStore() {
  let browseLayout = $state<BrowseLayout>(readStoredBrowseLayout());

  function setBrowseLayout(layout: BrowseLayout) {
    browseLayout = layout;
    try {
      localStorage.setItem(BROWSE_LAYOUT_KEY, layout);
    } catch {
      // Ignore - the preference just won't persist across restarts.
    }
  }

  return {
    get browseLayout() { return browseLayout; },
    setBrowseLayout,
  };
}

export const uiPrefsStore = createUiPrefsStore();
