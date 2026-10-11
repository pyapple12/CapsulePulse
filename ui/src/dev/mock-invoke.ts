/**
 * DEV 冒烟基座（CT 模式移植）：浏览器直接打开 ui/（无 Tauri 外壳）时拦截 IPC 全家，
 * 让映射期的对比探针能在 IAB 里驱动真实组件树。生产构建经 main.ts 的
 * import.meta.env.DEV 死分支静态消除，dist 零痕迹。
 * 种子数据对齐 design 实验场演示态（在岗计时中 / 阈值 50 / 自动下班 8h / 有统计）。
 */

/** MM-DD 标签（周卡日期列，与后端 WeekDay.date 同格式） */
function mmdd(d: Date): string {
  return `${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

/** 命令路由表：cmd → 返回值（同步值直接返回，异步语义由 invoke 包装） */
function routeCommand(cmd: string, args?: Record<string, unknown>): unknown {
  // 今日起点 09:00（本地时区），运行中累计到当前时刻
  const now = Date.now();
  const todayStart = new Date();
  todayStart.setHours(9, 0, 0, 0);
  const elapsedMs = Math.max(0, now - todayStart.getTime());
  const todayMidnight = new Date();
  todayMidnight.setHours(0, 0, 0, 0);

  switch (cmd) {
    case "session_status":
      return { state: "running", total_ms: elapsedMs, on_duty: true };
    case "session_stats":
      return { today_secs: 12_600, week_secs: 54_300, all_secs: 93_600 };
    case "day_detail":
      // 单位对齐真实后端：时间戳一律秒（SessionStatus.total_ms 例外为毫秒）
      return {
        duty_started_at: Math.floor(todayStart.getTime() / 1000),
        duty_ended_at: null,
        duty_secs: Math.floor(elapsedMs / 1000),
        work_secs: Math.floor(elapsedMs / 1000),
        rest_secs: 0,
        blocks: [
          {
            start: Math.floor(todayStart.getTime() / 1000),
            end: Math.floor(now / 1000),
            kind: "work",
          },
        ],
      };
    case "week_detail": {
      // 自然周（PL022）：查看日所在周固定周一~周日 7 行，"今"随真实星期、未来日零值
      const offset = Number(args?.offset ?? 0);
      const view = new Date(todayMidnight);
      view.setDate(view.getDate() + offset);
      const monday = new Date(view);
      monday.setDate(monday.getDate() - ((monday.getDay() + 6) % 7));
      const todayKey = mmdd(todayMidnight);
      return Array.from({ length: 7 }, (_, i) => {
        const d = new Date(monday);
        d.setDate(d.getDate() + i);
        const key = mmdd(d);
        const isFuture = d.getTime() > todayMidnight.getTime();
        const isToday = key === todayKey;
        const work = isFuture ? 0 : isToday ? Math.floor(elapsedMs / 1000) : 7_200 + i * 1_200;
        return {
          date: key,
          weekday: i + 1,
          work_secs: work,
          duty_secs: isFuture ? 0 : isToday ? work : work + 1_800,
        };
      });
    }
    case "workday_total":
      // 全历史三值总和 + 有数据天数（PL024.6：前端据此算各值平均与偏离）
      return { duty_secs: 115_200, work_secs: 93_600, rest_secs: 21_600, days: 12 };
    case "get_settings":
      return {
        threshold_min: 50,
        sound_enabled: true,
        notify_enabled: true,
        workday_auto_out_hours: 8,
      };
    case "set_settings":
    case "clock_in":
    case "clock_out":
    case "session_start":
    case "session_pause":
      return null;
    /* 窗口/事件插件面：isFocused 恒真（分态纱初值兜底）、拖拽空操作、listen/unlisten 发假 id */
    case "plugin:window|is_focused":
      return true;
    case "plugin:window|start_dragging":
    case "plugin:event|unlisten":
      return null;
    case "plugin:event|listen":
      return Date.now() % 100_000;
    default:
      console.warn(`[mock-invoke] 未路由命令：${cmd}（返回 null）`);
      return null;
  }
}

/** 安装 mock：仅在无 Tauri 外壳的纯浏览器环境生效（真机不覆盖任何东西） */
export function installMockIpc(): void {
  const w = window as Window & { __TAURI_INTERNALS__?: unknown };
  if (w.__TAURI_INTERNALS__) {
    return; // 真 Tauri 环境：不覆盖
  }
  let seq = 0;
  w.__TAURI_INTERNALS__ = {
    metadata: { currentWindow: { label: "main" }, currentWebview: { label: "main" } },
    plugins: {},
    nextSeq: 0,
    invoke: async (cmd: string, args?: Record<string, unknown>) => routeCommand(cmd, args),
    transformCallback: (cb: unknown) => {
      seq += 1;
      (window as unknown as Record<string, unknown>)[`_${seq}`] = cb;
      return seq;
    },
  };
  console.info("[mock-invoke] DEV 基座已安装（无 Tauri 外壳，IPC 走假通道）");
}
