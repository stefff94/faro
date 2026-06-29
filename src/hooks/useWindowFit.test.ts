import { describe, it, expect } from "vitest";
import { computeFit, FIT_MARGIN } from "./useWindowFit";

describe("computeFit", () => {
  it("adds left margin to width and top+bottom margins to height", () => {
    const f = computeFit(100, 50);
    expect(f.winW).toBe(100 + FIT_MARGIN.left);
    expect(f.winH).toBe(50 + FIT_MARGIN.top + FIT_MARGIN.bottom);
  });

  it("places content offset at the reserved left/top margins", () => {
    const f = computeFit(100, 50);
    expect(f.contentX).toBe(FIT_MARGIN.left);
    expect(f.contentY).toBe(FIT_MARGIN.top);
    expect(f.contentW).toBe(100);
    expect(f.contentH).toBe(50);
  });
});
