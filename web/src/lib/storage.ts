// Safe localStorage access wrapped in try/catch. Some browsers throw on any
// localStorage touch instead of just no-opping (Firefox "block all
// cookies", some private-mode configurations); left unguarded, that would
// throw out of component initialization and blank the app. Falls back to
// `null`/no-op — settings simply won't persist that session.

export function readStorage(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

export function writeStorage(key: string, value: string): void {
  try {
    localStorage.setItem(key, value);
  } catch {
    // Storage blocked — settings just won't persist this session.
  }
}
