/**
 * 前端测试环境补齐（jsdom 缺项）：**只补测试跑得起来所需的最小桩**，不改产品行为、
 * 不替产品兜底（产品侧的降级白名单仍以 AGENTS 为准）。
 *
 * 现状缺两项：`ResizeObserver`（`ui/src/use-glass-bar.ts` 用它跟随滚动面尺寸变化）、
 * `window.matchMedia`（`StatsView` 用它查"减少动效"偏好）——jsdom 都不实现，
 * 不补会在 mount / setup 期直接抛错。
 */

/** 空实现桩：测试只关心"渲染是否发生"，不关心浮钮几何（jsdom 无布局、矩形恒为 0） */
class ResizeObserverStub {
  observe(): void {}
  unobserve(): void {}
  disconnect(): void {}
}

if (window.ResizeObserver == null) {
  window.ResizeObserver = ResizeObserverStub as unknown as typeof ResizeObserver;
}

/** 动效偏好恒返回"不减少动效"（= 本机与产品默认路径；减动效分支不在本用例覆盖范围） */
if (window.matchMedia == null) {
  window.matchMedia = ((query: string) => ({
    matches: false,
    media: query,
    onchange: null,
    addListener: () => {},
    removeListener: () => {},
    addEventListener: () => {},
    removeEventListener: () => {},
    dispatchEvent: () => false,
  })) as unknown as typeof window.matchMedia;
}

/** `document.timeline`（图谱彗星动画的走时基准；`StatsView` 读 `currentTime`）：
 * jsdom 不实现（真实三端 WebView 都有）⇒ 用 `performance.now()` 顶上，语义与 DocumentTimeline 一致 */
if (document.timeline == null) {
  Object.defineProperty(document, "timeline", {
    configurable: true,
    value: {
      get currentTime(): number {
        return performance.now();
      },
    },
  });
}
