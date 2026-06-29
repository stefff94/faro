import { useEffect } from "react";
import type React from "react";
import { invoke } from "@tauri-apps/api/core";

// Transparent margins (CSS px) reserved around the painted widget for shadow/glow.
// `top` MUST match `.drawer { top: 30px }` in src/App.css (kept in sync by hand).
export const FIT_MARGIN = { left: 40, top: 30, bottom: 40 } as const;

export function computeFit(
  contentW: number,
  contentH: number,
): {
  winW: number;
  winH: number;
  contentX: number;
  contentY: number;
  contentW: number;
  contentH: number;
} {
  return {
    winW: contentW + FIT_MARGIN.left,
    winH: contentH + FIT_MARGIN.top + FIT_MARGIN.bottom,
    contentX: FIT_MARGIN.left,
    contentY: FIT_MARGIN.top,
    contentW,
    contentH,
  };
}

// Observe the painted widget and keep the OS window sized to hug it. Coalesces
// bursts of resize callbacks into one report per animation frame.
export function useWindowFit(ref: React.RefObject<HTMLElement | null>) {
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    let raf = 0;
    const report = () => {
      const r = el.getBoundingClientRect();
      if (r.width === 0 || r.height === 0) return;
      const fit = computeFit(Math.ceil(r.width), Math.ceil(r.height));
      invoke("resize_to_content", { fit }).catch(() => {
        /* ignore — not in Tauri context (e.g. browser dev) */
      });
    };
    const ro = new ResizeObserver(() => {
      cancelAnimationFrame(raf);
      raf = requestAnimationFrame(report);
    });
    ro.observe(el);
    report();
    return () => {
      ro.disconnect();
      cancelAnimationFrame(raf);
    };
  }, [ref]);
}
