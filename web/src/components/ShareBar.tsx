// Share toolbar: PNG export ("export png") and client-side share links
// ("copy share link"), rendered above the anatomy view. Pure encode/decode/
// filename logic lives in ../lib/share.ts; this component only builds the UI
// and drives clipboard/download/hash side effects.
//
// Mounted once a result exists and then kept mounted (not remounted) across
// every later analysis in the session — including every throttled live
// stream tick — so `status` (e.g. "copied") is genuinely component state,
// not rebuilt from scratch on each render the way the old DOM version
// rebuilt its toolbar on every call.

import { useState, type RefObject } from "react";
import { toPng } from "html-to-image";
import { encodeShareFragment, exportFilename } from "../lib/share";
import type { AnalysisResult, TraceInput } from "../lib/types";

/** Thin wrapper around html-to-image's toPng so tests can stub the actual
 * rasterization — happy-dom/vitest cannot perform real canvas rendering. */
export function capturePng(node: HTMLElement): Promise<string> {
  return toPng(node);
}

type Capture = (node: HTMLElement) => Promise<string>;

interface ShareBarProps {
  /** The anatomy-view container (report card = header + legend + annotated
   * text + score card) that "export png" rasterizes. */
  captureTargetRef: RefObject<HTMLElement | null>;
  trace: TraceInput;
  result: AnalysisResult;
  /** Defaults to the real html-to-image call; overridden only in tests. */
  capture?: Capture;
}

export function ShareBar({ captureTargetRef, trace, result, capture = capturePng }: ShareBarProps) {
  const [status, setStatus] = useState("");

  function triggerDownload(href: string, filename: string): void {
    const link = document.createElement("a");
    link.href = href;
    link.download = filename;
    document.body.append(link);
    link.click();
    link.remove();
  }

  async function exportPng(): Promise<void> {
    const node = captureTargetRef.current;
    if (!node) return;

    const footer = document.createElement("div");
    footer.className = "share-export-footer";
    footer.textContent = location.origin + location.pathname;
    node.append(footer);
    try {
      const dataUrl = await capture(node);
      triggerDownload(dataUrl, exportFilename({ id: trace.id }, result.composite));
      setStatus("png exported");
    } catch (err) {
      console.warn("reasonmetrics: png export failed", err);
      setStatus("could not export png");
    } finally {
      footer.remove();
    }
  }

  async function copyShareLink(): Promise<void> {
    const encoded = encodeShareFragment(trace);
    if ("tooLarge" in encoded) {
      const kb = Math.round(encoded.bytes / 1024);
      setStatus(`trace too large for a link (${kb} KB compressed) — downloading file instead`);
      const jsonName = exportFilename({ id: trace.id }, result.composite).replace(/\.png$/, ".json");
      const blob = new Blob([JSON.stringify(trace, null, 2)], { type: "application/json" });
      const url = URL.createObjectURL(blob);
      triggerDownload(url, jsonName);
      setTimeout(() => URL.revokeObjectURL(url), 0);
      return;
    }

    location.hash = `t=${encoded.fragment}`;
    const href = location.href;

    if (navigator.clipboard?.writeText) {
      try {
        await navigator.clipboard.writeText(href);
        setStatus("copied");
        return;
      } catch {
        // Clipboard write can fail (permissions, insecure context); fall
        // back to showing the URL for manual copy below.
      }
    }
    setStatus(href);
  }

  return (
    <div className="share-bar">
      <button type="button" className="share-export-btn" onClick={() => void exportPng()}>
        export png
      </button>
      <button type="button" className="share-copy-btn" onClick={() => void copyShareLink()}>
        copy share link
      </button>
      <span className="share-status">{status}</span>
    </div>
  );
}
