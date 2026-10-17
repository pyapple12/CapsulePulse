/**
 * 图谱节点层的内容签名去重（FIX010.8）：**内容没变就整段跳过**（一个 DOM 都不碰），
 * 只有真变了才重建（design：避免复用跨色渐变伪影）。
 *
 * 断言面 = `#graphNodes` 的 MutationObserver 记录数：零条 = 跳过、有记录 = 重建。
 *
 * 构造要点（不这样构造就测不出东西）：夹具的打卡/块时间必须**相对 `Date.now()`**，
 * 否则 `打卡 + 剩余` 落在过去 ⇒ 轴末退化到"此刻"（`endTs = max(..., nowMs)`）⇒ 每次刷新的
 * 节点签名都不同、永远走重建分支（这正是"旧实现看不出区别"的原因：它本来每次都重建）。
 */
import { flushPromises, mount } from "@vue/test-utils";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import StatsView from "../components/StatsView.vue";

/** 可写夹具（测试中途改内容，验证"真变了才重建"） */
const state = vi.hoisted(() => {
  const nowSec = Math.floor(Date.now() / 1000);
  const onDuty = {
    duty_started_at: nowSec - 3_600,
    duty_ended_at: null as number | null,
    duty_secs: 3_600,
    work_secs: 2_400,
    rest_secs: 600,
    blocks: [
      { start: nowSec - 3_600, end: nowSec - 1_200, kind: "work" },
      { start: nowSec - 1_200, end: nowSec - 600, kind: "rest" },
    ],
    budget_hours: 8,
    used_secs: 3_600,
    // 剩余远大于 0 ⇒ 轴末 = 打卡 + 剩余（未来时刻、跨刷新稳定），不退化到"此刻"
    remaining_secs: 25_200,
    shift_start: nowSec - 3_600,
    can_overtime: true,
    cap_reached: false,
  };
  const week = Array.from({ length: 7 }, (_, i) => ({
    date: `01-0${i + 1}`,
    weekday: i + 1,
    work_secs: i * 600,
    duty_secs: i * 900,
  }));
  return { day: structuredClone(onDuty), onDuty: structuredClone(onDuty), week };
});

vi.mock("@tauri-apps/api/core", () => ({
  invoke: async (cmd: string): Promise<unknown> => {
    if (cmd === "day_detail") return structuredClone(state.day);
    if (cmd === "week_detail") return structuredClone(state.week);
    if (cmd === "workday_total") {
      return { duty_secs: 3_600, work_secs: 3_000, rest_secs: 600, days: 1, first_offset: -1 };
    }
    return null;
  },
}));

/** 观察节点层变更；容器缺失直接抛错（模板改了就该让本用例红） */
function watchNodes(): { records: MutationRecord[]; reset: () => void; stop: () => void } {
  const host = document.querySelector("#graphNodes");
  if (host == null) {
    throw new Error("渲染容器缺失：#graphNodes");
  }
  const records: MutationRecord[] = [];
  const observer = new MutationObserver((batch) => records.push(...batch));
  observer.observe(host, { childList: true, subtree: true, characterData: true, attributes: true });
  return {
    records,
    reset: () => {
      records.length = 0;
    },
    stop: () => observer.disconnect(),
  };
}

const settle = (): Promise<void> => new Promise((resolve) => setTimeout(resolve, 0));

/** 等微任务 + 渲染落地 */
async function flushAll(): Promise<void> {
  await flushPromises();
  await settle();
  await flushPromises();
  await settle();
}

/** 画布宽桩：jsdom 无布局（`clientWidth` 恒 0），而节点横坐标 = `f(时刻, W)` ⇒ 需可控 W */
function setCanvasWidth(w: number): void {
  Object.defineProperty(SVGElement.prototype, "clientWidth", {
    configurable: true,
    get: () => w,
  });
}

describe("图谱节点层签名去重（FIX010.8）", () => {
  let wrapper: ReturnType<typeof mount> | undefined;

  beforeEach(() => {
    state.day = structuredClone(state.onDuty);
    setCanvasWidth(320);
  });

  afterEach(() => {
    wrapper?.unmount();
    wrapper = undefined;
    delete (SVGElement.prototype as { clientWidth?: number }).clientWidth;
    vi.unstubAllGlobals();
  });

  it("内容没变 ⇒ 跳过；内容变了 / 画布宽变了 ⇒ 重建", async () => {
    wrapper = mount(StatsView, {
      attachTo: document.body,
      props: { refreshKey: 0, dataDayOffset: 0, open: true },
    });
    await flushAll();

    const nodes = watchNodes();

    // ① 首刷已有节点（夹具保证至少 W/R/D 三点）
    expect(document.querySelectorAll("#graphNodes g").length).toBeGreaterThan(0);

    // ② 再刷一次：数据是**新对象但内容相同**（真实后端亦然）⇒ 签名不变 ⇒ 零变更
    nodes.reset();
    await wrapper.setProps({ refreshKey: 1 });
    await flushAll();
    expect(nodes.records, "内容未变时不得重建节点层").toHaveLength(0);

    // ③ 内容真变（多一段工作 ⇒ 多一个节点）⇒ 必须重建
    const nowSec = state.day.shift_start + 3_600;
    state.day = {
      ...state.day,
      blocks: [
        { start: nowSec - 3_600, end: nowSec - 1_800, kind: "work" },
        { start: nowSec - 1_800, end: nowSec - 1_200, kind: "rest" },
        { start: nowSec - 1_200, end: nowSec - 600, kind: "work" },
      ],
    };
    nodes.reset();
    await wrapper.setProps({ refreshKey: 2 });
    await flushAll();
    expect(nodes.records.length, "内容真变时必须重建").toBeGreaterThan(0);

    // ④ 只有画布宽变（内容相同）⇒ 横坐标缩放变了 ⇒ 也必须重建
    nodes.reset();
    setCanvasWidth(400);
    await wrapper.setProps({ refreshKey: 3 });
    await flushAll();
    expect(nodes.records.length, "画布宽变了必须重建（宽度进签名）").toBeGreaterThan(0);

    // ⑤ 已下班（D 点落到末块末尾）⇒ 内容变 ⇒ 重建
    nodes.reset();
    state.day = { ...state.day, duty_ended_at: state.day.shift_start + 3_600, remaining_secs: 0 };
    await wrapper.setProps({ refreshKey: 4 });
    await flushAll();
    expect(nodes.records.length, "下班后 D 点位置变，必须重建").toBeGreaterThan(0);

    nodes.stop();
  });
});
