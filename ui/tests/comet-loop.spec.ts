/**
 * 彗星两条自维持循环的启停纪律（FIX010.1 / FIX010.13）：**同一时刻至多一条 rAF 链**，
 * 且"图上不存在彗星走廊"（未在岗 / 过去日 / 空日）时循环必须停机。
 *
 * 手法：`requestAnimationFrame` / `cancelAnimationFrame` 打桩成**可手动推进的登记表**（jsdom 虽实现
 * rAF 但不可控、也不反映"排了几条链"），断言面 = 登记表里"下一帧要跑的回调"个数；本组件只有彗星用
 * rAF（全文件仅三处调用）⇒ 该数即存活链数。
 *
 * 复现路径（FIX009.2 引入的回归）：在岗今天（1 条链）→ 翻到过去日（图隐藏，旧实现不停循环）→
 * 翻回今天（旧实现按 `wasHidden` 又起一条 ⇒ 两条并存且旧链 id 被覆盖，无法取消）。
 */
import { flushPromises, mount } from "@vue/test-utils";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import StatsView from "../components/StatsView.vue";

/** 夹具须在 mock 工厂之前就绪（工厂会被提升）⇒ 用 vi.hoisted */
const fixture = vi.hoisted(() => {
  /** 在岗的今天（引爆彗星走廊的必要条件：今日 + 未下班） */
  const onDuty = {
    duty_started_at: 1_700_000_000,
    duty_ended_at: null,
    duty_secs: 3_600,
    work_secs: 3_000,
    rest_secs: 600,
    blocks: [
      { start: 1_700_000_000, end: 1_700_002_400, kind: "work" },
      { start: 1_700_002_400, end: 1_700_003_600, kind: "rest" },
    ],
    budget_hours: 8,
    used_secs: 3_600,
    remaining_secs: 25_200,
    shift_start: 1_700_000_000,
    can_overtime: true,
    cap_reached: false,
  };
  /** 已下班的过去日（图仍在、只是 `onDutyToday` 为假 ⇒ 走 `display:none` 分支） */
  const ended = { ...onDuty, duty_ended_at: 1_700_003_600 };
  const week = Array.from({ length: 7 }, (_, i) => ({
    date: `01-0${i + 1}`,
    weekday: i + 1,
    work_secs: i * 600,
    duty_secs: i * 900,
  }));
  return { onDuty, ended, week };
});

/** 后端命令替身：`day_detail` 按 offset 分派——0 = 在岗今天，其余 = 已下班日 */
vi.mock("@tauri-apps/api/core", () => ({
  invoke: async (cmd: string, args?: { offset?: number }): Promise<unknown> => {
    if (cmd === "day_detail") {
      return structuredClone(args?.offset === 0 ? fixture.onDuty : fixture.ended);
    }
    if (cmd === "week_detail") return structuredClone(fixture.week);
    if (cmd === "workday_total") {
      // first_offset = −1 ⇒ 允许往前翻一天（翻页下界不放行则复现路径不成立）
      return { duty_secs: 3_600, work_secs: 3_000, rest_secs: 600, days: 1, first_offset: -1 };
    }
    return null;
  },
}));

/** rAF 登记表（存活回调 = 已排程、尚未跑的帧回调） */
const raf = vi.hoisted(() => ({
  live: new Map<number, FrameRequestCallback>(),
  seq: 0,
}));

beforeEach(() => {
  // jsdom 无布局 ⇒ `clientWidth` 恒 0，而彗星走廊的准入门槛是 `cometWalk > 24`
  // （`cometWalk = W − 出生点 + 24`，W = `svg.clientWidth`）⇒ 不打桩则整段彗星逻辑被跳过。
  // 给 SVG 元素一个固定画布宽（仅本文件生效；真机观感仍走 live 目测）。
  Object.defineProperty(SVGElement.prototype, "clientWidth", {
    configurable: true,
    get: () => 320,
  });
  raf.live.clear();
  raf.seq = 0;
  vi.stubGlobal("requestAnimationFrame", (cb: FrameRequestCallback): number => {
    raf.seq += 1;
    raf.live.set(raf.seq, cb);
    return raf.seq;
  });
  vi.stubGlobal("cancelAnimationFrame", (id: number): void => {
    raf.live.delete(id);
  });
});

afterEach(() => {
  delete (SVGElement.prototype as { clientWidth?: number }).clientWidth;
  vi.unstubAllGlobals();
});

/** 手动推进 n 帧：把当前存活回调取出执行（回调内部各自重排 ⇒ 复用登记表） */
function tickFrames(n: number): void {
  for (let i = 0; i < n; i += 1) {
    const callbacks = [...raf.live.values()];
    raf.live.clear();
    for (const cb of callbacks) {
      cb(performance.now());
    }
  }
}

/** 等 MutationObserver / 组件内部微任务送达 */
const settle = (): Promise<void> => new Promise((resolve) => setTimeout(resolve, 0));

/** 把渲染与取数推到稳定态 */
async function flushAll(): Promise<void> {
  await flushPromises();
  await settle();
  await flushPromises();
  await settle();
}

describe("彗星循环启停纪律（FIX010.1）", () => {
  let wrapper: ReturnType<typeof mount> | undefined;

  afterEach(() => {
    wrapper?.unmount();
    wrapper = undefined;
  });

  it("在岗今天只起一条链；翻走过往日即停机；翻回今天仍只一条", async () => {
    wrapper = mount(StatsView, {
      attachTo: document.body,
      props: { refreshKey: 0, dataDayOffset: 0, open: true },
    });
    await flushAll();

    // ① 在岗今天：起一条链（不管推进多少帧，链数恒 1）
    tickFrames(3);
    expect(raf.live.size, "在岗时应有且仅有一条 rAF 链").toBe(1);

    // ② 翻到过去日：图上没有彗星走廊 ⇒ 循环必须停机（旧实现继续 60fps 空转）
    await wrapper.find('[title="前一日"]').trigger("click");
    await flushAll();
    tickFrames(2);
    expect(raf.live.size, "过去日应停机，不留存活链").toBe(0);

    // ③ 翻回今天：重启**一条**链，绝不允许与旧链叠加（旧实现此处变 2 条）
    await wrapper.find('[title="后一日"]').trigger("click");
    await flushAll();
    tickFrames(2);
    expect(raf.live.size, "回到在岗只允许一条链").toBe(1);

    // ④ 卸载后不留存活链（`onUnmounted` 回收生效）
    wrapper.unmount();
    wrapper = undefined;
    tickFrames(1);
    expect(raf.live.size, "卸载后不应有存活链").toBe(0);
  });
});
