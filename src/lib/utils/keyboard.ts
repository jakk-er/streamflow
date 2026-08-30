/** True while the user is typing into a text field / textarea / select /
 * contenteditable - shortcuts default to standing down there so e.g. typing
 * "f"/"m"/"k"/space/digits into a search box types normally instead of
 * triggering a global fullscreen/mute/channel-number shortcut. */
function isEditableTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  if (target.isContentEditable) return true;
  return target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.tagName === 'SELECT';
}

export function registerShortcut(keys: string, callback: () => void, options?: { allowInInputs?: boolean }): () => void {
  const handler = (event: KeyboardEvent) => {
    if (!options?.allowInInputs && isEditableTarget(event.target)) return;

    const parts = keys.toLowerCase().split('+').map(k => k.trim());
    const key = parts.pop()!;
    const modifiers = parts;

    const ctrl = modifiers.includes('ctrl');
    const shift = modifiers.includes('shift');
    const alt = modifiers.includes('alt');
    const meta = modifiers.includes('cmd') || modifiers.includes('meta');

    const isMac = typeof navigator !== 'undefined' && /mac|iphone|ipad|ipod/.test(navigator.platform.toLowerCase());

    if (ctrl && !event.ctrlKey) return;
    if (shift && !event.shiftKey) return;
    if (alt && !event.altKey) return;
    if (meta && !event.metaKey) return;

    if (isMac && meta && !event.metaKey) return;
    if (!isMac && ctrl && !event.ctrlKey) return;

    const targetKey = event.key?.toLowerCase() ?? '';
    const expectedKey = key === 'escape' ? 'escape' : key.toLowerCase();

    if (targetKey !== expectedKey) return;

    event.preventDefault();
    event.stopPropagation();
    callback();
  };

  window.addEventListener('keydown', handler);
  return () => window.removeEventListener('keydown', handler);
}

export function registerChannelNumber(callback: (num: number) => void): () => void {
  let buffer = '';
  let timeout: number | null = null;

  const handler = (event: KeyboardEvent) => {
    if (isEditableTarget(event.target)) return;
    if (event.ctrlKey || event.metaKey || event.altKey || event.shiftKey) return;
    if (event.key.length !== 1) return;
    const code = event.key;
    if (!/[0-9]/.test(code)) return;

    event.preventDefault();

    if (timeout !== null) {
      window.clearTimeout(timeout);
    }

    buffer += code;

    if (buffer.length >= 2 || parseInt(buffer, 10) > 9) {
      const num = parseInt(buffer, 10);
      callback(num);
      buffer = '';
      timeout = null;
      return;
    }

    timeout = window.setTimeout(() => {
      const num = parseInt(buffer, 10);
      callback(num);
      buffer = '';
      timeout = null;
    }, 500);
  };

  window.addEventListener('keydown', handler);
  return () => {
    window.removeEventListener('keydown', handler);
    if (timeout !== null) {
      window.clearTimeout(timeout);
    }
  };
}
