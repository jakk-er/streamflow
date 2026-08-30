// Tauri command errors arrive as one string with a Rust-side framing prefix
// (e.g. "ApiError: Authentication failed: ..."). Useful in logs, but reads
// as raw internals to a user - strip it before display.
const NOISE_PREFIXES = [
  /^ApiError:\s*/i,
  /^CommandError:\s*/i,
  /^Invalid response:\s*/i,
  /^Authentication failed:\s*/i,
];

const MAX_LENGTH = 300;

export function formatError(err: unknown): string {
  let message = err instanceof Error ? err.message : String(err);

  let stripped = true;
  while (stripped) {
    stripped = false;
    for (const prefix of NOISE_PREFIXES) {
      const next = message.replace(prefix, '');
      if (next !== message) {
        message = next;
        stripped = true;
      }
    }
  }

  message = message.trim();
  if (message.length > MAX_LENGTH) {
    message = message.slice(0, MAX_LENGTH).trimEnd() + '…';
  }

  return message || 'Something went wrong';
}
