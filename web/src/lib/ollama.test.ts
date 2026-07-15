// Pure-logic tests for the Ollama client: NDJSON stream parsing (thinking
// accumulation, content fallback, chunk-boundary buffering, done handling),
// the leading+trailing throttle helper, and the trace-assembly mapping. No
// DOM here and no real network — fetch is stubbed per test with a Response
// wrapping a hand-built ReadableStream.

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  listModels,
  OllamaHttpError,
  OllamaTimeoutError,
  streamChat,
  throttle,
  toTraceInput,
} from "./ollama";

function streamOf(chunks: string[]): ReadableStream<Uint8Array> {
  const encoder = new TextEncoder();
  return new ReadableStream<Uint8Array>({
    start(controller) {
      for (const chunk of chunks) {
        controller.enqueue(encoder.encode(chunk));
      }
      controller.close();
    },
  });
}

function stubFetch(response: Response): ReturnType<typeof vi.fn> {
  const fn = vi.fn().mockResolvedValue(response);
  vi.stubGlobal("fetch", fn);
  return fn;
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("listModels", () => {
  it("GETs /api/tags and returns model names", async () => {
    const fetchMock = stubFetch(
      new Response(
        JSON.stringify({ models: [{ name: "llama3" }, { name: "qwen2.5-coder:7b" }] }),
        { status: 200 },
      ),
    );

    const names = await listModels("http://localhost:11434");

    expect(names).toEqual(["llama3", "qwen2.5-coder:7b"]);
    expect(fetchMock).toHaveBeenCalledWith(
      "http://localhost:11434/api/tags",
      expect.objectContaining({ signal: expect.any(AbortSignal) }),
    );
  });

  it("trims a trailing slash on the base URL", async () => {
    const fetchMock = stubFetch(new Response(JSON.stringify({ models: [] }), { status: 200 }));

    await listModels("http://localhost:11434/");

    expect(fetchMock).toHaveBeenCalledWith(
      "http://localhost:11434/api/tags",
      expect.objectContaining({ signal: expect.any(AbortSignal) }),
    );
  });

  it("throws OllamaHttpError with the status on a non-2xx response", async () => {
    stubFetch(new Response("nope", { status: 500 }));

    await expect(listModels("http://localhost:11434")).rejects.toMatchObject(
      expect.objectContaining({ status: 500 }),
    );
    await expect(listModels("http://localhost:11434")).rejects.toBeInstanceOf(OllamaHttpError);
  });

  it("propagates a network/CORS TypeError untouched", async () => {
    const fn = vi.fn().mockRejectedValue(new TypeError("Failed to fetch"));
    vi.stubGlobal("fetch", fn);

    await expect(listModels("http://localhost:11434")).rejects.toBeInstanceOf(TypeError);
  });

  it("times out a hung probe with OllamaTimeoutError", async () => {
    vi.useFakeTimers();
    try {
      // A fetch that never resolves on its own — the real failure mode is a
      // stalled private-network-access preflight. It rejects only when aborted.
      vi.stubGlobal(
        "fetch",
        vi.fn(
          (_url: string, init: RequestInit) =>
            new Promise<Response>((_resolve, reject) => {
              init.signal?.addEventListener("abort", () =>
                reject(new DOMException("aborted", "AbortError")),
              );
            }),
        ),
      );

      const promise = listModels("http://localhost:11434", 10_000);
      const assertion = expect(promise).rejects.toBeInstanceOf(OllamaTimeoutError);
      await vi.advanceTimersByTimeAsync(10_000);
      await assertion;
    } finally {
      vi.useRealTimers();
    }
  });
});

describe("streamChat: NDJSON parsing", () => {
  it("accumulates message.thinking fragments and ignores content for the thinking field", async () => {
    const lines = [
      '{"message":{"thinking":"Step 1. ","content":""},"done":false}\n',
      '{"message":{"thinking":"Step 2.","content":""},"done":false}\n',
      '{"message":{"thinking":"","content":"42"},"done":false}\n',
      '{"message":{"thinking":"","content":""},"done":true}\n',
    ];
    stubFetch(new Response(streamOf(lines), { status: 200 }));

    const onDelta = vi.fn();
    const onDone = vi.fn();

    await streamChat({
      baseUrl: "http://localhost:11434",
      model: "deepseek-r1",
      prompt: "2+2?",
      onDelta,
      onDone,
    });

    expect(onDelta).toHaveBeenNthCalledWith(1, { thinking: "Step 1. ", content: "" });
    expect(onDelta).toHaveBeenNthCalledWith(2, { thinking: "Step 1. Step 2.", content: "" });
    expect(onDelta).toHaveBeenNthCalledWith(3, { thinking: "Step 1. Step 2.", content: "42" });
    expect(onDone).toHaveBeenCalledTimes(1);
    expect(onDone).toHaveBeenCalledWith({ thinking: "Step 1. Step 2.", content: "42" });
  });

  it("accumulates message.content as the only signal for a non-reasoning model", async () => {
    const lines = [
      '{"message":{"content":"Hello"},"done":false}\n',
      '{"message":{"content":" world"},"done":false}\n',
      '{"message":{"content":""},"done":true}\n',
    ];
    stubFetch(new Response(streamOf(lines), { status: 200 }));

    const onDelta = vi.fn();
    const onDone = vi.fn();

    await streamChat({
      baseUrl: "http://localhost:11434",
      model: "tinyllama",
      prompt: "hi",
      onDelta,
      onDone,
    });

    expect(onDelta).toHaveBeenNthCalledWith(1, { thinking: "", content: "Hello" });
    expect(onDelta).toHaveBeenNthCalledWith(2, { thinking: "", content: "Hello world" });
    expect(onDone).toHaveBeenCalledWith({ thinking: "", content: "Hello world" });
  });

  it("buffers a JSON line split across chunk boundaries", async () => {
    const full =
      '{"message":{"thinking":"chunked","content":""},"done":false}\n' +
      '{"message":{"thinking":"!","content":""},"done":true}\n';
    // Split mid-object, well inside the first line.
    const splitAt = full.indexOf('"chunked"') + 3;
    stubFetch(
      new Response(streamOf([full.slice(0, splitAt), full.slice(splitAt)]), { status: 200 }),
    );

    const onDelta = vi.fn();
    const onDone = vi.fn();

    await streamChat({
      baseUrl: "http://localhost:11434",
      model: "deepseek-r1",
      prompt: "p",
      onDelta,
      onDone,
    });

    expect(onDelta).toHaveBeenCalledTimes(1);
    expect(onDelta).toHaveBeenCalledWith({ thinking: "chunked", content: "" });
    expect(onDone).toHaveBeenCalledWith({ thinking: "chunked!", content: "" });
  });

  it("resolves after onDone and does not call onDelta for the done:true line", async () => {
    const lines = [
      '{"message":{"content":"a"},"done":false}\n',
      '{"message":{"content":"b"},"done":true}\n',
    ];
    stubFetch(new Response(streamOf(lines), { status: 200 }));

    const onDelta = vi.fn();
    const onDone = vi.fn();

    await streamChat({
      baseUrl: "http://localhost:11434",
      model: "tinyllama",
      prompt: "p",
      onDelta,
      onDone,
    });

    expect(onDelta).toHaveBeenCalledTimes(1);
    expect(onDelta).toHaveBeenCalledWith({ thinking: "", content: "a" });
    expect(onDone).toHaveBeenCalledTimes(1);
    expect(onDone).toHaveBeenCalledWith({ thinking: "", content: "ab" });
  });

  it("calls onDone once with the accumulated text when the stream ends without done:true", async () => {
    // A dropped connection / killed server can close the body cleanly with
    // no final done:true line; the accumulated text must still be delivered.
    const lines = [
      '{"message":{"content":"partial "},"done":false}\n',
      '{"message":{"content":"answer"},"done":false}\n',
    ];
    stubFetch(new Response(streamOf(lines), { status: 200 }));

    const onDelta = vi.fn();
    const onDone = vi.fn();

    await streamChat({
      baseUrl: "http://localhost:11434",
      model: "tinyllama",
      prompt: "p",
      onDelta,
      onDone,
    });

    expect(onDone).toHaveBeenCalledTimes(1);
    expect(onDone).toHaveBeenCalledWith({ thinking: "", content: "partial answer" });
  });

  it("does not double-fire onDone for a final done:true line missing its trailing newline", async () => {
    const lines = [
      '{"message":{"content":"a"},"done":false}\n',
      '{"message":{"content":"b"},"done":true}', // no trailing \n
    ];
    stubFetch(new Response(streamOf(lines), { status: 200 }));

    const onDone = vi.fn();

    await streamChat({
      baseUrl: "http://localhost:11434",
      model: "tinyllama",
      prompt: "p",
      onDelta: vi.fn(),
      onDone,
    });

    expect(onDone).toHaveBeenCalledTimes(1);
    expect(onDone).toHaveBeenCalledWith({ thinking: "", content: "ab" });
  });

  it("skips an unparseable line mid-stream and continues to completion", async () => {
    // A malformed mid-stream line (partial write, stray keepalive, etc.)
    // must not kill the whole stream with a raw SyntaxError; it should be
    // skipped so onDone still fires with the text from the good lines.
    const lines = [
      '{"message":{"content":"a"},"done":false}\n',
      "not valid json\n",
      '{"message":{"content":"b"},"done":true}\n',
    ];
    stubFetch(new Response(streamOf(lines), { status: 200 }));

    const onDelta = vi.fn();
    const onDone = vi.fn();

    await streamChat({
      baseUrl: "http://localhost:11434",
      model: "tinyllama",
      prompt: "p",
      onDelta,
      onDone,
    });

    expect(onDone).toHaveBeenCalledTimes(1);
    expect(onDone).toHaveBeenCalledWith({ thinking: "", content: "ab" });
  });

  it("cancels the reader before releasing the lock once the stream ends", async () => {
    const lines = ['{"message":{"content":"a"},"done":true}\n'];
    stubFetch(new Response(streamOf(lines), { status: 200 }));

    const cancelSpy = vi.spyOn(ReadableStreamDefaultReader.prototype, "cancel");

    await streamChat({
      baseUrl: "http://localhost:11434",
      model: "tinyllama",
      prompt: "p",
      onDelta: vi.fn(),
      onDone: vi.fn(),
    });

    expect(cancelSpy).toHaveBeenCalledTimes(1);
    cancelSpy.mockRestore();
  });

  it("does not call onDone when the stream is aborted", async () => {
    const encoder = new TextEncoder();
    const body = new ReadableStream<Uint8Array>({
      start(controller) {
        controller.enqueue(encoder.encode('{"message":{"content":"a"},"done":false}\n'));
      },
      // First read drains the queued line; the refill pull then errors the
      // stream the way an aborted fetch surfaces mid-body.
      pull(controller) {
        controller.error(new DOMException("aborted", "AbortError"));
      },
    });
    stubFetch(new Response(body, { status: 200 }));

    const onDelta = vi.fn();
    const onDone = vi.fn();

    await expect(
      streamChat({
        baseUrl: "http://localhost:11434",
        model: "tinyllama",
        prompt: "p",
        onDelta,
        onDone,
      }),
    ).rejects.toMatchObject({ name: "AbortError" });

    expect(onDelta).toHaveBeenCalledWith({ thinking: "", content: "a" });
    expect(onDone).not.toHaveBeenCalled();
  });

  it("times out a hung initial response with OllamaTimeoutError", async () => {
    vi.useFakeTimers();
    try {
      vi.stubGlobal(
        "fetch",
        vi.fn(
          (_url: string, init: RequestInit) =>
            new Promise<Response>((_resolve, reject) => {
              init.signal?.addEventListener("abort", () =>
                reject(new DOMException("aborted", "AbortError")),
              );
            }),
        ),
      );
      const onDone = vi.fn();
      const promise = streamChat({
        baseUrl: "http://localhost:11434",
        model: "tinyllama",
        prompt: "p",
        onDelta: vi.fn(),
        onDone,
        timeoutMs: 10_000,
      });
      const assertion = expect(promise).rejects.toBeInstanceOf(OllamaTimeoutError);
      await vi.advanceTimersByTimeAsync(10_000);
      await assertion;
      expect(onDone).not.toHaveBeenCalled();
    } finally {
      vi.useRealTimers();
    }
  });

  it("lets a caller abort win over the timeout (stays an AbortError)", async () => {
    const caller = new AbortController();
    vi.stubGlobal(
      "fetch",
      vi.fn(
        (_url: string, init: RequestInit) =>
          new Promise<Response>((_resolve, reject) => {
            init.signal?.addEventListener("abort", () =>
              reject(new DOMException("aborted", "AbortError")),
            );
          }),
      ),
    );
    const promise = streamChat({
      baseUrl: "http://localhost:11434",
      model: "tinyllama",
      prompt: "p",
      onDelta: vi.fn(),
      onDone: vi.fn(),
      signal: caller.signal,
      timeoutMs: 10_000,
    });
    caller.abort();
    await expect(promise).rejects.toMatchObject({ name: "AbortError" });
  });

  it("throws OllamaHttpError with the status on a non-2xx response", async () => {
    stubFetch(new Response("nope", { status: 404 }));

    await expect(
      streamChat({
        baseUrl: "http://localhost:11434",
        model: "missing",
        prompt: "p",
        onDelta: vi.fn(),
        onDone: vi.fn(),
      }),
    ).rejects.toMatchObject(expect.objectContaining({ status: 404 }));
  });
});

describe("toTraceInput", () => {
  it("uses accumulated thinking as thinking and content as answer when thinking arrived", () => {
    const result = toTraceInput("2+2?", { thinking: "reasoning...", content: "4" });
    expect(result).toEqual({ problem: "2+2?", thinking: "reasoning...", answer: "4" });
  });

  it("falls back to content as thinking with an empty answer when no thinking ever arrived", () => {
    const result = toTraceInput("2+2?", { thinking: "", content: "It's 4" });
    expect(result).toEqual({ problem: "2+2?", thinking: "It's 4", answer: "" });
  });
});

describe("throttle", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("invokes on the leading edge, coalesces rapid calls, and fires once more on the trailing edge", () => {
    const fn = vi.fn();
    const throttled = throttle(fn, 500);

    throttled("a");
    expect(fn).toHaveBeenCalledTimes(1);
    expect(fn).toHaveBeenLastCalledWith("a");

    throttled("b");
    throttled("c");
    throttled("d");
    expect(fn).toHaveBeenCalledTimes(1);

    vi.advanceTimersByTime(500);
    expect(fn).toHaveBeenCalledTimes(2);
    expect(fn).toHaveBeenLastCalledWith("d");

    vi.advanceTimersByTime(1000);
    expect(fn).toHaveBeenCalledTimes(2);
  });

  it("fires a fresh leading call after a gap longer than the window", () => {
    const fn = vi.fn();
    const throttled = throttle(fn, 500);

    throttled("a");
    vi.advanceTimersByTime(1000);
    throttled("b");

    expect(fn).toHaveBeenCalledTimes(2);
    expect(fn).toHaveBeenNthCalledWith(1, "a");
    expect(fn).toHaveBeenNthCalledWith(2, "b");
  });

  it("cancel() drops a pending trailing call", () => {
    const fn = vi.fn();
    const throttled = throttle(fn, 500);

    throttled("a");
    throttled("b");
    throttled.cancel();

    vi.advanceTimersByTime(500);
    expect(fn).toHaveBeenCalledTimes(1);
  });
});
