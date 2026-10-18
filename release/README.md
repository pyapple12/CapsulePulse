# CapsulePulse 使用说明

玻璃质感的工作计时看板——打卡开启一天，班内计时 = 工作、空隙 = 休息，下班出在岗 / 工作 / 休息三值与时间图谱。

---

## 快速上手

1. 把 `capsule-pulse.exe` 放到任意文件夹（建议单独建一个，如 `D:\CapsulePulse\`）；
2. 双击运行——首次启动会在 exe 同级自动创建 `configs\` 与 `data\` 两个文件夹（设置与数据都住这里，**不要删**）；
3. 玻璃小板出现在屏幕上，点「上班」开始记录今天。

> 绿色单文件版，无需安装；换电脑 = 整个文件夹拷走。

## 日常使用

### 打卡与计时

- **上班**：点底部「上班」→ 确认框 → 开始记录今日在岗；**班内计时 = 工作**；
- **休息 / 工作**：三段控件随时切换——**工作 = 继续计时，休息 = 暂停**（休息也在岗，只是不计入工作）；
- **重置**：把当前工作段收段落库、大计时器清零，回到休息态（不自动开跑）；
- **下班**：点「下班」→ 确认框 → 收段落库并结算今日在岗 / 工作 / 休息三值。

### 提醒与沙漏

- 主页沙漏 = 距离下次休息提醒的进度：**连续工作满阈值**（默认 50 分钟）时响铃（系统通知 + 提示音）并弹出文案条；
- 沙漏**沉底**表示"当前没有在计时"（休息 / 未上班 / 刚打开）；开始工作后沙漏翻转回满、按阈值匀速漏，**铃声响起那一刻沙子刚好流完**；
- 工作中改阈值：沙漏**不翻、水位不跳**，只改流速（终点仍对齐铃声）；阈值下限随已工作时长收窄，**铃响后本段锁定**（暂停后才能再改）。

### 统计板

点左上角统计钮打开：**时间图谱**（每班一段、班间灰、节点=打卡/下班/自动下班）、**三值**（在岗 / 工作 / 休息，含与历史平均的偏离）、**周卡**（自然周七柱 + 总日均）、**当日明细**；左右箭头前后翻日，周卡随查看日联动。

### 设置板

点右上角齿轮打开，七行即时生效：提醒阈值（步进器 / 可手打）、自动下班小时数（4–12，今日已打卡则整日锁定）、提醒通知（系统通知与提示音一行一开关）、窗口置顶、贴边吸附、显隐快捷键、状态快捷键（录制新组合，**最多 3 键**）。

### 托盘与快捷键

- 关闭窗口 = **缩回托盘**（不退出）；托盘图标**悬停**出今日概览悬浮卡（三值 / 剩余 / 下次提醒倒计时）；**右键**出自绘菜单（聚焦主窗 / 切换状态 / 置顶 / 贴边 / 退出）；
- 默认全局快捷键：**Alt+Shift+S** 显隐主窗、**Alt+Shift+P** 切换工作 / 休息（可在设置板改）；
- 应用**不出现在任务栏**（只在托盘）：`Alt+Shift+S` 或**左键点托盘图标**把主窗唤回前台；关闭窗口依旧是缩回托盘。

### 自动下班与崩溃恢复

- 今日在岗累计满「自动下班小时数」时，到点前 60 秒弹确认框（可选「加班」把上限 +4 小时，硬顶 12 小时）；到点未操作即自动下班记账；
- 异常退出后重启：停机 ≤10 分钟**无缝续**（继续计时），更久则按最后心跳补记下班并提示。

### 备份与迁移

拷走 exe 同级的 `configs\` 与 `data\` 两个文件夹即可（设置 + 全部记录）。删除 `configs\config.json` 会回到默认设置。

### 系统要求

Windows 10 / 11（64 位）。玻璃观感依赖系统合成器：**平时窗口纯透明**（失焦带轻纱），**打开统计板 / 设置板且窗口聚焦时**才启用系统毛玻璃背板（Win11 22H2+ 效果最佳，旧版本回退为透明）；首次启动会写入一条**当前用户**的通知署名注册表项（`HKCU\SOFTWARE\Classes\AppUserModelId\com.pyapple12.capsule-pulse`），让系统通知正确署名 CapsulePulse 并带图标——不需要管理员权限，可随时删除（下次启动会重建）。

---

## 更新日志

### v0.2.0（首个公开版本）

- 打卡 / 下班双向确认；工作 / 休息 / 重置三段计时；下班结算在岗 / 工作 / 休息三值
- 时间图谱 + 三值偏离 + 周卡（自然周）+ 当日明细，可前后翻日
- 提醒阈值可配置（步进器 / 手打 / 长按连发），系统通知 + 提示音双通道
- 沙漏与提醒**严格同步**：沉底是常态，改阈值只改流速、铃声响起时沙子刚好流完
- 自动下班（可配 4–12 小时）+ 到点前确认框 + 加班步进（硬顶 12 小时）
- 崩溃恢复：停机 ≤10 分钟无缝续，更久按最后心跳补记下班
- 托盘常驻：悬停今日概览卡 + 自绘右键菜单（聚焦 / 切换状态 / 置顶 / 贴边 / 退出）
- 全局快捷键可自定义（显隐 / 状态切换，最多 3 键）
- 窗口置顶、贴边吸附、位置记忆、双击标题缩回托盘
- 按需毛玻璃：打开统计板 / 设置板且窗口聚焦时才启用系统背板，平时纯透明
- 通知署名修复：绿色单 exe 也能署名 CapsulePulse + 正式图标
- **不进任务栏**（只在托盘，`Alt+Shift+S` / 托盘左键唤回）；release 版也不再弹控制台黑窗
- 崩溃恢复精修：无缝续把停机前那段时长也算进本段 ⇒ 重启后沙漏水位与提醒倒计时**接着走**（不再从零重启）

---

CapsulePulse V0.2.0 · 仅用于学习与交流

---

# CapsulePulse — User Guide

A glassy work-timer board: clock in to start a day; in-shift time counts as work, gaps as rest; clock out to get on-duty / work / rest totals plus a timeline graph.

---

## Quick start

1. Put `capsule-pulse.exe` in any folder (e.g. `D:\CapsulePulse\`);
2. Double-click it — the first launch creates `configs\` and `data\` next to the exe (settings and records live there, **do not delete**);
3. The glass board appears; click "Clock in" to start today.

> Portable single-exe build, no installer. To move machines, just copy the whole folder.

## Everyday use

### Clock-in and timing

- **Clock in**: bottom pill → confirm → today's on-duty record starts; **in-shift time = work**;
- **Rest / Work**: switch anytime — **Work = keep counting, Rest = pause** (rest still counts as on-duty, just not as work);
- **Reset**: closes the current work segment into storage, zeroes the big timer and returns to rest (it does not auto-start);
- **Clock out**: bottom pill → confirm → closes the segment and settles today's on-duty / work / rest totals.

### Reminder and hourglass

- The hourglass shows progress toward the next break reminder: at the threshold (default 50 min) of continuous work it rings (system notification + chime) and shows a banner;
- A **sunk** hourglass means "nothing is being timed" (rest / off duty / just launched); once you start working it flips over and drains evenly, so **the sand runs out exactly when the chime rings**;
- Changing the threshold mid-shift **does not flip the hourglass and does not jump the water level** — only the drain rate changes (the end still lands on the chime); the lower bound narrows with elapsed work, and after a ring the segment **locks** (pause to change it again).

### Stats board

Open with the chart button (top-left): **timeline graph** (one bar per shift, grey gaps, nodes = clock-in / clock-out / auto clock-out), **totals** (on-duty / work / rest with deviation from your historical average), **week card** (natural week, seven bars + all-time daily average) and **today's detail**; arrows page through days, the week card follows.

### Settings board

Open with the gear (top-right); seven rows apply instantly: reminder threshold (stepper or typed), auto clock-out hours (4–12; locked for the day once you clock in today), reminder notifications (system notification and chime on one switch), always on top, snap to edge, show/hide hotkey, state hotkey (record a new combo, **at most 3 keys**).

### Tray and hotkeys

- Closing the window **hides it to the tray** (it does not quit); **hover** the tray icon for a today overview card (totals / remaining / next reminder countdown); **right-click** for a custom menu (focus main window / toggle state / always on top / snap to edge / quit);
- Default global hotkeys: **Alt+Shift+S** show/hide, **Alt+Shift+P** toggle work / rest (both changeable in settings);
- The app has **no taskbar entry** (tray-only): bring the window back with `Alt+Shift+S` or a **left-click on the tray icon**; closing the window still hides it to the tray.

### Auto clock-out and crash recovery

- When today's on-duty total reaches the configured hours, a confirm dialog appears 60 seconds before the deadline (choose "Overtime" to add 4 hours, hard cap 12); if untouched, it clocks out automatically and books the time;
- After an abnormal exit: downtime ≤10 minutes **resumes seamlessly** (timing continues); longer downtime books a clock-out at the last heartbeat and tells you.

### Backup and migration

Copy the `configs\` and `data\` folders next to the exe (settings + all records). Deleting `configs\config.json` restores default settings.

### Requirements

Windows 10 / 11 (64-bit). The glass look depends on the system compositor: the window is **fully transparent by default** (a light veil when unfocused) and only uses the system frosted backdrop while a stats/settings board is open and the window is focused (best on Win11 22H2+; older builds fall back to transparent). The first launch writes one **current-user** notification-identity registry key (`HKCU\SOFTWARE\Classes\AppUserModelId\com.pyapple12.capsule-pulse`) so that system notifications are properly signed as CapsulePulse with its icon — no admin rights needed, safe to delete (it is recreated on the next launch).

---

## Changelog

### v0.2.0 (first public release)

- Two-way confirmation for clock-in / clock-out; work / rest / reset three-state timing; clock-out settles on-duty / work / rest totals
- Timeline graph + totals deviation + natural-week card + daily detail, with day-by-day paging
- Configurable reminder threshold (stepper / typed / press-and-hold repeat), system notification + chime
- Hourglass **strictly synchronised** with the reminder: sunk is the default state, changing the threshold only changes the drain rate, and the sand runs out exactly when the chime rings
- Auto clock-out (4–12 hours configurable) with a pre-deadline confirm dialog and overtime stepping (hard cap 12 h)
- Crash recovery: downtime ≤10 minutes resumes seamlessly, longer downtime books a clock-out at the last heartbeat
- Tray residency: hover overview card + custom right-click menu (focus / toggle state / always on top / snap / quit)
- Customisable global hotkeys (show-hide / state toggle, at most 3 keys)
- Always on top, snap to edge, window position memory, double-click the title to hide to tray
- On-demand frosted backdrop: system backdrop only while a board is open and the window is focused; otherwise fully transparent
- Notification signature fixed: the portable single exe is signed as CapsulePulse with its real icon
- **No taskbar entry** (tray-only; bring it back via `Alt+Shift+S` or a left-click on the tray icon); the release build no longer opens a console window either
- Crash-recovery refinement: a seamless resume now counts the pre-downtime portion of the current segment, so the hourglass level and the reminder countdown **continue** instead of restarting from zero

---

CapsulePulse V0.2.0 · For learning and exchange only
