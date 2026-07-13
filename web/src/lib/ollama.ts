// Pure Ollama HTTP client: no DOM. Lists local models, streams a chat
// completion parsing Ollama's newline-delimited JSON, and exposes a small
// leading+trailing throttle helper used to cap how often the live view
// re-runs analyzeTrace while tokens are arriving. web/src/ui/livePanel.ts
// wires this into the DOM; this file only knows fetch/streams/JSON.

import type { TraceInput } from "./types";

/** Thrown when Ollama responds with a non-2xx status. Network/CORS failures
 * surface as the fetch-native `TypeError` instead, so callers can tell the
 * two apart. */
export class OllamaHttpError extends Error {
  status: number;

  constructor(status: number, message: string) {
    super(message);
    this.name = "OllamaHttpError";
    this.status = status;
  }
}

/** Running totals accumulated from a chat stream. */
export interface OllamaDelta {
  thinking: string;
  content: string;
}

export interface StreamChatOptions {
  baseUrl: string;
  model: string;
  prompt: string;
  /** Fires after every parsed non-final line with the running totals. */
  onDelta: (accumulated: OllamaDelta) => void;
  /** Fires once, after the `done:true` line, with the final totals. */
  onDone: (final: OllamaDelta) => void;
  signal?: AbortSignal;
}

interface OllamaChatLine {
  message?: { thinking?: string; content?: string };
  done?: boolean;
}

function joinUrl(baseUrl: string, path: string): string {
  return `${baseUrl.replace(/\/+$/, "")}${path}`;
}

/** List locally available model names via `GET /api/tags`. */
export async function listModels(baseUrl: string): Promise<string[]> {
  const res = await fetch(joinUrl(baseUrl, "/api/tags"));
  if (!res.ok) {
    throw new OllamaHttpError(res.status, `GET /api/tags failed with status ${res.status}`);
  }
  const body = (await res.json()) as { models?: { name: string }[] };
  return (body.models ?? []).map((m) => m.name);
}

/**
 * Stream a chat completion from `POST /api/chat` with `stream:true`,
 * parsing the response body as newline-delimited JSON. `thinking` and
 * `content` accumulate independently — each line only ever contributes to
 * the field(s) it actually carries, so a reasoning model's `content`
 * (its answer) never leaks into the `thinking` total. Resolves once the
 * `done:true` line has been parsed and `onDone` has fired. If the stream
 * closes without ever delivering `done:true` (dropped connection, killed
 * server), `onDone` still fires once with the accumulated state, so callers
 * always receive the final full text; an aborted stream instead rejects
 * with the AbortError and never calls `onDone`.
 */
export async function streamChat(opts: StreamChatOptions): Promise<void> {
  const { baseUrl, model, prompt, onDelta, onDone, signal } = opts;

  const res = await fetch(joinUrl(baseUrl, "/api/chat"), {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      model,
      messages: [{ role: "user", content: prompt }],
      stream: true,
    }),
    signal,
  });

  if (!res.ok) {
    throw new OllamaHttpError(res.status, `POST /api/chat failed with status ${res.status}`);
  }
  if (!res.body) {
    throw new Error("Ollama response has no body");
  }

  const reader = res.body.getReader();
  const decoder = new TextDecoder();
  let buffer = "";
  const totals: OllamaDelta = { thinking: "", content: "" };
  let doneFired = false;

  function applyLine(line: string): boolean {
    if (line.trim().length === 0) return false;
    const parsed = JSON.parse(line) as OllamaChatLine;
    if (parsed.message?.thinking) totals.thinking += parsed.message.thinking;
    if (parsed.message?.content) totals.content += parsed.message.content;

    if (parsed.done) {
      doneFired = true;
      onDone({ ...totals });
      return true;
    }
    onDelta({ ...totals });
    return false;
  }

  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      buffer += decoder.decode(value, { stream: true });

      let newlineIndex: number;
      while ((newlineIndex = buffer.indexOf("\n")) !== -1) {
        const line = buffer.slice(0, newlineIndex);
        buffer = buffer.slice(newlineIndex + 1);
        try {
          if (applyLine(line)) return;
        } catch {
          // A malformed mid-stream line (partial write, stray keepalive,
          // etc.) must not kill the whole stream with a raw SyntaxError;
          // skip it and keep reading — the mirror of the trailing-buffer
          // guard below.
        }
      }
    }
    // Stream closed without a trailing newline on the last line.
    if (buffer.trim().length > 0) {
      try {
        applyLine(buffer);
      } catch {
        // A connection dropped mid-line leaves unparseable JSON; the
        // accumulated totals are still delivered via onDone below.
      }
    }
    // Stream closed without a done:true line (dropped connection, killed
    // server): still deliver the final accumulated state exactly once.
    // Aborts never reach here — reader.read() rejects with the AbortError.
    if (!doneFired) {
      onDone({ ...totals });
    }
  } finally {
    // Cancel before releasing so an error/abort path doesn't leave the
    // HTTP body streaming in the background; a rejection here (e.g. the
    // stream already errored from an abort) is expected and swallowed —
    // the real error still propagates from the `try` above.
    void reader.cancel().catch(() => {});
    reader.releaseLock();
  }
}

/**
 * Build the trace analyzed live during streaming: `thinking` is the
 * accumulated `message.thinking` text, or the accumulated `content` when no
 * thinking fragments ever arrived (non-reasoning models); `answer` is the
 * accumulated content when thinking exists, else empty.
 */
export function toTraceInput(prompt: string, delta: OllamaDelta): TraceInput {
  if (delta.thinking.length > 0) {
    return { problem: prompt, thinking: delta.thinking, answer: delta.content };
  }
  return { problem: prompt, thinking: delta.content, answer: "" };
}

/** A throttled wrapper around `fn` plus a `cancel()` to drop any pending
 * trailing call. */
export interface Throttled<Args extends unknown[]> {
  (...args: Args): void;
  cancel(): void;
}

/**
 * Leading+trailing throttle: the first call in an idle period invokes `fn`
 * immediately; calls arriving within `waitMs` of the last invocation are
 * coalesced into a single trailing call — with the most recent arguments —
 * scheduled for the end of the window. At most one invocation fires per
 * `waitMs`, and the last call's arguments are never dropped silently.
 */
export function throttle<Args extends unknown[]>(
  fn: (...args: Args) => void,
  waitMs: number,
): Throttled<Args> {
  let timer: ReturnType<typeof setTimeout> | null = null;
  let lastInvoke = -Infinity;
  let pendingArgs: Args | null = null;

  function invoke(args: Args): void {
    lastInvoke = Date.now();
    fn(...args);
  }

  const throttled = ((...args: Args) => {
    const now = Date.now();
    if (timer === null && now - lastInvoke >= waitMs) {
      invoke(args);
      return;
    }
    pendingArgs = args;
    if (timer === null) {
      const remaining = waitMs - (now - lastInvoke);
      timer = setTimeout(() => {
        timer = null;
        if (pendingArgs) {
          const args2 = pendingArgs;
          pendingArgs = null;
          invoke(args2);
        }
      }, Math.max(remaining, 0));
    }
  }) as Throttled<Args>;

  throttled.cancel = () => {
    if (timer !== null) {
      clearTimeout(timer);
      timer = null;
    }
    pendingArgs = null;
  };

  return throttled;
}
