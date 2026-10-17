/**
 * 子窗口量内容管线（FIX007.12）：校验 `snap4` 的取整边界与 `measureChrome` 的 chrome 求和 / 缺失分支。
 *
 * jsdom 无布局 ⇒ 元素尺寸与计算样式用桩喂入：本用例测的是**数学与口径**（窗 = 内容 + chrome，
 * 再过 `snap4`），不是真实渲染；真机观感仍走 live 目测。
 */
import { afterEach, describe, expect, it, vi } from "vitest";

import { measureChrome, snap4 } from "../src/measure-window";

describe("snap4：向上取到 4 的倍数（只多不少 ⇒ 内容永不裁）", () => {
  it("边界与整定实例值", () => {
    expect(snap4(0)).toBe(0);
    expect(snap4(1)).toBe(4);
    expect(snap4(4)).toBe(4);
    expect(snap4(4.1)).toBe(8);
    expect(snap4(34)).toBe(36); // PATCH007.24 实例：单行 34 在 125% 下为 42.5 物理 ⇒ 取到 36（= 45 物理）
    expect(snap4(228)).toBe(228); // 多行固定宽 4×57，本就在格点上
  });
});

describe("measureChrome：内容层尺寸 + 卡片 chrome（从 CSS 读，不手抄常量）", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("任一元素缺失 ⇒ null（调用方据此跳过本轮，不设窗不重锚）", () => {
    expect(measureChrome(null, document.createElement("div"))).toBeNull();
    expect(measureChrome(document.createElement("div"), null)).toBeNull();
  });

  it("chrome = 左右 / 上下各自「内边距 + 边框 + 外边距」之和", () => {
    const body = document.createElement("div");
    const card = document.createElement("div");
    vi.spyOn(body, "getBoundingClientRect").mockReturnValue({
      width: 100,
      height: 30,
    } as DOMRect);
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

    const measured = measureChrome(body, card);
    expect(measured).not.toBeNull();
    expect(measured?.contentW).toBe(100);
    expect(measured?.contentH).toBe(30);
    expect(measured?.chromeX).toBe(18); // 8 + 8 + 1 + 1 + 0 + 0
    expect(measured?.chromeY).toBe(18); // 6 + 6 + 1 + 1 + 2 + 2
    // 与两窗原口径一致：窗 = 内容 + chrome，再向上取 4 的倍数
    expect(snap4((measured?.contentW ?? 0) + (measured?.chromeX ?? 0))).toBe(120);
    expect(snap4((measured?.contentH ?? 0) + (measured?.chromeY ?? 0))).toBe(48);
  });
});
