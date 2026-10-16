/**
 * 段序列构建（FIX006.4）：把 `DaySummary.blocks`（只覆盖在岗窗口）补上班间空档，产出毫秒段序列。
 * 单一实现——表盘弧段 / 电池（TimerCard）与时间图谱（StatsView）共用，杜绝两处各推一份漂移。
 */

import type { DayBlock } from "../types";

/** 段序列元素：`work` / `rest` 为真实块，`gap` 为班间空档（后端 `blocks` 不含空档）。 */
export interface DaySegment {
  t0: number;
  t1: number;
  kind: "work" | "rest" | "gap";
}

/**
 * 由当日区块构建段序列（毫秒）。
 * @param blocks 后端归约出的区块（秒，只覆盖在岗窗口，按时间序）
 * @param tailToMs 给定时把末段右端延伸到该毫秒（在岗中的"活尾巴"）；不传则原样
 */
export function daySegments(blocks: DayBlock[], tailToMs?: number): DaySegment[] {
  const segs: DaySegment[] = [];
  let lastEnd: number | null = null;
  for (const b of blocks) {
    const t0 = b.start * 1000;
    const t1 = b.end * 1000;
    if (lastEnd != null && t0 > lastEnd) {
      segs.push({ t0: lastEnd, t1: t0, kind: "gap" });
    }
    segs.push({ t0, t1, kind: b.kind });
    lastEnd = t1;
  }
  const tail = segs[segs.length - 1];
  if (tail != null && tailToMs != null && tailToMs > tail.t1) {
    tail.t1 = tailToMs;
  }
  return segs;
}
