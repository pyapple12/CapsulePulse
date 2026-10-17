/**
 * 统计板渲染门控（FIX007.9）：**板关时数据变化不得重建**图谱 / 周卡 / 明细；板开时必须重建。
 *
 * 断言面 = 原始 DOM 渲染容器的 MutationObserver 记录数（关板 0 条 / 开板 >0 条）。
 * 刻意**不观察 `document.body`**：玻璃浮钮自身带 MutationObserver→sync 回写，
 * 观察全局会把它算进来、负向断言必然假红。几何一律不断言（jsdom 无布局、矩形恒 0）。
 */
import { flushPromises, mount } from "@vue/test-utils";
import { afterEach, describe, expect, it, vi } from "vitest";

import StatsView from "../components/StatsView.vue";

// 夹具须在 mock 工厂之前就绪（工厂会被提升）⇒ 用 vi.hoisted
const fixture = vi.hoisted(() => {
  const day = {
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
  const week = Array.from({ length: 7 }, (_, i) => ({
    date: `01-0${i + 1}`,
    weekday: i + 1,
    work_secs: i * 600,
    duty_secs: i * 900,
  }));
  return { day, week };
});

/** 后端命令替身：每次调用返回**新对象**（真实后端亦然）——与后端同款的"数据变了"语义 */
vi.mock("@tauri-apps/api/core", () => ({
  invoke: async (cmd: string): Promise<unknown> => {
    if (cmd === "day_detail") return structuredClone(fixture.day);
    if (cmd === "week_detail") return structuredClone(fixture.week);
    if (cmd === "workday_total") {
      return { duty_secs: 3_600, work_secs: 3_000, rest_secs: 600, days: 1, first_offset: null };
    }
    return null;
  },
}));

/** 三块渲染的原始 DOM 落点（`renderDetail` 走响应式 `detailRows`，与它们同批重建） */
const RENDER_HOSTS = ["#graphBars", "#graphNodes", "#wcBars"];

/** 观察渲染容器的变更记录；容器缺失直接抛错（模板改了就该让本用例红） */
function watchRenderHosts(): { records: MutationRecord[]; stop: () => void } {
  const records: MutationRecord[] = [];
  const observer = new MutationObserver((batch) => records.push(...batch));
  for (const sel of RENDER_HOSTS) {
    const host = document.querySelector(sel);
    if (host == null) {
      throw new Error(`渲染容器缺失：${sel}`);
    }
    observer.observe(host, { childList: true, subtree: true, characterData: true });
  }
  return { records, stop: () => observer.disconnect() };
}

/** 等 MutationObserver 的微任务送达（jsdom 按微任务派发记录） */
const settle = (): Promise<void> => new Promise((resolve) => setTimeout(resolve, 0));

/** 挂载统计板并等首刷落地（mount 期 `onMounted` 自己会取一次数） */
async function mountBoard(open: boolean) {
  const wrapper = mount(StatsView, {
    attachTo: document.body,
    props: { refreshKey: 0, dataDayOffset: 0, open },
  });
  await flushPromises();
  await settle();
  return wrapper;
}

describe("统计板渲染门控（FIX007.9）", () => {
  let wrapper: ReturnType<typeof mount> | undefined;
  afterEach(() => {
    wrapper?.unmount();
    wrapper = undefined;
  });

  it("板关：数据变化（动作刷新）不重建", async () => {
    wrapper = await mountBoard(false);
    const watched = watchRenderHosts();
    // 动作路径：App 侧 refreshKey++ ⇒ 本组件重取数据 ⇒ day / week 换新对象
    await wrapper.setProps({ refreshKey: 1 });
    await flushPromises();
    await settle();
    watched.stop();
    expect(watched.records).toHaveLength(0);
  });

  it("板开：数据变化即时重建（门不得把功能关死）", async () => {
    wrapper = await mountBoard(true);
    const watched = watchRenderHosts();
    await wrapper.setProps({ refreshKey: 1 });
    await flushPromises();
    await settle();
    watched.stop();
    expect(watched.records.length).toBeGreaterThan(0);
  });
});
