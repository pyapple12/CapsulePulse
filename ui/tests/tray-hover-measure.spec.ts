/**
 * 悬浮卡"内容没变就不量"（FIX010.12）：后端每秒推 `hover-update`，但卡片文本是**分钟级**
 * （行①/② 走 `splitDuration` 取分钟、行③ `toFixed(1)` 为 6 秒粒度）⇒ 同一内容重复推送时
 * **不该再量几何**（`nextTick` + `getBoundingClientRect` + `getComputedStyle`）。
 *
 * 断言面 = 测量调用次数（spy `Element.prototype.getBoundingClientRect`）。
 * 夹具给了固定 chrome（仿 `measure-window.spec.ts`），使量出的尺寸为真实数字。
 */
import { flushPromises, mount } from "@vue/test-utils";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import TrayHover from "../src/components/TrayHover.vue";

/** 捕获 `listen("hover-update")` 的回调，供用例主动推送 */
const bus = vi.hoisted(() => ({
  handler: null as ((event: { payload: unknown }) => void) | null,
}));

const base = {
  state: "on_duty",
  label: "本班",
  remaining_secs: 7_430,
  work_secs: 3_600,
  rest_secs: 600,
  until_reminder_min: 12.3,
  single_line: false,
};

vi.mock("@tauri-apps/api/core", () => ({
  invoke: async (cmd: string): Promise<unknown> => {
    if (cmd === "tray_hover") return structuredClone(base);
    return null;
  },
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: async (_name: string, cb: (event: { payload: unknown }) => void) => {
    bus.handler = cb;
    return () => {};
  },
}));

/** 固定 chrome（与 measure-window.spec.ts 同款），让 measured 为真实数字 */
function stubChrome(): void {
  vi.stubGlobal(
    "getComputedStyle",
    () =>
      ({
        paddingLeft: "8px",
        paddingRight: "8px",
        borderLeftWidth: "1px",
        borderRightWidth: "1px",
        marginLeft: "0px",
        marginRight: "0px",
        paddingTop: "6px",
        paddingBottom: "6px",
        borderTopWidth: "1px",
        borderBottomWidth: "1px",
        marginTop: "2px",
        marginBottom: "2px",
      }) as unknown as CSSStyleDeclaration,
  );
}

const settle = (): Promise<void> => new Promise((resolve) => setTimeout(resolve, 0));

async function flushAll(): Promise<void> {
  await flushPromises();
  await settle();
}

/** 推一次 `hover-update`（模拟后端每秒推送） */
async function push(patch: Record<string, unknown>): Promise<void> {
  bus.handler?.({ payload: { ...base, ...patch } });
  await flushAll();
}

describe("悬浮卡几何测量的去重（FIX010.12）", () => {
  let wrapper: ReturnType<typeof mount> | undefined;
  let measureCalls: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    stubChrome();
    measureCalls = vi
      .spyOn(Element.prototype, "getBoundingClientRect")
      .mockReturnValue({ width: 60, height: 20 } as DOMRect);
  });

  afterEach(() => {
    wrapper?.unmount();
    wrapper = undefined;
    bus.handler = null;
    measureCalls.mockRestore();
    vi.unstubAllGlobals();
  });

  it("同一内容重复推送不量；内容真变才量", async () => {
    wrapper = mount(TrayHover, { attachTo: document.body });
    await flushAll();

    // ① 首帧（挂载取数）已量过至少一次
    expect(measureCalls.mock.calls.length, "首帧应量一次").toBeGreaterThan(0);

    // ② 同一分钟内的推送（剩余秒变了、渲染文本没变）⇒ 不应再量
    const before = measureCalls.mock.calls.length;
    await push({ remaining_secs: 7_420 });
    expect(measureCalls.mock.calls.length, "内容未变时不得再量几何").toBe(before);

    // ③ 渲染文本真变（跨一分钟）⇒ 必须再量
    await push({ remaining_secs: 7_320 });
    expect(measureCalls.mock.calls.length, "内容真变必须重测").toBeGreaterThan(before);

    // ④ 单行/多行切换（行数变）⇒ 必须再量
    const beforeSingle = measureCalls.mock.calls.length;
    await push({ single_line: true });
    expect(measureCalls.mock.calls.length, "行数变必须重测").toBeGreaterThan(beforeSingle);
  });
});
