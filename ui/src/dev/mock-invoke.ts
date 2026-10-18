/**
 * DEV 冒烟基座（CT 模式移植）：浏览器直接打开 ui/（无 Tauri 外壳）时拦截 IPC 全家，
 * 让映射期的对比探针能在 IAB 里驱动真实组件树。生产构建经 main.ts 的
 * import.meta.env.DEV 死分支静态消除，dist 零痕迹。
 * 种子数据对齐 design 实验场演示态（在岗计时中 / 阈值 50 / 自动下班 8h / 有统计）。
 * **IPC DTO 有增删必须同步本基座**（FIX007.10）：类型镜像在 `ui/types.ts`、数据源在本文件——
 * 只改前者会出现"基座缺字段 ⇒ DEV 专属错渲染"（`?? 0` 兜底成 1970 起点、布尔缺省成 false）。
 * 故各 DTO 分支的返回对象一律用 `satisfies <DTO>` 标注：类型系统替"记得同步"把关，
 * 漏字段 / 拼错字段在 `vue-tsc` 阶段即失败（仅类型约束，零运行时开销）。
 */
import type {
  DaySummary,
  HoverSummary,
  RecoveryNotice,
  ReminderSettings,
  SessionStatus,
  WeekDay,
  WorkdayTotal,
} from "../../types";

/** MM-DD 标签（周卡日期列，与后端 WeekDay.date 同格式） */
function mmdd(d: Date): string {
  return `${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

/** DEV 会话状态机（PATCH002.2）：打卡与各会话命令都改这份状态——让"打卡即工作 / 三段切换 /
 * 重置回休息"这类链路在 DEV 基座可复现、可验证（此前 session_status 写死 running，验不了）。
 * 初值 = 在岗计时中（段起点取今日 09:00，对齐 day_detail 的演示态）。 */
/** 基座提醒阈值（分钟）——与 `get_settings` 返回值同源，供 `reminder_fired` 推导（PATCH010.6） */
const MOCK_THRESHOLD_MIN = 50;
let mockState: "idle" | "running" | "paused" = "running";
let mockOnDuty = true;
let mockAccumMs = 0; // 已完成段累计（毫秒）
let mockSegStart = (() => {
  const t = new Date();
  t.setHours(9, 0, 0, 0);
  return t.getTime();
})();

/** 当前累计毫秒（Running = 累计 + 本段实时） */
function mockTotalMs(): number {
  return mockAccumMs + (mockState === "running" ? Math.max(0, Date.now() - mockSegStart) : 0);
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
      return {
        state: mockState,
        total_ms: mockTotalMs(),
        on_duty: mockOnDuty,
        // PATCH005.4：当前数据日偏移（基座恒当日；跨零点夜班场景由自查时临时置 −1）
        data_day_offset: 0,
        // PATCH010.6：本段连续工作毫秒（仅 running 非零；paused/idle 归零 ⇒ 沙漏沉底）
        segment_ms: mockState === "running" ? Math.max(0, Date.now() - mockSegStart) : 0,
        // PATCH010.6：本段是否已触发过提醒（基座阈值恒 50 分钟 ⇒ 达阈值即真，用于验阈值行锁定）
        reminder_fired:
          mockState === "running" && Date.now() - mockSegStart >= MOCK_THRESHOLD_MIN * 60_000,
      } satisfies SessionStatus;
    case "take_recovery_notice":
      // PL027.8：一次性崩溃恢复提示——基座恒返回 null（无缝续静默路径）
      return null satisfies RecoveryNotice | null;
    case "take_auto_out_notice":
      // PATCH014.7：一次性"即将自动下班"待决探针——基座恒返回 null（无待决通知）
      return null satisfies number | null;
    case "day_detail": {
      // 单位对齐真实后端：时间戳一律秒（SessionStatus.total_ms 例外为毫秒）
      const start = Math.floor(todayStart.getTime() / 1000);
      const nowSec = Math.floor(now / 1000);
      // PATCH003.2：让 DEV 基座能复现"已下班 → 再打卡"过渡（此前恒返回在岗态，验不了）
      if (mockOnDuty) {
        const used = Math.floor(elapsedMs / 1000);
        return {
          duty_started_at: start,
          duty_ended_at: null,
          duty_secs: used,
          work_secs: used,
          rest_secs: 0,
          budget_hours: 8, // PATCH005.3：基座固定基准 8h
          used_secs: used,
          remaining_secs: Math.max(0, 8 * 3_600 - used),
          // FIX007.10：与 duty_started_at 同源（当前班起点秒，供表盘底轨 / 轴末起算）
          shift_start: start,
          can_overtime: true, // 未跨零点且基准 < 12h 硬顶 ⇒ 可加班（自动下班确认框的加班钮）
          cap_reached: false,
          blocks: [{ start, end: nowSec, kind: "work" }],
        } satisfies DaySummary;
      }
      const noon = start + 12 * 3_600;
      const onePm = start + 13 * 3_600;
      return {
        duty_started_at: start,
        duty_ended_at: onePm,
        duty_secs: 3 * 3_600,
        work_secs: 2 * 3_600,
        rest_secs: 3_600,
        budget_hours: 0, // PATCH005.3：暂下班 → 不画蓝条、轴末取末段末尾（预算字段 0）
        used_secs: 3 * 3_600,
        remaining_secs: 0,
        // FIX007.10：当日已收尾，起点仍如实给出（底轨画的是"今天这一班"）
        shift_start: start,
        can_overtime: false,
        cap_reached: false,
        blocks: [
          { start, end: noon, kind: "work" },
          { start: noon, end: onePm, kind: "rest" },
        ],
      } satisfies DaySummary;
    }
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
      }) satisfies WeekDay[];
    }
    case "workday_total":
      // 全历史三值总和 + 有数据天数（PL024.6：前端据此算各值平均与偏离）
      return {
        duty_secs: 115_200,
        work_secs: 93_600,
        rest_secs: 21_600,
        days: 12,
        first_offset: -11,
      } satisfies WorkdayTotal;
    case "get_settings":
      return {
        threshold_min: 50,
        sound_enabled: true,
        notify_enabled: true,
        workday_auto_out_hours: 8,
        always_on_top: false,
        snap_to_edge: true,
        hotkey_toggle_visible: "Alt+Shift+S",
        hotkey_toggle_timer: "Alt+Shift+P",
        window_pos: null, // FIX007.10：位置记忆为空（基座无真实窗口位置）
      } satisfies ReminderSettings;
    /* PL028.2 录键：基座无注册能力，回显规范化串（真注册与冲突拒绝在 Rust 侧） */
    case "set_hotkey":
      return String(args?.combo ?? "");
    /* PL028.1 悬浮概览：基座给"在岗三行"样例，**按最长行取值**（供窗宽核对：10小时09分钟）
       —— 真值由 Rust 侧聚合，段带/四状态见 hover.rs 用例 */
    case "tray_hover":
      return {
        state: "on_duty",
        label: "本班",
        remaining_secs: 10_860,
        work_secs: 36_540,
        rest_secs: 0,
        until_reminder_min: 48.3,
        single_line: false,
      } satisfies HoverSummary;
    case "set_settings":
    case "main_hide_to_tray":
    case "set_board_backdrop":
    case "tray_menu_action":
    case "tray_menu_resize":
    case "tray_hover_resize":
      return null;
    /* 会话链（PATCH002.2）：与后端同语义——打卡即工作、工作/休息幂等切换、重置收段归零回休息 */
    case "clock_in":
      mockOnDuty = true;
      mockState = "running";
      mockAccumMs = 0;
      mockSegStart = Date.now();
      return null;
    case "clock_out":
      mockOnDuty = false;
      mockState = "idle";
      mockAccumMs = 0;
      return null;
    case "session_set_run_mode": {
      const mode = args?.mode;
      if (mode === "work") {
        if (mockState !== "running") {
          mockState = "running";
          mockSegStart = Date.now();
        }
      } else if (mode === "rest" && mockState === "running") {
        mockAccumMs = mockTotalMs();
        mockState = "paused";
      }
      return null;
    }
    case "session_restart":
      mockAccumMs = 0;
      mockState = "paused";
      return null;
    /* PL025.9：加班（后端把基准拉满 12h）——DEV 基座无基准概念，只做"命令存在"的占位 */
    case "overtime":
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
