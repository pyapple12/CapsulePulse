# 项目方案与审计归档（z.plan.md）

> 文件职责：方案文档与审计归档。与 `x.progress.md`（任务清单）、`CapsulePulse_plan.md`（总体规划）分工：总体规划不动，方案演进与审计记录都落在本文件。
> 结构：一、已完成 ✅ → 二、待完成 → 三、主题规划（按需）→ 四、审计观察项豁免定案清单 → 附录 PL{NNN}（专题方案，立项时创建）→ 附录 A{NNN}（审计报告，由 audit-report 归档环节生成）。

## 一、已完成 ✅

- **PL002 存储与统计聚合**（2026-09-09 收口）→ 附录 PL002
- **PL001 玻璃壳与最小计时闭环**（2026-09-08 收口）→ 附录 PL001

## 二、待完成

- （暂无——下一个大件：计划书 Phase 3 提醒调度（PL003 候选）或 Phase 2 托盘常驻，未立项）

## 三、主题规划

- （按需）跨附录主题出现时在此登记；远期与遗留项见 y.problems.md

## 四、审计观察项豁免定案清单

> 豁免唯一权威源：已定案项审计时（audit-project）不再重复报告。新定案条目由归档环节（audit-report）经用户确认后追加。分级：①**永久豁免**——不再报告不再讨论；②**条件豁免**——标注触发条件，条件变化时重新评估。

（暂无）

---

## 附录 PL001：玻璃壳与最小计时闭环（2026-09-08 立项）

> 背景：CapsulePulse 代码零起步。产品核心洞察 = "按下按钮 → 时间在玻璃上看板上走"——PL001 用最薄一刀垂直打穿这条链路（对应 CapsuleRetro PL001 打穿数据链路的方法论），同时回答本项目最大的技术风险：Tauri 2 透明窗口 + window-vibrancy(Acrylic) + CSS backdrop-filter 的玻璃栈在本机是否成立。
> 关键洞察：① 玻璃栈是唯一可能判死产品形态的风险（SQLite/通知/托盘皆为成熟路径），必须最先验证，作前置判定点；② 计时状态机（纯逻辑、无外部依赖）是 Rust 入门最好的第一课，TDD 工作流随第一个模块一起立起来；③ 每秒 tick 走最简方案（前端 setInterval 拉取），Rust 推送留到有需要再议（YAGNI）。
> 目标：收口时交付"玻璃窗口里能计时的最小闭环"——透明 Acrylic 窗口 + 大计时器数字每秒走 + 开始/暂停按钮切换；状态机 cargo test 全绿，门禁四件套全绿。
> 状态：✅ 已完成（2026-09-08 收口，G/S/U 全过；任务清单见 x.progress.md「PL001」；实测结论见下）

### 方向定案（2026-09-08，用户拍板）

1. **范围 = 最小计时闭环**：SQLite/统计聚合、提醒调度、托盘常驻一律不入本期（→ PL002/PL003）
2. **平台 = Windows 实机优先**：计划书 §2.4/§4 "macOS 主开发/验证平台" 在本项目调整为 Windows 实机优先——macOS vibrancy 与 Linux blur 适配整体延后至 Windows 版成熟后（登记 [problems#1]）；代码按 `#[cfg(target_os)]` 分支预留，但只在 Windows 实测
3. **tick 方案定案**：前端 setInterval 每秒 invoke 读取状态与 total()，Rust 侧不做推送

### 实现措施（按层拆解到文件/函数级）

#### 阶段 A：工程骨架

- **工具链探测**：cargo / rustc / node / npm / Tauri CLI 在位与版本盘点，缺口安装清单交用户安装；git init 由用户执行（.gitignore 已就位）
- **Tauri 2 骨架**：`src-tauri/`（Cargo.toml、tauri.conf.json 透明无边框窗口 360×480、main.rs 空壳）+ `src/`（Vue3 + TS + Vite，App.vue 占位）+ `.prettierrc`（已建）
- **门禁接入**：cargo fmt / clippy -D warnings / cargo check + prettier / vue-tsc / npm build 首跑全绿，记录耗时基线（回写本附录）

> **阶段 A 开展结论（2026-09-08，PL001.1–3 完成）**
>
> - 工具链全在位（cargo 1.96.1 / Node 26.7.0 / VS Build Tools 18 含 MSVC），零缺口；Tauri CLI 不装 cargo 版（长编译），走 npm devDep `@tauri-apps/cli`——探测记录 `.temp/pl001a-toolchain.md`
> - 骨架落地：`src-tauri/`（透明无边框 360×480；**不配 devUrl 自包含**，规避系列已知的 debug exe 连 dev server 白屏坑）+ Vue3/TS/Vite 前端；占位图标程序生成（`.temp/gen-icon.mjs`）；**等价验证通过**——debug exe 启动 MainWindowTitle=[CapsulePulse]，进程受控关闭
> - 依赖版本锚定：typescript 钉 5.x（5.9.3）——npm 默认解析到 TS 7.0.2（原生编译器）与 vue-tsc 3.3 不兼容（ERR_PACKAGE_PATH_NOT_EXPORTED），如实记录
> - 门禁首跑基线：fmt <1s / clippy -D warnings 2m10s（含全量依赖编译）/ check 5s / cargo build 2m12s（12.5MB debug exe）/ npm build 4s——全绿

> **阶段 B 玻璃判定书（2026-09-08，全过——不触发降级）**
>
> | #   | 判定项         | 结论 | 证据与经过                                                                                                                                                                                                                          |
> | --- | -------------- | ---- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
> | G1  | 透明 + Acrylic | ✅   | 用户确认：边缘透明生效、Acrylic 模糊可见、无黑底、无不可接受闪烁（`apply_acrylic` tint (32,32,32,125)）                                                                                                                             |
> | G2  | 卡片毛玻璃     | ✅   | 用户确认：卡片 backdrop-filter 毛玻璃质感成立、层次清晰                                                                                                                                                                             |
> | G3  | 深浅色跟随     | ✅   | 用户确认：`prefers-color-scheme` 双主题跟随实现、可读                                                                                                                                                                               |
> | G4  | 无边框拖动     | ✅   | **首测判负 → 修复后复测过**：`data-tauri-drag-region` 不生效，根因 = `core:window:default` 权限集只含只读查询、不含 `allow-start-dragging`，拖动 IPC 被 ACL 静默拒绝；capabilities 显式补授后用户复测拖动正常、无文字误选、尺寸固定 |
>
> - **判定：玻璃栈成立**，产品视觉身份可落地，继续阶段 C/D；降级预案（半透明纯色）不启用
> - **关键陷阱沉淀**（Tauri 2 ACL 静默拒第二例，系列知识）：`data-tauri-drag-region` 走 `start_dragging` IPC 命令，需显式授 `core:window:allow-start-dragging`——ACL 拒绝的典型症状 = 功能静默失效、控制台零报错；已登记 AGENTS.md 素材与环境陷阱 + w.study §3.1
> - 门禁回归：npm build 3.5s / clippy 7.5s / cargo build 25s 全绿（window-vibrancy 0.8.0 新增入账）

> **阶段 C 开展结论（2026-09-08，PL001.8–1.10 完成，TDD 红→绿）**
>
> - **TDD 过程**：先写 7 用例（仅测试模块）跑 `cargo test` 确认红灯（E0405/E0433 类型未实现）→ 实现后转绿 7/7；零真实 sleep，全部经手拨假钟验证
> - **落地形态**：`core/session.rs` = `WorkSession<C: Clock = RealClock>` 持态 + `SessionState` 三态（Idle/Running{start}/Paused{elapsed}，Copy enum）+ `SessionError`（thiserror：NotIdle/NotRunning/NotPaused）；Clock 契约 = 单调不减（假钟违约 panic 属程序错误，已文档化）
> - **实现决策两笔**：① 新增第五操作 `reset()`（任意态回 Idle 清零、幂等）——验收场景 U1"暂停态重开归零"所需的最小扩展，`start` 保持"仅 Idle"严格报错语义不变；② 工程转 Tauri 2 标准骨架 `lib.rs`（pub mod core + run() 装配）+ `main.rs` 薄入口——dead_code 门禁暴露 bin crate 形态问题（pub 项无外部消费者即报死代码），lib 形态永久消除该误报，阶段 D commands.rs 随之落 lib 侧
> - **用例清单（7）**：Idle 零总量 / 多轮循环累计（暂停冻结 + resume 继承 + Running 现算）/ Running 实时增长 / 非法态严格报错 / 零时长会话 / 假钟百轮狂拨（含零步进与超大步进）无 panic / reset 幂等清零
> - **门禁**：fmt / clippy --all-targets -D warnings / check / doc（0 告警）/ test 7 过 / build 全绿；thiserror 2.0.20 新增入账
> - **附带定案（2026-09-08，用户拍板）**：目录风格改造——`src-tauri/`→`core/`（整体改名）、`src/`→`ui/`、内层 `core/` 模块摊平（session.rs 平铺 src 根）、`configs/` 预建；根目录分类学 = ui/core/configs/modules（modules 待规模需要再建）。机制实证：框架文件必须与 Cargo.toml 同住（tauri-build 源码），CLI 按配置文件探测、目录名无关（tauri-cli app_paths.rs 源码）；阶段 D 起按新布局落码

> **阶段 D 开展结论（2026-09-08，PL001.11–1.12 完成，TDD 红→绿 + U1 三轮验收）**
>
> - **命令层**：`SessionHandle(Mutex<WorkSession>)` 经 `.manage()` 注册；命令核心抽为接收 `&SessionHandle` 的自由函数——U2 无窗口 cargo test 直测（4 用例），`#[tauri::command]` 仅解包 State。`CommandError`（Session 透传 / Poisoned）严格报错、跨 IPC 序列化为文案；内部计时字段不出 IPC，DTO 仅 `state` 标识 + `total_ms`
> - **实现决策**：① 新增第五命令 `session_restart`（reset + start 同锁原子）——U1"暂停态重开归零"的承载；② serde（derive）直依赖 + `@tauri-apps/api` 2.11.1（tauri info 实测缺失）；③ 自定义命令不经 ACL（系列实证，本轮再证）
> - **前端**：`ui/components/TimerCard.vue`（等宽数字 tabular-nums、按钮三态 开始/暂停/继续+重开、tick 拉取 + 卸载清理、动作后立即刷新）；App.vue 沿用玻璃定案
> - **U1 三轮验收（显示体验驱动两次修正）**：轮 1 = 秒进位最坏迟到 2s（1s 轮询与开始时刻相位错配 + 秒截断）判不可接受 → tick 250ms；轮 2 = 确认 1.0~1.25s 属 HH:MM:SS 秒表语义（"01"必然满 1 秒），静止感仍在 → **用户拍板显示定案变更：HH:MM:SS → HH:MM:SS.d 十分秒位**（计划书 §4 偏差，本附录为权威定案）+ tick 100ms + DTO `total_secs`→`total_ms`；轮 3 = 用户确认通过
> - **U2**：cargo test 11/11（状态机 7 + 命令层 4）
> - **陷阱**：运行中的 exe 锁文件致 cargo build 报 os error 5——重建前先关运行实例（已登记 AGENTS.md 陷阱清单）

> **PL001 收口结论（2026-09-08，全组完结）**
>
> - **验收标准逐条核对**：①玻璃判定书面结论 ✅（全过不降级）②S1–S3 全绿 ✅ ③U1/U2 ✅（三轮）④门禁四件套全绿 ✅ ⑤文档回写 ✅（本附录 + x.progress 勾结 + README/AGENTS 状态行）
> - **最终形态**：透明 Acrylic 窗口（无边框 360×480）+ 玻璃卡片（毛玻璃/深浅色跟随/可拖动）+ 大计时器（HH:MM:SS.d）+ 开始/暂停/继续/重开；状态机与命令层纯 Rust 可测（11/11），前端零业务
> - **全组 15 条勾结**（含 PL001.10a lib 骨架、PL001.10b 目录风格改造两条阶段内决策）
> - **门禁基线**：cargo fmt --check / clippy -D warnings / test 11 / doc 0 告警 / npm build（vue-tsc + vite）
> - **遗留**：无阻塞项；100ms 拉取若未来需更高实时性可评估 Rust 推送（tick 拉制定案不变）

#### 阶段 B：玻璃可行性打样（★ 判定点，先于一切 UI 功能）

- **透明 + Acrylic**：window-vibrancy `apply_acrylic`（`#[cfg(windows)]` 分支），tauri.conf `transparent: true` + `decorations: false`；验证 G1
- **玻璃卡片 + 深浅色**：卡片半透明底 + `backdrop-filter: blur()` 叠加系统级模糊；`prefers-color-scheme` 双主题跟随；验证 G2/G3
- **无边框拖动**：`data-tauri-drag-region` 拖动区覆盖 + 固定 360×480（窗口缩放留后期打磨）；验证 G4
- **判定书**：G1–G4 逐项结论；**判死条件**——Acrylic 在本机不可用或闪烁不可接受 → 降级"半透明纯色"方案并重议产品形态，书面结论回写本附录

#### 阶段 C：计时状态机（TDD，时间源注入）

- **`src-tauri/src/core/session.rs`**：`trait Clock { fn now(&self) -> Instant }`（测试注入手拨假钟）+ 三态 `Idle / Running { start } / Paused { elapsed }`（`enum SessionState` 或 `WorkSession` 持态——实现时取简）；`SessionError`（thiserror）
- **四操作**：`start`（仅 Idle 合法）/ `pause`（仅 Running）/ `resume`（仅 Paused，继承累计）/ `total()`（Running 现算、Paused 返累计、Idle 为零）；非法态操作返回 `Result<_, SessionError>`——严格抛错主线，不静默忽略
- **边界用例**：Idle 下 pause/resume 报错、Running 下重复 start 报错、多轮 start-pause-resume 循环累计正确、零时长会话、假钟任意拨动无 panic、零真实 sleep

#### 阶段 D：UI 接线（最薄闭环）

- **命令层 `commands.rs`**：`session_start / pause / resume / status` 四命令——会话存 `Mutex<WorkSession>`（tauri State），status 返回 `{ state, total_secs }`；状态逻辑可脱离窗口单测
- **前端 `TimerCard.vue`**：大计时器等宽数字（HH:MM:SS）+ 按钮状态切换（开始 ↔ 暂停/继续）；setInterval 1s invoke status，组件卸载清理
- **`App.vue`**：玻璃卡片布局（计时器 + 按钮），拖动区沿用阶段 B 定案

### 验证方案（全部可执行、可断言）

| #   | 层级   | 检验内容       | 手段与通过标准                                                                     |
| --- | ------ | -------------- | ---------------------------------------------------------------------------------- |
| G1  | 玻璃   | 透明 + Acrylic | 窗口透明生效、无黑底、无可接受度以下的闪烁（人工，桌面背景透过可见）               |
| G2  | 玻璃   | 卡片模糊叠加   | backdrop-filter 生效，卡片后内容呈毛玻璃观感（人工）                               |
| G3  | 玻璃   | 深浅色跟随     | 切换系统主题，卡片与文字双主题清晰可读（人工）                                     |
| G4  | 玻璃   | 无边框拖动     | 按住卡片区域拖动流畅、无文字选中副作用、窗口 360×480 固定（人工）                  |
| S1  | 状态机 | 四操作合法路径 | TDD 先 FAIL 后 PASS；多轮 start-pause-resume 循环累计与预期一致                    |
| S2  | 状态机 | 非法态严格报错 | Idle 下 pause/resume、Running 下重复 start 返回 Err；错误类型明确，不吞不降级      |
| S3  | 状态机 | 时间源注入     | 假钟拨动验证 resume 继承累计、total() 现算语义；测试零真实 sleep                   |
| U1  | UI     | 完整计时流程   | 开始 → 数字每秒走；暂停 → 停走；继续 → 续走；暂停态重开 → 归零（人工）             |
| U2  | UI     | 命令层可测     | commands 状态逻辑在 cargo test 下覆盖（不经窗口、不依赖前端）                      |
| —   | 门禁   | 四件套全绿     | cargo fmt --check + clippy -D warnings + test + npm build（含 prettier / vue-tsc） |

### 验收标准

1. GB 玻璃判定有书面结论：G1–G4 全过，或判死后有降级方案结论
2. S1–S3 全绿：状态机零散落 unwrap、时间源注入、严格抛错
3. U1/U2 过：窗口内完整计时流程可用
4. 门禁四件套全绿
5. 收尾：实测结论回写本附录、x.progress.md 勾结、README/AGENTS 状态行回改

### 明确不做（YAGNI 边界）

- SQLite/统计聚合（今日/本周/累计）→ PL002
- 提醒调度（阈值/通知/声音）→ PL003
- 托盘/全局快捷键/关闭最小化 → PL002 或 PL004 视进度
- macOS vibrancy / Linux blur 适配 → Windows 版成熟后（[problems#1]）

### 拆分 todo

见 x.progress.md「PL001」任务组（13 条子任务，按 阶段 A/B/C/D 四小节分层，每条含做法与验证方式）。

---

## 附录 PL002：存储与统计聚合（2026-09-09 立项）

> 背景：PL001 收口后计时活在内存里，关掉 app 数据归零——产品完整性最大缺口。计划书 §2.2 数据模型已有定案（单表 sessions、暂停时落库、今日/本周/累计 SUM），设计风险低，正适合做下一刀垂直切片：存储层 → 时间边界聚合 → 命令接线 → StatsCard 统计行 UI（复刻 PL001 的打穿方法论）。
> 关键洞察：① 本次唯一"有陷阱"的逻辑是时间边界（本地时区 + 跨零点 + 跨周），全部走注入时间的纯函数测试（AGENTS 陷阱清单既有要求）；② 落库取数源 = 暂停时的"本段时长"，状态机 pause() 演进为返回段时长即可，session.rs 纯度不破；③ 重开语义升级为"先落库再重开"（数据不丢原则，用户拍板）。
> 目标：收口时"计时 → 暂停 → 统计行更新 → 重启 app 数据仍在"端到端成立，跨零点/跨周边界用例全绿。
> 状态：✅ 已完成（2026-09-09 收口，T1–T3/U1 全过；任务清单见 x.progress.md「PL002」；实测结论见下）

### 方向定案（2026-09-09，用户拍板）

1. **范围 = 存储+统计闭环**：SQLite 落库 + 今日/本周/累计聚合 + 统计行 UI；提醒调度排 PL003、托盘常驻排 PL004
2. **本周 = 周一起自然周**（与 ISO 8601 一致）
3. **重开语义 = 先落库再重开**：Running 态重开时当前段落库再归零（"重开"= 结束本段并立刻开新段，数据不丢）
4. **tick 架构沿用**：前端拉取、Rust 唯一时间权威；本地时区经 chrono，边界核心逻辑以本地朴素时间入参保持机器时区无关可测

### 实现措施（按层拆解到文件/函数级）

#### 阶段 A：依赖与时间边界（TDD）

- **依赖接入**：`rusqlite`（bundled feature，SQLite 编译进二进制免系统 dll）、`chrono`（本地时区）、`dirs`（用户目录，AGENTS 路径处理要求）；首次编译变长如实记录
- **`core/src/period.rs`**：今日起点/本周起点纯函数——入参本地朴素时间（NaiveDateTime），出参 Unix 秒；本周起点 = 周一 00:00:00；生产入口薄函数以 `Local::now().naive_local()` 转换，核心逻辑机器时区无关可测
- 边界用例：跨零点、周日 23:59:59 → 周一 00:00:00 翻转、周内各天、恰在边界值 00:00:00

#### 阶段 B：存储层 Repository（TDD，内存 db）

- **`core/src/storage.rs`**：`Storage`——`open(path)`（运行时）/ `open_in_memory()`（测试）；建表按计划书 §2.2 schema：`sessions(id INTEGER PRIMARY KEY AUTOINCREMENT, started_at INTEGER NOT NULL, seconds INTEGER NOT NULL)`
- `add_session(started_at, seconds)`：参数绑定，禁 SQL 拼接（审计维度 7 锚点）
- 聚合：`today_total / week_total / all_total`——SUM(seconds) WHERE started_at ≥ 边界（all 无条件）；`StorageError`（thiserror：Sqlite 透传）
- 全部用例跑内存 db，零用户数据写入（红线）

#### 阶段 C：命令层接线

- **pause() 返回段时长**：`WorkSession::pause()` 演进为 `Result<Duration>`（本段 = 落库取数源；resume 重挂起点使段语义精确），既有用例同步演进（演进式 TDD）
- **AppContext 扩展**：现 SessionHandle 扩为 session + storage 双成员（或并列 managed state，实现时取简）；pause 命令：段 > 0 才落库（零秒段跳过不写噪音行），`started_at = wall_now - 段秒`（SystemTime 在命令层取，session.rs 纯度不破）
- **restart 先落库**：Running → 先 pause 落本段；Paused/Idle → 无未落库段；再 reset + start（单锁原子保持）
- **`session_stats` 命令**：返回 today/week/all 三值秒数
- **运行时路径**：`~/.capsule-pulse/pulse.db`（dirs 解析 + 目录自建）；打开失败严格报错、启动失败（错误策略主线，不登记容错）；测试一律注入路径/内存库，禁触真实用户目录（红线）

#### 阶段 D：统计行 UI 与收口

- **StatsCard 统计行**：`今日 Xh Ym ｜ 本周 Xh Ym ｜ 累计 Xh Ym` 三值（计划书 §1 产品定义含历史累计，覆盖 §4 mock 的两值形态）；玻璃样式沿用 B 阶段定案
- 刷新时机 = 挂载 + 每次动作后 + 30s 兜底轮询（统计非实时数据，不进 100ms tick）
- 收口：门禁全绿 + 结论回写 + 状态行 + 勾结

### 验证方案（全部可执行、可断言）

| #   | 层级     | 检验内容                      | 手段与通过标准                                                             |
| --- | -------- | ----------------------------- | -------------------------------------------------------------------------- |
| T1  | 时间边界 | 今日/本周起点                 | 注入朴素时间用例先 FAIL 后 PASS：跨零点、周日→周一翻转、边界值恰等         |
| T2  | 存储聚合 | 插入/聚合/边界排除            | 内存 db 用例：插入→往返、跨零点/跨周排除、多段求和、空表为零               |
| T3  | 命令接线 | pause 落库/restart 落库/stats | 内存 Storage 注入用例：pause→库内一行、Running 重开→段入库、stats 三值正确 |
| U1  | 闭环人工 | 持久化端到端                  | 计时→暂停→统计行增长；重开→段计入；重启 app→统计仍在                       |
| —   | 门禁     | 四件套全绿                    | cargo fmt --check / clippy -D warnings / test / npm build（含 vue-tsc）    |

### 验收标准

1. T1–T3 全绿（含跨零点/跨周边界）
2. U1 人工闭环通过（含 app 重启持久性）
3. 重开落库语义按定案执行（数据不丢）
4. 门禁全绿 + 实测结论回写本附录 + 状态行同步

### 明确不做（YAGNI 边界）

- 提醒调度/设置持久化 → PL003；托盘/全局快捷键/关闭最小化 → PL004
- 历史记录页面、数据备份、图表 → 远期
- 运行中崩溃/关机丢当前段的恢复提示（计划书 §8）→ 远期（§2.2 已接受最多丢当前段）

### 拆分 todo

见 x.progress.md「PL002」任务组（13 条子任务，按 阶段 A/B/C/D 四小节分层，每条含做法与验证方式）。

> **阶段开展结论（2026-09-09，PL002.1–2.12 完成，TDD 红→绿 ×3 + U1 人工验收一次通过）**
>
> - **TDD 轨迹**：period（E0425 红→绿）/ storage（红→绿）/ commands 重写 + session 演进（编译红→绿）三批；终态 24 项测试全绿（lib 23 + 探针 1），零真实时间、零真实用户数据
> - **关键实现决策四笔**：① WorkSession 内部演进为"本段起点 + 段累计"分离——pause() 返回本段时长（落库取数源），SessionState 形状与对外语义不变；② rusqlite Connection 非 Sync → Storage 入 Mutex 解 tauri manage 的 Send+Sync（锁序恒 session→storage 单向，无死锁面）；③ started_at = wall_now − 段秒在命令层推导，SystemTime 不进 session.rs；④ 零秒段跳过不写噪音行
> - **chrono 0.4.45 API 校正**：num_days_from_monday 在 Weekday 枚举上（now.weekday().num_days_from_monday()），Datelike 直调已移除
> - **U1 人工六项一次通过**：统计行（首启自建库）/ 暂停落库即增 / 多段累计 / 重开先落库 / 重启 app 持久性 / 玻璃无退化
> - **过程违规自纠一处**：中途误用 python 脚本改源码（违反 AGENTS 文件修改规则），当场改回 edit 通道；一处测试断言语义写错由测试失败暴露后修正
> - **依赖**：rusqlite 0.40（bundled）/ chrono 0.4.45 / dirs 7.0（@tauri-apps/api 2.11.1 已于 PL001 期间入账）

> **PL002 收口结论（2026-09-09，全组完结）**
>
> - **验收标准逐条**：T1–T3 全绿 ✅ / U1 六项 ✅（含重启持久性）/ 重开落库语义按定案 ✅ / 门禁 + 回写 ✅
> - **最终形态**：暂停/重开即落库（零秒段跳过）；统计行 `今日｜本周｜累计`；数据落 `~/.capsule-pulse/pulse.db`；统计刷新 = 挂载 + 动作后 + 30s 兜底
> - **13 条勾结**；门禁基线：fmt --check / clippy -D warnings / test 24 / doc 0 告警 / npm build
> - **遗留**：无阻塞项；统计行目前不含当日运行中段的实时增量（暂停才落库，§2.2 定案的自然结果），如有需要 PL003+ 可在快照命令里并入 session.total()
