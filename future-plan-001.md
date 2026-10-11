# Future Plan 001 — CT 可移植项候选池（PATCH001 之外）

> 状态：**候选池**（2026-10-11 整理，源自 CapsulePulse ↔ CapsuleTODO 全量对照）。已由 PATCH001 收走的四条不在本文件：粒子 mouseleave 回位 / 标题双击缩回托盘（含阈值把手）/ 聚焦态窗口框架整定（激活描边隐藏 + 系统圆角；**背板本体未做**，见本文件 §五）/ 窗口位置记忆。
> 用法：任一项被用户发起后，方案细化入 `z.plan.md` 附录（`PATCH{NNN}` 或 `PL{NNN}`，按体量定），执行条目入 `x.progress.md`，本文件对应条目销项（保留条目 + 标注去向，不删）。
> CT 出处一律给「文件:行」，对照基线 = CapsuleTODO V0.2.2.3（`E:\CodeMission\CapsuleTODO`）。

---

## 一、窗口与外观

### 1. 主题三态持久化（跟随系统 / 浅 / 暗）

> **立项状态**：🔭 未立项。前置条件已具备度最高的一项。

- **CT 出处**：`core/src/settings.rs`（theme 字段 + 越界钳制）、`core/src/commands/settings.rs`（get/set 命令）、`ui/App.vue:196-197,272-279`（theme ref + prefs-changed 回显）、`ui/src/components/SettingsOverlay.vue`（跟随系统 switch + 日夜切换）、`ui/src/composables/titleParticles.ts:323-329`（`refresh()` 只重读辉光/落影参数不重建）
- **CP 现状**：`ui/index.html:2` 硬锁 `data-theme="dark"`；`ui/src/styles/glass.css` 的**三块令牌其实已就位**（浅色默认块 / `prefers-color-scheme` 跟随块 / `data-theme` 手动块），只缺「UI + 持久化 + 解锁 index.html」
- **移植要点**：`ReminderSettings` 加 `theme: u8`（0 跟随 / 1 浅 / 2 暗，serde default 0）→ 启动回读写 `documentElement.dataset.theme`；设置板加双控件（跟随 switch + 日夜切换，照 CT settings.css 配方）；切档即落库 + 写 dataset
- **连带**：粒子引擎需补 `refresh()` 口（CP 现返回裸清理函数，CT 返回 `{refresh, destroy}` 对象）；`apply_frame_style` 的 `dark` 参数改读实际主题（与 PATCH001.3 耦合）
- **成本**：低-中（令牌与结构都在，工作量集中在设置板 UI 与回读链）

### 2. 窗口置顶开关（always_on_top）+ 全屏自动让位

- **CT 出处**：`core/src/settings.rs:98-102`（字段 + serde default）、`core/src/lib.rs:298`（启动按配置校正）、`core/src/tray.rs:123-155`（托盘菜单取反 + 命令核心复用）、`core/src/fullscreen.rs`（全屏应用覆盖时自动摘置顶，恢复时回挂）
- **CP 现状**：无（`tauri.conf.json` 亦未声明 `alwaysOnTop`，默认 false）
- **移植要点**：设置字段 + 设置板 switch + 启动校正 + 托盘菜单项；`fullscreen.rs` 属可选增强（单独评估，CP 窗口 300×400 被全屏应用盖住的概率不低）
- **成本**：中（字段化设置命令 + 菜单联动；见本文件 §三.9）

### 3. 贴边吸附（snap_to_edge）

- **CT 出处**：`core/src/snap.rs`（**纯逻辑整文件可拿**：四边 ±50px 阈值、落位 5px、幂等防死循环，10 个单测）、`core/src/lib.rs:76-127`（`snap_if_needed`，视觉矩形 vs outer 矩形换算吸收隐形 resize 边）、`core/src/settings.rs`（`SNAP_THRESHOLD_PX` / `SNAP_GAP_PX` 单一来源）
- **CP 现状**：无
- **移植要点**：`snap.rs` 原样落位（零依赖纯函数，与 CP「业务纯逻辑可单测」纪律同构）；接线挂 `WindowEvent::Moved` 防抖状态机（松手吸附，CT 定案）；设置字段 + 托盘菜单项
- **成本**：中（纯逻辑零风险，成本在 Moved 状态机与设置项）

### 4. 窗口系统圆角（apply_round_corners）

> **立项状态**：✅ **已闭环 PATCH001.5**（2026-10-11；移植 CT 同日 `e8d049b`——`tauri.conf.json` 加 `shadow: false` 关掉 Tauri 给无边框窗的投影（消顶部 1px 框线），关投影会**连带丢失隐式圆角**，故显式补 `DWMWA_WINDOW_CORNER_PREFERENCE = ROUND`；方案见 `z.plan.md` 附录 PATCH001 §二、条目见 `x.progress.md` PATCH001.5）。**原"待判定"结论已修正**：CP 单窗同样需要——不是多窗暗角问题，而是 Tauri `shadow: true`（默认）的 1px 边框 + 隐式圆角在关投影后一并消失。

- **CT 出处**：`core/src/glass_backdrop.rs:84-94`（`DWMWA_WINDOW_CORNER_PREFERENCE = ROUND`；Win10 无此属性、调用失败无害）、`core/src/lib.rs:473-474,502-503`（托盘预览窗与自绘菜单窗）
- **CP 现状**：CSS `--radius-board: 8px` 只切了玻璃层，OS 窗口矩形仍是直角（`shadow: true` 时由 DWM 隐式圆角兜着，关投影即露方角）
- **成本**：极低（一个函数 + setup 一次调用）——已随 PATCH001.5 落地

---

## 二、托盘与热键

### 5. 托盘菜单扩展 + 勾选态

- **CT 出处**：`core/src/tray.rs`（菜单项：聚焦 / 吸附 / 置顶 / 退出 + 勾选态取反）、`ui/src/components/TrayMenu.vue`（自绘菜单窗，`ui/App.vue:270-279` 经 `prefs-changed` 广播回显设置板）
- **CP 现状**：`core/src/lib.rs:105-132` 三项（显示/隐藏、开始/暂停、退出），无勾选态、无自绘
- **移植要点**：最小版 = 用 Tauri 原生菜单加 `CheckMenuItem` 承载置顶/吸附（不引入自绘菜单窗）；自绘菜单窗属 §6 范畴，不混做
- **成本**：低（最小版）/ 高（自绘版）

### 6. 托盘悬停预览窗

- **CT 出处**：`ui/src/components/TrayPreview.vue`、`core/src/lib.rs:460-517`（预览窗创建：`focused(false)` / `always_on_top(true)` / 首绘后隐藏）、`core/src/tray.rs`（常驻守候线程 + 悬停延迟出现状态机 + 右键让位）
- **CP 现状**：无
- **移植要点**：多窗架构（第二 `WebviewWindow` + 生命周期 + 幽灵图标收尾 `RunEvent::ExitRequested` 显式 `remove_tray_by_id`）；**必须同时补 `window-focus` 的 label 过滤**（见 §三.10），否则预览窗获焦会把主窗纱态翻反
- **成本**：高（CT 侧历经 PL015/PL018 三个里程碑才稳），建议单独立项、不与其他项捆绑

### 7. 全局热键可录入（设置板热键录制）

- **CT 出处**：`ui/src/components/SettingsOverlay.vue`（点击后按下组合即录入 + 占用红字回退）、`core/src/settings.rs`（`bubble_hotkey` 字符串 + `DEFAULT_BUBBLE_HOTKEY`）、`core/src/lib.rs:316-330`（启动注册 + 锁读失败可见化）
- **CP 现状**：`core/src/lib.rs:135-152` 硬编码 `Alt+Shift+P`（计时切换）/ `Alt+Shift+S`（显隐），不可配置
- **移植要点**：设置字段 + 录入 UI + 注册失败可见反馈（占用冲突检测）；注意 CP 有两个热键（CT 只有一个），要么各自可配、要么只开放其中一个
- **成本**：中-高

### 8. `tauri.conf.json`：`skipTaskbar`

- **CT 出处**：`core/tauri.conf.json:26` `"skipTaskbar": true`
- **CP 现状**：未设 → 任务栏常驻图标
- **待定**：CP 是「常驻看板」，任务栏留入口 vs 纯托盘常驻是产品取舍（CT 已选隐藏）
- **成本**：极低（一行）

---

## 三、架构参考（非功能项，供后续立项时参照）

### 9. 设置命令字段化

- **CT 出处**：`core/src/commands/settings.rs` 每字段一对命令 + `*_core` 自由函数（`settings_set_always_on_top_core` 等），托盘/设置板/启动三入口复用同一核心
- **CP 现状**：`core/src/commands/reminder.rs:46-54` 是**整结构** `set_settings`（前端提交整份设置即全量覆盖）
- **影响面**：整结构覆盖与「Rust 自管字段」天然冲突——PATCH001.4 的 `window_pos` 必须在 `set_settings_inner` 里显式继承；往后每加一个 Rust 自管字段（位置/主题/置顶…）都要再加一次继承，属结构性债务
- **建议**：加到第三个 Rust 自管字段时，把 `set_settings` 拆成字段级命令（或把自管字段拆出 `ReminderSettings`），一次性了结

### 10. `window-focus` 的 `label == "main"` 过滤

- **CT 出处**：`core/src/lib.rs:599-604`（FIX005.2：预览窗获焦不挂背板、不广播焦点态）
- **CP 现状**：单窗口，无需过滤
- **触发条件**：一旦引入第二个 WebviewWindow（托盘预览 / 自绘菜单）**必须同步补**，否则主窗纱态与真实焦点相反

---

## 四、不适用登记（对照后判定 CP 不需要）

| CT 件                                                                     | 判定依据                                                                        |
| ------------------------------------------------------------------------- | ------------------------------------------------------------------------------- |
| `useGlassBar` / `useBoardRead` / `useScrollKit`（滑杆 / 三角 / 滚动套件） | CP 无滚动容器（`.window` `overflow: clip`，PL019.1 定案），无滚动即无滑杆与三角 |
| `useMaskDead`（罩死行判定）                                               | CP 无归档板 / 行内编辑 / 拖拽排序等「拍子武装」交互                             |
| `useDragReorder` + `delConfirmBus`                                        | CP 无列表拖拽与两段删除确认                                                     |
| `useEmptyState`                                                           | CP 空态是图谱/三值的 `na:na` 显式渲染（PL024.5），形态不同                      |
| 粒子 `touchmove` / `scroll` 刷新 zone                                     | 桌面端无触摸；CP 无滚动，`zone` 恒等于窗口矩形                                  |
| `tauri.conf.json` CSP + `assetProtocol`                                   | CP 无图片/外部资源（CT 为图片气泡落的），不引入 CSP 约束                        |

---

## 五、聚焦真磨砂背板（DWM 系统亚克力）——若做，先读两条基础结论

> **立项状态**：🔭 未立项（2026-10-11 用户明确"只要没有蓝框"，故 PATCH001 只做窗口框架整定、不做背板）。CT 侧为现行实现（`core/src/glass_backdrop.rs:53-63` `set_focused_backdrop`：聚焦 `DWMSBT_TRANSIENTWINDOW=3`、失焦 `DWMSBT_NONE=1`——**非 0，0 是 AUTO**；CT 计划书标注此配方为"✅ CapsulePulse 实测配方"= CP 自己 PL011 的旧功能回迁）。
> **代价**：Windows 分支回归（CT `core/src/lib.rs:599-619` 的 `#[cfg]` 包层 + 非 Windows 的 `let _ = (focused, window);` 疤痕）；且 macOS/Linux 聚焦态将保持"无背板"，三端观感分叉。
> **前置阅读**：下面两条结论是 2026-10-11 逐行对照 + 源码追踪（Tauri `shadow` → tao `MARKER_UNDECORATED_SHADOW`）得出的，**做背板前必须先读**。

### 11. 基础结论 A：深色声明（`DWMWA_USE_IMMERSIVE_DARK_MODE`）——加背板时必须补

- **作用**：窗口级深色基调声明；DWM 据此渲染该窗口的**框架与系统背板**的基调。
- **CT 实测**：不声明时背板按浅色基调渲染——"深色主题系统上白雾化"（CT 注释原文：2026-09-29 实测"变白"根因）。
- **CP 现状（2026-10-11 结论）**：**无用**——CP 既无框架（`shadow: false` 已拆）也无背板（PL021.4 定案不挂 OS 材质）→ DWM 对该窗口不渲染任何东西，基调声明没有作用对象。故 PATCH001 的移植里**没有**它（不是遗漏）。
- **触发条件**：一旦挂背板，**必须同步补**，否则浅色主题机器上背板发白。
- **附**：与前端 `data-theme` 无关（CP 暗色由 `ui/index.html:2` 锁定，DWM 不参与）。

### 12. 基础结论 B：边框隐藏（`DWMWA_BORDER_COLOR = NONE`）——加背板后由"冗余"变"必需"

- **机制**：`DWMWA_BORDER_COLOR` 只管"给窗口框架的边框上什么颜色"——**框架没了，它就没有作用对象**。
- **CP 现状（2026-10-11 结论）**：**冗余**——CP 的框架只有一个来源 = Tauri `shadow: true`（tao 的 `MARKER_UNDECORATED_SHADOW` 会给无边框窗保留 1px 非客户区边框，`tao-0.35.3/src/platform_impl/windows/event_loop.rs:2164-2174`）；PATCH001.5 的 `shadow: false` 把框架本体拆掉后，PATCH001.3 的边框隐藏已无对象（用户 2026-10-11 实机确认：Win11 框线 + 顶部 1px 均消失）。**保留不撤**的理由见下条。
- **加背板后**：**变必需**——背板本身会让 DWM 按完整窗口框架渲染，Win11 活动描边随之显形（CT 注释原文"背板挂载后 DWM 按完整窗口框架渲染，Win11 活动描边显形——实测蓝色一圈根因"）；这条路径与 Tauri `shadow` 无关、`shadow: false` 关不掉。CT 因此三者并存（`apply_frame_style` 的边框隐藏 + `shadow: false` + `apply_round_corners`），**各打一条路径、均不冗余**。
- **行动**：撤不撤 PATCH001.3 那次调用，与"是否做背板"**绑定决策**——现在撤、将来做背板还得加回；保持现状则零成本兜底（少一次 FFI vs 一条一行恢复的保险）。

---

## 六、待定项（立项前需用户拍板）

1. **主题三态**要不要连「跟随系统」档一起做（CP 现在是恒暗；跟随档会让**将来背板的深色声明**需要动态化）？
2. **置顶 / 吸附**默认值取 CT 的「默认关」（V0.2.0.0 定案），还是 CP 场景下默认开？
3. **托盘预览窗**是否值得做（成本最高，且会牵出多窗生命周期 + label 过滤 + 幽灵图标三处收尾）？
4. **`skipTaskbar`**：CP 要不要从任务栏消失？
5. **热键**：两个热键（计时切换 / 显隐）都开放录入，还是只开放其一？
6. **聚焦真磨砂背板**做不做（见 §五）——做了：带回 Windows 分支 + **必须补深色声明** + 边框隐藏由"冗余"变"必需"；不做：PATCH001.3 那次边框隐藏调用可撤。
