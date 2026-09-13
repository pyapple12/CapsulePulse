# 项目方案与审计归档（z.plan.md）

> 文件职责：方案文档与审计归档。与 `x.progress.md`（任务清单）、`CapsulePulse_plan.md`（总体规划）分工：总体规划不动，方案演进与审计记录都落在本文件。
> 结构：一、已完成 ✅ → 二、待完成 → 三、主题规划（按需）→ 四、审计观察项豁免定案清单 → 附录 PL{NNN}（专题方案，立项时创建）→ 附录 A{NNN}（审计报告，由 audit-report 归档环节生成）。

## 一、已完成 ✅

- **PL011 焦点联动材质·平时透明聚焦磨砂**（2026-09-13 收口，V0.1.1.1；平时 alpha 透明常驻 + 聚焦 Acrylic 磨砂 + 分态纱浓度）→ 附录 PL011
- **PL010 材质重构·真实玻璃**（2026-09-13 收口，V0.1.0.14；路线三易，最终 = DWM 系统背板聚焦真磨砂 + 全窗单层 + 拖拽修复）→ 附录 PL010
- **PL009 光与生命感**（2026-09-13 收口，V0.1.0.13）→ 附录 PL009
- **PL008 布局翻新**（2026-09-13 收口，V0.1.0.12）→ 附录 PL008
- **PL007 糖果玻璃材质层**（2026-09-13 收口，V0.1.0.11）→ 附录 PL007
- **PL006 苹果玻璃 UI 重设计·Liquid Glass**（2026-09-12 收口，V0.1.0.9）→ 附录 PL006
- **PL005 工作日/打卡模型与统计视图**（2026-09-10 收口，V0.1.0.8）→ 附录 PL005
- **PL004 托盘常驻与全局快捷键**（2026-09-10 收口，V0.1.0.7）→ 附录 PL004
- **FIX001 第 1 轮审计修复**（2026-09-10 收口，V0.1.0.5）→ 附录 A001（勾结见 x.progress.md）
- **A001 第 1 轮全量代码审计**（2026-09-09 归档，首轮）→ 附录 A001
- **PL003 提醒调度与设置持久化**（2026-09-09 收口）→ 附录 PL003
- **PL002 存储与统计聚合**（2026-09-09 收口）→ 附录 PL002
- **PL001 玻璃壳与最小计时闭环**（2026-09-08 收口）→ 附录 PL001

## 二、待完成

（当前无立项任务；下一步方向由用户定）

（UI 三连、PL010 材质返工与 PL011 焦点联动材质均已收口；打包分发/三端适配见计划书 Phase 6/7，未立项）

## 三、主题规划

- （按需）跨附录主题出现时在此登记；远期与遗留项见 y.problems.md

## 四、审计观察项豁免定案清单

> 豁免唯一权威源：已定案项审计时（audit-project）不再重复报告。新定案条目由归档环节（audit-report）经用户确认后追加。分级：①**永久豁免**——不再报告不再讨论；②**条件豁免**——标注触发条件，条件变化时重新评估。
> 2026-09-13 全量整理（A003 收口后）：三轮审计（A001/A002/A003）观察项去重归并于此——失效项销项（A001-O3 window-vibrancy 随依赖移除；restart 双锁窗口随 FIX003.9 消除；eprintln 半收敛已完成）、重复项归并（A002/A003 对 O4/O6 的重复报告删除），三份审计报告中的观察项节同步删除（原位置留归并指引）。整理时逐项重审定案，含 A002"其余"合并行的 4 小项与托盘 hide 焦点语义。

### ① 永久豁免（21 项）

**A001 定案维持（3 项）：**

1. **存储探针保留**（来源 A001-O4，2026-09-09 定案）：`core/tests/storage_probe.rs` 为 rusqlite bundled 工具链冒烟资产，定案保留，去留悬置就此了结；仅当 rusqlite 依赖移除时随之退役。
2. **configs/ 目录语义**（来源 A001-O5，2026-09-09 定案；**2026-09-10 翻案重订**）：原案"用户参数落 ~/.capsule-pulse/、configs 留给固定参数"已废止——用户拍板杜绝机器用户目录，运行时数据双落址：config.json 落 configs/、pulse.db 落 data/（dev=项目根 / release=exe 同级，2026-09-10 热更新定案）。
3. **提醒重发间隔 5 分钟硬编码**（来源 A001-O6，2026-09-09 定案；A003 重复报告已归并）：`ReminderConfig::from_minutes` 内 repeat=300s 为 2026-09-09 用户定案值，有注释依据、无外部调参场景；远期若做多档位/可配置重发属新需求设计，非审计问题。

**2026-09-13 整理定案（18 项）：**

4. **DWM 属性魔数内联**（A003）：DWMWA=38、DWMSBT=3/1 无官方 Rust 绑定可引（引 windows crate 违反零新依赖定案），extern 直连行内注释有依据。
5. **未知菜单 id 静默 + 窗口缺失分支静默**（A002→A003）：菜单 id 闭环构造、主窗口 setup 硬校验且从不销毁——架构保证不可达的防御分支。
6. **clock_in 先 reset 后写库**（A002→A003）：Off 态 session 恒 Idle，reset 幂等空操作，注释已声明语义。
7. **冗余 clock_out 双计**（A002→A003，workday.rs:212）：仅外部改库可达，状态机 + 锁原子化永久保证应用内不可达。
8. **提醒文案双端独立维护**（A002→A003）：双端语义耦合，收敛成本大于漂移风险（YAGNI）。
9. **快速翻日请求乱序覆盖**（A002→A003，StatsView）：本地 IPC 毫秒回程 + 手速限制，架构性低概率。
10. **TimerCard 100ms tick**（A002 维持）：U1 三轮验收定案（十分秒位显示语义）。
11. **.arrow 第三按钮变体**（A003，StatsView）：有意保留的视觉中性小件设计决策。
12. **refreshKey/offset 同 tick 双拉**（A002→A003）：watch 双源幂等读命令，同 #9 低概率无害。
13. **pointermove 每帧 querySelectorAll**（A003）：rAF 节流 + 元素 ≤10，成本可忽略。
14. **session_stats 失败静默冻结旧值**（A003）：有意降级——轮询场景冻结旧值优于报错打断，注释声明。
15. **DockNav role="tab" 无 tablist 父级**（A003）：桌面小工具两页签、button 键盘可达，a11y 收益低（YAGNI）。
16. **`saturate(1.6)` 配方遗产**（A003，--glass-blur）：浮层虹彩色彩增强仍有视觉作用，非死配置。
17. **托盘 hide 时 Focused(false) 触发语义未实证**（A003）：PL011 用户目验已含日常显隐路径，材质终态正确自愈，中间态无观察者。
18. **vite envPrefix 前瞻键**（A002"其余"拆分）：前瞻配置有意保留。
19. **`:key` 索引键**（A002"其余"拆分）：v-for 列表（blocks/week）静态无重排，索引键无缺陷。
20. **`v-model.number` 空串**（A002"其余"拆分）：Rust 侧校验兜底（1–240 / 1–72 严格夹取）。
21. **命令命名三风格并存**（A002"其余"拆分）：改名连带全量 invoke + generate_handler，高风险低收益；破坏性 API 变更时机（若有）另议。

### ② 条件豁免（13 项，按触发条件分组）

**触发 = 打包分发（Phase 6，一组销 5 项）：**

1. **capability 偏宽**（A002→A003，default.json:5）：notification:default 前端零消费（通知走 Rust 侧不经 ACL）+ core:default 范围；打包期随能力清单收窄。
2. **未配 CSP**（A002→A003，tauri.conf.json）：本地内嵌资产无远程内容；打包期配置。
3. **窗口 label 依赖默认值 "main"**（A002→A003）：代码 3 处 + capabilities 1 处隐式引用；打包期显式化。
4. **main.rs 无 windows_subsystem**（A003）：release 构建弹控制台黑窗；打包期添加（diag.rs:4 已登记评估点），落地后关键失败路径已由 warn_diag 双写兜底。
5. **diag 日志无轮转/上限**（A003）：写入点全为低频失败路径；打包期随长期运行场景一并评估。

**触发 = 跨平台适配（[problems#1]，2 项）：**

6. **周边界 DST 偏差**（A001-O1，3 处同根：period.rs week_start_secs / commands/workday.rs day_bounds 固定 86400 / StatsView dayLabel 固定 86400000ms）：含 DST 切换时区有 1 小时偏差；跨平台立项或面向 DST 时区分发前补注入时区用例（需验证）。
7. **前端 listen 卸载竞态**（A001-O7）：根组件生命周期 = 应用生命周期，当前无可达路径；App.vue 不再是永不卸载的根组件时重新评估（需验证）。

**触发 = 性能证据（db 体积/查询耗时异常，3 项）：**

8. **events 无索引**（A001-O2）：`total_since` 全表扫在"一行/暂停"写入速率下成本可忽略（计划书 §2.2 v1 SUM 定案）。
9. **week_detail 逐日取锁**（A002→A003）：7 天 14 次取锁 + 7 次查询，本地 SQLite 毫秒级、翻页触发低频。
10. **浮层双层 backdrop-filter 并存**（A002"4 层"表述已随 PL010/011 架构更新 → A003 现状 2 层）：短暂低频态、240px 小面积。

**触发 = 需求演进 / 理论缺口（3 项）：**

11. **跨零点段归属起点日 + as_secs 秒级截断**（A001-O8）：单表 `(started_at, seconds)` schema 固有语义（段不拆分）；引入"跨零点段拆分"需求时重评数据模型。
12. **原子写无 fsync**（A002→A003，settings.rs:82）：断电窗口 rename 后可能旧内容，纯理论缺口；产品定位升级为关键数据可靠性时重评。
13. **pause 回滚窗口与 async 自动下班理论竞态**（A003，commands/session.rs:93-110）：需存储失败 + 操作同毫秒多重条件交错；错账实际复现时重评（需验证）。

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

---

## 附录 PL003：提醒调度与设置持久化（2026-09-09 立项）

> 背景：计时与统计闭环已立（PL001/PL002），产品核心功能只剩提醒——"连续工作达阈值 → 声音 + 系统通知"（计划书 §2.3）。阈值/开关的可配置与持久化同属本期，否则提醒只是写死的常量。
> 关键洞察：① "连续工作"取自状态机的当前运行段（PL002 段/累计分离的现成红利，pause 即休息重置）；② 判定逻辑 = 纯函数评估器（段时长 + 设置 + 触发状态注入），既有 100ms tick 拉取顺路评估，架构零新增；③ 通知/声音降级互不依赖是 AGENTS 容错白名单的预告首项，本期正式登记。
> 目标：收口时"改阈值 → 短段触发 → 通知 + 声音 + 卡片文案齐发；关声音仅通知、关通知仅声音；重启设置仍在"端到端成立。
> 状态：✅ 已完成（2026-09-09 收口，N1/N2/T4a/U1/R1 全过；任务清单见 x.progress.md「PL003」；实测结论见下）

### 方向定案（2026-09-09，用户拍板）

1. **连续工作 = 当前运行段**：start/继续起算，暂停即视为休息、计数清零（KISS 语义；"短休不清零"的强连续语义远期再议）
2. **重发 = 达阈值后每 +5 分钟**：理睬与否皆继续工作即重发催促；暂停/重开即重置
3. **设置面板 = ⚙ 展开式**：阈值输入（1–240 分钟）+ 声音/通知开关，主界面保持极简
4. **提示音 = 程序生成**：脚本产出约 0.5 秒双音 WAV 入 `ui/public/`，无版权无依赖随包分发

### 实现措施（按层拆解到文件/函数级）

#### 阶段 A：设置持久化 + 提示音素材（TDD）

- **依赖/素材**：`tauri-plugin-notification` 插件接入；`.temp/gen-chime.mjs` 生成 `ui/public/chime.wav`（44.1kHz 16bit 单声道双音正弦 + 淡出）
- **`core/src/settings.rs`**：`ReminderSettings`（threshold_min=50 / sound_enabled=true / notify_enabled=true，serde default）+ `load(path)/save(path)`（JSON）；`SettingsError`（thiserror：Json/Io）
- 用例：文件不存在 → 返回默认（首启语义，容错白名单登记）；往返一致；损坏 JSON 严格报错；非法阈值（0）拒绝；全部临时目录（红线）

#### 阶段 B：提醒评估器（TDD）

- **`core/src/reminder.rs`**：`ReminderConfig`（threshold/repeat，自设置换算）+ `ReminderFire`（段内上次触发点）+ `evaluate(segment, config, fire) -> bool` + `clear()`
- 触发条件无减法实现（`segment >= threshold && (last.is_none() || segment >= last + repeat)`），规避段回退下溢；`clear()` 于暂停/开始/重开时调用（暂停即重置）
- 用例：未达阈值不触发、恰达阈值触发、触发后 4 分钟不发 5 分钟整发、暂停重置后新段再达阈值再触发、旧触发点跨段不误触发

#### 阶段 C：接线

- **`WorkSession::segment_secs()`**：当前段时长访问器（Running = now − start，否则零）——PL002 段分离现成红利，纯函数 + 用例
- **AppContext 扩展**：+ `settings: Mutex<ReminderSettings>`、`fire: Mutex<ReminderFire>`；`get_settings / set_settings` 命令（set 持久化 JSON + 更新内存态）
- **status 集成评估**：`session_status` 顺路 evaluate；触发 → emit `reminder-due` 事件（前端播声/显文案）+ 系统通知（notify_enabled 门控，tauri-plugin-notification 直发）；capabilities 增 `notification:default`（ACL 主动预防，系列已两例静默拒教训）；lib.rs 注册插件 + 启动加载设置（NotFound → 默认，其他错误启动退出）
- **容错白名单登记（首项正式落地）**：通知发送失败降级仅声音（错误落日志，事件照发）——场景/降级/理由三要素登记 AGENTS
- **前端**：`listen("reminder-due")`（注册失败 .catch 可见——CapsuleRetro 教训）→ `<audio>` 播 chime.wav（sound_enabled 门控）+ 卡片提醒文案条

#### 阶段 D：设置 UI 与收口

- **⚙ 设置面板**：展开式（阈值 number input 1–240、声音/通知 toggle）→ set_settings 即时生效；提醒文案条（阈值触达后显示"已连续工作 N 分钟，休息一下吧"）
- **降级三态人工验证**：默认双通 / 关声音仅通知 / 关通知仅声音（都关 → 仅文案）
- 收口：门禁全绿 + 结论回写 + 状态行 + 勾结

### 验证方案（全部可执行、可断言）

| #   | 层级       | 检验内容                    | 手段与通过标准                                                       |
| --- | ---------- | --------------------------- | -------------------------------------------------------------------- |
| N1  | 提醒评估   | 阈值/重发/重置语义          | 纯函数用例先 FAIL 后 PASS（未达/恰达/重发间隔/跨段不误触发）         |
| N2  | 设置持久化 | 默认/往返/损坏报错/非法拒绝 | 临时路径用例（NotFound → 默认；损坏 JSON 严格报错）                  |
| T4a | 命令接线   | get/set 设置往返            | 临时路径内存态用例                                                   |
| T4b | 触发副作用 | 事件发出 + 系统通知 + 声音  | 薄层 live 验证并入 U1（评估决策已由 N1 纯函数覆盖）                  |
| R1  | 降级三态   | 声音/通知开关独立生效       | 人工三态：双开 / 仅通知 / 仅声音                                     |
| —   | 门禁       | 四件套全绿                  | cargo fmt --check / clippy -D warnings / test / npm build（vue-tsc） |

### 验收标准

1. N1/N2/T4a 全绿（阈值、重发、重置、持久化语义经注入测试）
2. U1：改阈值（如 1 分钟）→ 短段触发 → 通知 + 声音 + 文案齐发；暂停即重置
3. R1 降级三态人工通过；容错白名单两条以上登记完成
4. 门禁全绿 + 实测结论回写本附录 + 状态行同步

### 明确不做（YAGNI 边界）

- 托盘常驻/全局快捷键/关闭最小化 → PL004
- 免打扰时段、"短休不清零"强连续语义、多提醒档位、休息建议内容定制 → 远期
- 提示音可选择/自定义音频 → 远期（本期单音频固定）
- 打包分发（msi/nsis）→ PL004/Phase 5

### 拆分 todo

见 x.progress.md「PL003」任务组（12 条子任务，按 阶段 A/B/C/D 四小节分层，每条含做法与验证方式）。

> **阶段开展结论（2026-09-09，PL003.1–3.11 完成，TDD 红→绿 ×2 + U1 修正轮通过）**
>
> - **TDD 轨迹**：settings（红→绿 4 用例）/ reminder（红→绿 4 用例）；终态 35 项测试全绿（lib 34 + 探针 1），零真实时间、零真实用户数据
> - **关键实现决策三笔**：① 提醒评估与副作用分离——status_snapshot 返回 ReminderDecision（评估纯逻辑，无 AppHandle 即可测），deliver_reminder 薄层执行 emit + 系统通知；② 触发条件无减法比较（segment >= last + repeat）规避段回退下溢，跨段残留有专用用例；③ pause/start/resume/restart 四动作接线 fire.clear()（暂停即重置定案的落点）
> - **素材定案变更（用户拍板）**：提示音由程序生成 WAV 改为用户自备 mp3（`assets/house_alarm-clock_loud.mp3`，文件名不改），经 vite import 打包（哈希进 dist，实证）
> - **两个实测教训**：① **vite publicDir 默认在项目根**——音频放 `ui/public/` 不生效（404 无声），降级白名单路径被意外实测有效；修复 = import 打包；② 未打包 exe 系统通知署名回退 PowerShell（tauri-winrt-notification 官方回退设计），登记 [problems#2] 随 Phase 5 打包解决
> - **容错白名单 +3**：设置缺省回退默认 / 通知失败仅声音 / 声音失败仅通知（预告项正式落地，降级互不依赖）
> - **U1 + R1**：U1 两轮（轮 1 揪出提示音 404 与署名两真问题，轮 2 修复后全过）；R1 降级三态一次通过；设置重启持久

> **PL003 收口结论（2026-09-09，全组完结）**
>
> - **验收标准逐条**：N1/N2/T4a 全绿 ✅ / U1 ✅（修正轮）/ R1 三态 ✅ / 容错登记 ✅（3 条）/ 门禁 + 回写 ✅
> - **最终形态**：⚙ 面板（阈值 1–240 分钟/声音/通知）→ config.json 持久化即时生效；触达阈值 → 系统通知 + 提示音 + 文案条，5 分钟重发，暂停/重开重置；数据/设置均落 `~/.capsule-pulse/`
> - **12 条勾结**；门禁基线：fmt --check / clippy -D warnings / test 35 / doc 0 告警 / npm build
> - **遗留**：通知署名 PowerShell（[problems#2]，随打包解决）；统计行实时增量口子（PL002 遗留，不变）；“短休不清零”强连续语义（远期）

---

## 附录 A001：全量代码审计报告（第1轮，2026-09-09）

> 状态：✅ 已修复（2026-09-10 收口，V0.1.0.5；FIX001 十项勾结见 x.progress.md）
> 范围：core/ 全部 .rs（11 文件 + 探针）+ Cargo.toml + tauri.conf.json + capabilities/ + ui/ 全部前端（6 文件）+ 根配置（package.json / vite.config.ts / tsconfig.json / index.html）+ 静态资源引用核对；不审计 node_modules / dist / target / .temp / .agents / 第三方依赖。
> 方式：18 个程序文件逐行通读 + 门禁工具实测（cargo fmt --check / clippy -D warnings / test 35 / doc 0 告警 / vue-tsc / prettier 全绿）；编号确认 = A001 + FIX001（用户拍板 2026-09-09），8 项观察项全部维持观察不提升 P 级，并定案收录第四节（永久 3 + 条件 5）。

### 零、上轮修复复核清单

无上轮（首轮审计）。

### 一、P0-P3 修复清单（按严重度）

无 P0/P1；1 项 P2、8 项 P3，性质均为新增。

| #    | 文件：行号                                                                             | 类型                        | 描述                                                                                                                                                                                                                                                                                                            | 建议                                                               | 性质 | 影响面     |
| ---- | -------------------------------------------------------------------------------------- | --------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------ | ---- | ---------- |
| P2-1 | core/src/settings.rs:67                                                                | 防御性缺口（维度2）         | `save` 用 `std::fs::write` 直接截断覆盖 config.json——写入中途崩溃/断电产生损坏 JSON，下次启动 `load` 严格报错（lib.rs:42-48 按策略退出），应用无法启动，唯一恢复 = 手删文件；触发概率极低但后果为启动阻断                                                                                                       | 改“同目录临时文件写入 + rename”原子落盘                            | 新增 | 配置体系   |
| P3-1 | core/src/commands/session.rs:49-53                                                     | 错误策略一致性（维度13）    | `clear_reminder_fire` 以 `if let Ok(...)` 静默吞 fire 锁 PoisonError——全项目其余六处锁均严格报错，唯此处静默降级且未登记容错白名单；实际危害小（中毒后 status_snapshot 必然严格报错，故障仍显性化），属合规缺口                                                                                                 | 与其他锁一致改严格报错（若保留静默须先在 AGENTS 白名单登记三要素） | 新增 | 提醒调度   |
| P3-2 | README.md:3,5,17,44-48,58-73                                                           | 文档一致性（维度6）         | ① Version 徽章 0.1.0.1（当前 V0.1.0.4）；② Phase 徽章仍 PL001 完成；③ 状态行“（V0.1.0.2）”与三期全完结矛盾；④ `cargo tauri dev` 应为 `npm run tauri dev` 且“前端热更”失实（未配 devUrl、beforeDevCommand=npm run build）；⑤ 结构树仍规划态（commands.rs 已为 commands/ 目录、缺 period/settings/SettingsPanel） | 随下次提交一次性同步实态                                           | 新增 | 文档       |
| P3-3 | ui/App.vue:94,121；core/src/commands/reminder.rs:22                                    | 默认值双处漂移（维度12/11） | Rust 侧 emit 已随事件携带真实阈值，前端 listen 忽略 payload，文案条退用 `settings?.threshold_min ?? 50`——前端兜底与 Rust 默认（settings.rs:24-26）双处维护，get_settings 首拉失败时文案可能失真                                                                                                                 | listen 回调直读 payload，删前端兜底                                | 新增 | Vue 前端   |
| P3-4 | ui/components/SettingsPanel.vue:26；ui/App.vue:73-81                                   | 防御性反馈缺口（维度2）     | 阈值越界/清空提交后 Rust 严格拒绝，但 catch 仅 console.error——用户无可见反馈（面板不收起、无报错提示）；Rust 校验完备，缺口纯在前端反馈                                                                                                                                                                         | 面板内增加错误提示行                                               | 新增 | Vue 前端   |
| P3-5 | ui/App.vue:15-26；ui/components/TimerCard.vue:5-9；ui/components/SettingsPanel.vue:4-9 | 重复实现（维度4）           | 三个 IPC DTO 的 TS 镜像类型三处重复声明，Rust 加字段需同步多处，漂移风险随文件数放大                                                                                                                                                                                                                            | 收敛 ui/types.ts 单一镜像模块                                      | 新增 | 跨模块     |
| P3-6 | core/src/lib.rs:3                                                                      | 规范违反（维度6）           | 模块注释仍写“后续命令层（commands.rs）”——V0.1.0.4 拆分后已是 commands/ 目录                                                                                                                                                                                                                                     | 注释同步实态                                                       | 新增 | 文档       |
| P3-7 | core/src/commands/reminder.rs:27（调用点 :20）                                         | 死代码（维度5）             | `send_notification` 返回 bool 无消费者                                                                                                                                                                                                                                                                          | 返回值改 `()`（信息已在日志）                                      | 新增 | Tauri 后端 |
| P3-8 | core/src/commands/mod.rs:73-75 及 6 处调用                                             | 重复实现（维度4）           | `lock()` 助手仅覆盖 session 锁，storage/settings/fire 六处内联重复 `.lock().map_err(...)`                                                                                                                                                                                                                       | 增泛型 poison 助手收敛                                             | 新增 | Tauri 后端 |

### 二、亮点

- **错误策略执行干净**：业务代码零 `unwrap()`/`expect()`（全链路 thiserror → CommandError → IPC 文案），TS 零 `any`、invoke 失败至少 console 记录
- **并发纪律明确**：锁序单向（session→storage、settings→fire）无死锁面；“评估+记录”在 fire 锁内原子，同刻并发 tick 不双发
- **可测试性样板**：时钟/存储/路径全可注入，35 项测试零真实等待、零真实用户数据；提醒评估与副作用分离可无窗口直测
- **系列陷阱教训落到代码**：ACL 显式授 notification:default（主动预防）、提示音 import 打包规避 publicDir 陷阱
- **门禁全绿实测**：fmt / clippy -D warnings / test 35 / doc 0 告警 / vue-tsc / prettier；文档计数（35 项测试）与实际一致

> 总评：首轮无正确性缺陷与安全问题，整体质量高于同规模项目均值；1 项 P2（便宜的原子写保险）+ 8 项 P3（文档滞后与前端反馈细节为主）。

---

## 附录 PL004：托盘常驻与全局快捷键（2026-09-10 立项）

> 背景：核心闭环已立（计时/落库/统计/提醒/设置/双落址），日用最大缺口 = **关窗即死**——窗口一关进程退出，运行中的计时直接丢失。计划书 Phase 2 补"后台常驻 + 免聚焦操作"：窗口降级为"界面之一"，计时生命归进程与托盘管。
> 关键洞察：① 计时真相在 Rust 状态机（AppContext），窗口只是拉取式视图——隐藏/节流天然无正确性问题，常驻改造是纯生命周期问题；② 命令层自由函数（start_session/pause_session）是托盘菜单与全局热键的现成动作源，PL002 可测红利直接复用；③ 热键从 Rust 侧 setup 注册，不走前端 IPC，天然避开 ACL 静默拒的系列教训；④ 双落址后 data/pulse.db 为固定文件，双开会撞 SQLite 并发（SQLITE_BUSY）——单实例从"体验项"升级为"正确性项"。
> 目标：**关窗不死、托盘可控、快捷键免聚焦、双开自愈**——窗口隐藏期计时照走、提醒照发，托盘即可控入口。
> 状态：✅ 已完成（2026-09-10 收口，V0.1.0.7；任务勾结见 x.progress.md「PL004」）

### 方向定案（2026-09-10，用户拍板）

1. **范围四项全收**：关闭隐藏到托盘、托盘图标+菜单、全局快捷键、单实例
2. **关闭语义 = 隐藏到托盘**：拦截 CloseRequested → prevent_close + hide；托盘菜单"退出"才真退（app.exit）
3. **退出落库**：托盘退出时若 Running 先落库再退（延续"数据不丢"原则，PL002 重开语义同款）；Paused 段已在最近一次 pause 落库、Idle 无事
4. **默认键位**：Alt+Shift+P = 计时切换（Idle→start / Running→pause / Paused→resume 三态循环），Alt+Shift+S = 窗口显隐切换；键位固定，自定义远期
5. **YAGNI 边界**：开机自启、托盘图标状态变色、快捷键自定义、托盘气泡通知——全部远期

### 实现措施（按阶段拆解）

#### 阶段 A：托盘常驻（关闭不死）

- **依赖/装配**：tauri features 加 `tray-icon`；lib.rs setup 构建 TrayIconBuilder（图标沿用 core/icons/icon.ico 占位，正式图标属打包期）+ 菜单（tauri::menu）：显示/隐藏、开始/暂停、退出
- **关闭拦截**：`on_window_event` 捕获 CloseRequested → `api.prevent_close()` + `window.hide()`
- **菜单事件**：显示/隐藏窗口；开始/暂停按状态机当前态 toggle（复用命令自由函数）；退出 = Running 先落库再 `app.exit(0)`（落库失败 eprintln 记日志仍退出——退出意图优先，错误不静默）

#### 阶段 B：全局快捷键

- **插件**：tauri-plugin-global-shortcut（Cargo + lib.rs 注册）；setup 注册双热键，Rust 侧注册不经前端 IPC、无需新增 ACL
- **动作映射纯函数**：下一动作推导（Idle→start / Running→pause / Paused→resume）脱离 tauri 可测，先 FAIL 后 PASS
- Alt+Shift+P 接映射；Alt+Shift+S 接窗口显隐 toggle

#### 阶段 C：单实例与收口

- tauri-plugin-single-instance（builder 首位注册），二次启动回调 → 唤起主窗口
- 隐藏态提醒三通道实测并入验收；收口回写 + 状态行 + 勾结

### 验证方案（全部可执行、可断言）

| #   | 层级   | 检验内容     | 手段与通过标准                                                |
| --- | ------ | ------------ | ------------------------------------------------------------- |
| T5  | 命令层 | 热键动作映射 | cargo test：Idle→start / Running→pause / Paused→resume 三分支 |
| U1  | live   | 关窗不死     | 关窗后 1 分钟 → 托盘唤起 → 计时连续、统计含该段               |
| U2  | live   | 托盘菜单     | 显隐 / 开始暂停 / 退出逐项过                                  |
| U3  | live   | 全局热键     | 焦点在其它应用时 Alt+Shift+P / S 均生效                       |
| U4  | live   | 隐藏态提醒   | 隐藏 + 短阈值触达：通知照发 + 声音实测 + 唤起后文案条可见     |
| U5  | live   | 单实例       | 二次启动 exe → 唤起已有窗口，无第二进程                       |
| U6  | live   | 退出落库     | Running 态托盘退出 → 重启后统计含该段                         |
| —   | 门禁   | 四件套全绿   | fmt --check / clippy -D warnings / cargo test / npm run build |

### 验收标准

1. T5 全绿；U1–U6 逐项过
2. 关闭语义与退出落库按定案执行
3. 门禁全绿 + 实测结论回写本附录 + x.progress 勾结 + README/AGENTS 状态行

### 明确不做（YAGNI 边界）

- 开机自启（autostart 插件）→ 远期
- 托盘图标随计时状态变色/动画、正式托盘图标 → 打包期/远期
- 快捷键自定义配置、托盘气泡通知 → 远期
- macOS/Linux 托盘适配 → [problems#1]（本期限 Windows 实机验证）

### 拆分 todo

见 x.progress.md「PL004」任务组（7 条子任务全勾）。

> **PL004 收口结论（2026-09-10）**
>
> - **验收标准逐条**：T5 纯函数全绿 ✅（toggle_action 三态映射 + toggle_session 循环 + persist_before_quit 幂等，先红后绿）/ U1 关窗不死 ✅（隐藏 60s+ 唤起，计时 4m43s 连续）/ U2 托盘菜单 ✅（用户人工执行；退出项以 exit 0 + 测试库 223s 落库行数据实证）/ U3 热键 ✅（前台 Alt+Shift+P 实测切暂停恢复；后台真键盘场景留日常使用确认——合成按键无法证明）/ U4 隐藏态提醒 ✅（文案条唤起后在；toast 时点未捕获，归因隐藏期 tick 节流，日常复核）/ U5 单实例 ✅（双开自退 + 唤起）/ U6 退出落库 ✅（223s 段落数据实证）
> - **门禁**：fmt --check / clippy -D warnings / test 41 / doc 0 告警 / npm build 全绿
> - **依赖入账**：tauri tray-icon feature、tauri-plugin-global-shortcut 2、tauri-plugin-single-instance 2
> - **遗留**：热键后台场景与 toast 时点待日常使用自然复核（非缺陷，合成输入测试边界）；托盘图标沿用占位（正式图标属打包期）

---

## 附录 PL005：工作日/打卡模型与统计视图（2026-09-10 立项）

> 背景：核心闭环 + 常驻已立，产品从"计时器"升级为"出勤看板"——实机使用中提出三项增强：统计只有数字没有查看；需要"上班/下班"打卡语义（班内计时 = 工作、空隙 = 休息）；记录每次按下时间点形成时间图谱。对应计划书 Phase 5（2026-09-10 重排后新序）。
> 关键洞察：① 计时器零改动——工作日层是包在现有状态机外的第二层，sessions 表天然就是工作块记录；② events 事件表一表三用（图谱数据源 + 按下时间点留痕 + 衍生统计归约输入），顺带兑现计划书 §8 崩溃留痕远期对策；③ OnDuty 是首个跨重启运行态，workdays.clock_out IS NULL 天然承载；④ 自动下班挂现有 tick 评估口 + 记账回填（发现可迟到、账目准时），零新线程；⑤ 双口径天然兼容旧数据（22m 旧累计无打卡概念也照算）。
> 目标：上班打卡开一天，班内计时 = 工作、空隙 = 休息，下班出报告（在岗/工作/休息三值 + 按下时间点图谱），数据可翻看。
> 状态：✅ 已收口（2026-09-10，V0.1.0.8；任务勾结见 x.progress.md「PL005」，live 验证记录见 .temp/pl005-verification.md）

### 收口结论（2026-09-10）

- 全链路达成：上班打卡（玻璃确认框）→ 班内计时 = 工作块、空隙 = 休息块 → 下班出三值（在岗/工作/休息）+ 时间图谱 + 段明细 + 前后日翻看；未上班计时按钮前后端双保险置灰；重启恢复在岗；自动下班按"上班 + N"回填记账（live 实证 clock_out − clock_in = 3600 整，与发现时刻无关）。
- 落地形态：workdays/events 两表幂等追加（旧库首启自动补表零迁移）；workday.rs 纯逻辑层（状态机 + reduce_day 归约 + auto_out_due，注入时间直测）；commands/workday.rs 三命令 + try_auto_clock_out 挂 session_status 评估口；UI 双标签（v-show 保活 tick）+ 上班 pill + ConfirmModal + StatsView。
- 测试 41 → 77（新增 36：W1 归约 11 + W2 状态机 7 + W3 存储 6 + 设置 2 + 命令层 10）；门禁全绿；live U1–U6 全过（真实用户数据备份/恢复完好）。
- 实现细则对条目的两处扩展：① OnDuty 增携带库行 id（下班关行句柄随态流转免二次查询）；② 自动下班时段末事件随下班一并记回填时刻（保证归约时序一致，sessions 行仍按真实时长落库——双口径互不混淆）。
- 遗留：启动毫秒级竞态窗口（首 tick 即触发自动下班时文案条可能丢失，状态面自愈）登记 y.problems.md；补卡 UI / 周月视图 / 导出仍按"明确不做"延后。

### 方向定案（2026-09-10，用户拍板）

1. **未上班禁用计时按钮**：前端禁用 + Rust 严格拒绝双保险
2. **重启恢复在岗**：OnDuty 持久化（workdays.clock_out IS NULL = 在岗中），启动加载
3. **自动下班**：上班后满 N 小时未手动下班 → 自动结束（默认 8h，⚙ 可调）；**记账回填至上班 + N**（发现可迟到、账目准时）；不弹确认框（触发场景即人不在），文案条告知"已于 XX:XX 自动下班"
4. **双向确认框**：上班/下班均弹玻璃风格自定义 modal
5. **双口径并存**：统计行"今日/本周/累计"自然日口径不动；工作日三值（在岗/工作/休息）仅在统计视图按打卡区间呈现
6. **时间图谱**：events 记录每次按下时间点，视图呈现"几点几分到几点几分"的工休区块 + 段明细 + 前后日翻看

### 实现措施（按阶段拆解）

#### 阶段 A：数据层 + 纯逻辑（TDD）

- storage.rs：init 追加 `workdays`（clock_in/clock_out）与 `events`（at/kind）两表（IF NOT EXISTS 幂等，零迁移成本）+ 写入/查询方法
- 新模块 workday.rs：WorkdayState 状态机（Off/OnDuty；下班时 Running 自动结算该段）+ 事件归约纯函数（events → 在岗/工作/休息 + 图谱区块），复用 Clock 注入
- settings.rs：新增自动下班小时数字段（serde default = 8，旧 config.json 兼容）；⚙ 面板加设置行

#### 阶段 B：打卡接线 + UI

- commands/workday.rs：clock_in / clock_out / day_detail 三命令（day_detail 在 Rust 拼装图谱数据，前端零业务）
- AppContext 加 workday 成员；自动下班挂 session_status 评估口（与提醒评估同构）
- 前端：上班 pill 按钮（按压态）+ 双向玻璃确认框（自定义 modal，不用原生 confirm）+ 未上班计时按钮置灰

#### 阶段 C：统计视图

- App.vue 双标签切换（计时｜统计）；StatsView 组件：时间图谱（横向带，工作亮/休息暗，纯 CSS 不引图表库）+ 三值汇总 + 段明细 + 前后日翻看；types.ts 增 DTO

#### 阶段 D：边界与收口

- 重启恢复在岗 / 自动下班回填 / 跨夜悬置实测；门禁 + 回写

### 验证方案（全部可执行、可断言）

| #   | 层级   | 检验内容                                                    | 手段与通过标准                                       |
| --- | ------ | ----------------------------------------------------------- | ---------------------------------------------------- |
| W1  | 纯逻辑 | 事件归约（在岗/工作/休息/区块）                             | 注入事件流用例（含跨零点、回填、无休息极端）先红后绿 |
| W2  | 纯逻辑 | 状态机转移 + 下班自动结算                                   | 纯函数用例                                           |
| W3  | 数据层 | 重启恢复在岗                                                | clock_out 为空加载用例                               |
| W4  | 命令层 | day_detail 拼装                                             | 内存库 + 注入事件用例                                |
| U   | live   | 打卡按钮按压态/双确认框/置灰联动/图谱渲染/自动下班/重启在岗 | live 人工 + 桌面自动化                               |
| —   | 门禁   | 四件套全绿                                                  | fmt --check / clippy -D warnings / test / npm build  |

### 验收标准

1. W1–W4 全绿（先红后绿）
2. U 系列 live 过：双确认框、按压态、置灰联动、图谱渲染、自动下班回填、重启在岗
3. 统计行口径不受影响（双口径验证）
4. 门禁全绿 + 结论回写本附录 + x.progress 勾结 + README/AGENTS 状态行

### 明确不做（YAGNI 边界）

- 补卡 UI（忘下班的手动修正）→ 远期（自动下班兜底后优先级降低）
- 周视图/月历视图、数据导出 → 远期
- 第三方图表库 → 不引入（纯 CSS/SVG）

### 拆分 todo

见 x.progress.md「PL005」任务组。

---

## 附录 PL006：苹果玻璃 UI 重设计·Liquid Glass（2026-09-11 立项）

> 背景：PL005 功能闭环后用户提出"进化 UI 观感"，点名苹果玻璃设计理念。经 Apple Liquid Glass 官方文档 / HIG Materials / WWDC25 对标后定稿本方案（纯前端，零 Rust 改动、零新依赖）。
> 关键洞察：① 现状的问题不是"玻璃不够"，而是**盒子太多**——面板框/按钮框/标签框都在与数字抢注意力；② Liquid Glass 的结构规则 = 功能层（控件吃玻璃）与内容层（纯排版浮在玻璃上）分离 + 同心几何 + 材质唯一配方，恰好治这个病；③ 计时数字是产品本体，44px 提到 56px 让主角真正站出来。
> 目标：全窗口单一玻璃配方、内容层零盒子零阴影、三档同心圆角、四处操作动效，深浅色自适应，观感对齐 macOS/iOS 玻璃语言。
> 状态：✅ 已收口（2026-09-12，V0.1.0.9；任务勾结见 x.progress.md「PL006」，live 验证记录见 .temp/pl006-verification.md）

### 收口结论（2026-09-12）

- 六项令牌化改造全落地：全局设计令牌（:root CSS 变量）+ 唯一玻璃配方（blur 28 + saturate 1.6 + 高光内描边）+ 三档同心圆角；iOS 分段控件（滑块位移）；56px 展示级主数字；胶囊条图谱与两行三值；确认框/设置统一浮起玻璃片；提醒/自动下班胶囊条。
- 行为零回归实证：invoke 清单 diff 为空，cargo test 77 全绿（Rust 零改动），打卡全链/重启恢复在岗 live 复测通过，v-show 保活红线守住（统计页停留跨阈值提醒条照常出现）。
- G1–G4 全过：双主题（注册表切换实测深/浅两侧截图）、跨午夜图谱对照（昨日工作块裁至 00:00、今日工作 3m + 休息"至今"，与 events 表逐项吻合）、设置 saveError 链路完好。
- 实现补充：切统计页时主动重拉 day_detail（消除动作当秒零长工作块的观感瞬态）。
- 唯一免测项：自动下班绿底条未单独复测（与提醒条同 .reminder 通道同形态，仅 tint 底色不同，PL005 U5 已验通道）。

### 设计立场（Liquid Glass 三原则 → 本项目映射）

| Apple 原则                         | 本项目映射                                                                             |
| ---------------------------------- | -------------------------------------------------------------------------------------- |
| 功能层与内容层分离（玻璃只给控件） | 玻璃只给：pill、分段控件、按钮、弹层、提醒条；数字/三值/图谱不加盒子，纯排版浮在玻璃上 |
| 同心几何（内半径 = 外半径 − 间距） | 卡 R20 → 控件 R13 → 胶囊 R999 三档；弹层独立浮起用 R24                                 |
| 材质唯一、永不混用变体             | 全窗口单一玻璃配方（深浅色换参不换方）；仅有的着色 = 提醒琥珀条 + 自动下班绿条         |

### 设计令牌

色彩（浅色 / 深色）：

- `glass`：白 55% + blur(28px) **saturate(1.6)** / 近黑 #1E1E1E 45% + 同 blur——saturate 是苹果玻璃"比背景鲜活"的关键，现状缺失项
- `ink`：#1D1D1F / #F5F5F7（主文字）；`ink-2`：同色 55% 透明度（次级文字）
- `accent`：#0071E3 / #0A84FF（唯一强调色：工作块、主按钮、在岗态）；无渐变无第二彩色
- `tint-warn`（琥珀 FF9F0A 22% 底，仅提醒条）、`tint-good`（绿 30D158 22% 底，仅自动下班条）
- 语义靠明度：工作 = accent、休息 = ink 12%、无第三色相

字体（单字族系统栈 `"SF Pro Display", "Segoe UI Variable Display", "Segoe UI"`，数字一律 tabular-nums）：Display 56px/600/+1 字距（计时数字）｜Title 15px/600（应用名、弹窗标题）｜Body 13px｜Caption 11px（轴标）。

几何与材质：圆角三档 卡 20 / 控件 13 / 胶囊 999（弹层 24）；玻璃高光两行 = 顶部内侧 1px 白 `rgba(255,255,255,.35)`（深色 .12）+ 全周 0.5px `rgba(255,255,255,.16)`；投影只给浮起元素（pill/弹窗/设置浮层）`0 8px 24px rgba(0,0,0,.18)`，内容层零阴影；按压 = scale(.96) + 高光减弱。

### 布局定案

计时页（默认）：标题 15px/600 即拖拽区；iOS 式分段控件（玻璃胶囊轨道 + 白色滑块位移 220ms）；统计行去卡片改纯文字一行（ink-2）；打卡 pill accent 着色玻璃（在岗 = 内凹发光态）；计时数字 56px Display 无框无底；主按钮 accent 实底胶囊（唯一实底控件）。

统计页：日期导航居中（15px/600 + 28px 圆箭头钮）；图谱改**连续圆角胶囊条**（工作 accent 实底 / 休息 ink 12%，块间 2px 呼吸缝，切日宽度生长 300ms）；三值改两行对仗（标签行 ink-2 + 数值行 ink/600，去"｜"挤排）；明细去底色、hover 微亮 R8、在岗中行尾"至今"。

弹层（确认框/设置共用形态）：居中浮起玻璃片 R24 + 强投影 + 背后 6px 压暗；设置从内嵌卡片改为浮层（与确认框同开合动效 `cubic-bezier(.34,1.56,.64,1)`）；输入框 R8。

动效仅四处且全部回应操作：分段滑块位移 / 按压 scale / 弹层回弹开合 / 图谱切日生长；`prefers-reduced-motion` 全部退化为直切。

### 自我批判记录（排除 AI 生成味默认项）

深底+荧光 accent、渐变玻璃、彩色光晕、SaaS 卡片堆（同圆角同阴影全套盒）、ALL-CAPS 眉标、箭头缀按钮——全部未采用；"大数字"保留因其为产品本体而非排版套路。

### 验证方案（全部可执行、可断言）

| #   | 层级 | 检验内容                                         | 手段与通过标准                                                  |
| --- | ---- | ------------------------------------------------ | --------------------------------------------------------------- |
| G1  | live | 双主题观感（浅色/深色系统切换）                  | 玻璃/文字/强调色两侧均可读，无硬编码纯色残留                    |
| G2  | live | 分段滑块切换 + v-show 保活不回归                 | 统计页停留时计时 tick 仍跑（短阈值提醒条可触发 = 行为红线验证） |
| G3  | live | 按压态/置灰联动/弹层开合                         | 打卡全链路操作观感 + `prefers-reduced-motion` 直切              |
| G4  | live | 图谱/三值/明细新形态                             | 数值与 Rust day_detail 返回一致（口径零变化对照）               |
| —   | 门禁 | prettier / vue-tsc / npm build + Rust 门禁不回归 | 全绿                                                            |

### 验收标准

1. G1–G4 全过 + 门禁全绿
2. 全窗口仅一种玻璃配方；内容层零盒子零阴影；三档同心圆角无第四档
3. 行为零回归：打卡/计时/统计/提醒/自动下班全部照旧（Rust 侧零改动实证）
4. 结论回写本附录 + x.progress 勾结 + README/AGENTS 状态行

### 明确不做（YAGNI 边界）

- three.js/任何 3D 渲染 → 不引入（材质真实感靠 CSS 光照暗示；真 3D 内容需求出现时再议，合成验证已备于 .temp/webgl-probe）
- 图表库/网格背景/彩雾装饰/第二强调色 → 不引入
- 窗口尺寸与布局结构变更、Rust 侧任何改动 → 不做（本方案纯前端观感层）

### 拆分 todo

见 x.progress.md「PL006」任务组。

---

## 附录 A002：全量代码审计报告（第2轮，2026-09-12）

> 状态：✅ 已修复（2026-09-12 收口，V0.1.0.10；FIX002 十五条勾结见 x.progress.md，反向验证与 live 记录见 .temp/fix002-verification.md）
> 范围：core/ 全部 .rs（13 文件）+ Cargo.toml + tauri.conf.json + capabilities/default.json + ui/ 全部前端（10 文件）+ 根配置（package.json / vite.config.ts / tsconfig.json / index.html）+ 静态资源引用；不审计 node_modules / dist / target / .temp / .agents / 第三方依赖与生成代码。
> 方式：三路并行逐行通读（Rust 纯逻辑组 / Tauri 集成组 / 前端组 explore 子任务）+ 主会话回归复核 + 门禁实测（cargo test 77 / fmt --check / clippy -D warnings / vue-tsc / prettier 全绿）。

### 零、上轮修复复核清单（A001 → FIX001，V0.1.0.5）

| 上轮条目                       | 现状                                                   | 证据                   |
| ------------------------------ | ------------------------------------------------------ | ---------------------- |
| P2-1 设置原子写                | ✅ 仍在且完整（rename 失败清理闭环，残留 .tmp 可自愈） | settings.rs:81-92      |
| P3-1 fire 锁严格报错           | ✅ 仍在（回归锚测试在位）                              | session.rs:52、417-428 |
| P3-2 README 五处同步           | ✅ 仍在（随各期持续更新至 V0.1.0.9）                   | README.md:3,5,18,47,69 |
| P3-3 payload 直读零兜底        | ✅ 仍在（`?? 50` 全仓零命中）                          | App.vue:163            |
| P3-4 设置错误提示行            | ✅ 仍在（saveError 链路本轮 live 复测过）              | SettingsPanel.vue:43   |
| P3-5 types.ts 单一来源         | ✅ 仍在（4 组件引用，无本地 DTO 声明）                 | ui/types.ts            |
| P3-6 lib.rs 注释同步           | ✅ 仍在                                                | lib.rs:3               |
| P3-7 send_notification 返回 () | ✅ 仍在                                                | reminder.rs:27         |
| P3-8 poison 泛型助手           | ✅ 仍在（19 处调用收敛）                               | commands/mod.rs:81     |

结论：9/9 在位，零回退、零漏改。既有观察项（A001-O1 DST / O2 无索引 / O8 跨零点段归属 / O4 探针留存）维持原状。

### 一、P0-P3 修复清单（按严重度）

无 P0/P1；P2 一项、P3 十八项，性质均为新增（A001 九项无一复现）。

| #     | 文件:行号                                                                | 类型 | 描述                                                                                                                                                                                                                                                                                                                                             | 建议                                                                                                        | 性质 | 影响面          |
| ----- | ------------------------------------------------------------------------ | ---- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------- | ---- | --------------- |
| P2-1  | commands/session.rs:75-80（同根：65-71、85-90）                          | 1/2  | `pause_session` 次序 = 状态转移→清锁→落库→留痕：`persist_segment` 失败（db 被外部锁/磁盘故障）时状态已 Paused、段时长仅存内存，永不再落库——resume 后只落新段、clock_out 的 reset 丢弃累计，该段永久丢失且不可重试（重试 pause = NotRunning）。与 workday.rs:2"库写先行、状态殿后"定案相反；start/resume 的 mark_segment 失败留孤儿事件为同根变体 | 落库/留痕失败回滚状态机（pause 恢复 Running{start}、start 失败回 Idle、resume 失败回 Paused）；三函数同批修 | 新增 | 会话落库链      |
| P3-1  | App.vue:141 + TimerCard.vue:39                                           | 2    | 打卡/计时命令失败仅 console.error，确认框已先行关闭——用户视角"点了没反应"（并发打卡撞 AlreadyOnDuty、存储故障均可达）                                                                                                                                                                                                                            | 仿 saveError 增轻量错误胶囊条通道                                                                           | 新增 | Vue 前端        |
| P3-2  | App.vue:111                                                              | 1    | 提醒条仅随 TimerCard 动作清除：提醒触发后直接走 pill 下班（不经 TimerCard），提醒条滞留屏幕并可与自动下班绿条堆叠（确定性复现）                                                                                                                                                                                                                  | onConfirmOk 同步清 reminderVisible                                                                          | 新增 | Vue 前端        |
| P3-3  | commands/workday.rs:41-46 + session.rs:75-80                             | 1    | 自动下班链路 SegmentEnd 事件记发现时刻（wall_now），与 PL005 收口定案"段末随下班记回填时刻"及契约测试 workday.rs:563-580 口径脱节（reduce_day 容错吸收、三值无差错，events 留孤儿段末）                                                                                                                                                          | pause_session 增可选记账时刻参数，自动路径传 out_at；或修订定案+测试并注释留痕                              | 新增 | events 数据口径 |
| P3-4  | StatsView.vue:14-20                                                      | 2/13 | day_detail 失败仅 console：首拉失败误显"本日无打卡记录"（错误伪装成空数据）；翻日失败标签与内容错位                                                                                                                                                                                                                                              | 增 error 态区分空日与失败                                                                                   | 新增 | Vue 前端        |
| P3-5  | commands/workday.rs:81                                                   | 2    | `offset: Option<i64>` 无范围校验，极端值（\|offset\|≳9.5e7 天）`Duration::days` 加法 panic；当前前端只传 0/±1，devtools/误用可达（panic 后果需 live 验证）                                                                                                                                                                                       | 夹取 ±366 后走 CommandError 严格报错                                                                        | 新增 | day_detail 命令 |
| P3-6  | App.vue:237 + 57-63                                                      | 2    | `v-if="panelVisible && settings"`：get_settings 首拉失败时 settings 恒 null，⚙ 点击后浮层永不渲染且零反馈                                                                                                                                                                                                                                        | settings 为 null 时点击显错误条或 ⚙ 置灰                                                                    | 新增 | Vue 前端        |
| P3-7  | lib.rs:85                                                                | 13   | 托盘"退出落库失败仍退出"为未登记白名单的容错（白名单仅三项）；叠加 eprintln 在 release 不可见，该数据丢失事件实际零记录                                                                                                                                                                                                                          | 白名单登记第四项三要素（见 P3-8 诊断）                                                                      | 新增 | 错误策略合规    |
| P3-8  | lib.rs 全组 eprintln                                                     | 10   | 诊断全走 eprintln：dev 可见、release 形同虚设（无控制台无日志文件）——含启动失败、退出落库失败等关键记录；未来打包加 `windows_subsystem` 防黑窗则彻底不可见                                                                                                                                                                                       | 轻量文件日志（零依赖 append），至少覆盖启动失败与退出落库失败                                               | 新增 | Tauri 后端      |
| P3-9  | lib.rs:193-205 全部 11 命令                                              | 8/9  | 命令全部同步 = 在主线程执行（Tauri 2 官方语义）：SQLite 写入/SUM 聚合/day_detail 全表扫均跑 UI 事件循环；现数据量无症状，五锁防的"并发"实际不存在（workday.rs:121 TOCTOU 注释当前不可达）                                                                                                                                                        | 读命令（session_stats/day_detail/session_status）改 async 移出主线程；锁序纪律升为承压面前补文档            | 新增 | 线程架构        |
| P3-10 | storage.rs:82-88,123-129,134-143,161-171                                 | 2    | 三处"两笔独立写入"非事务（开班+事件/关班+事件/落段+段末）：两笔间进程崩溃产生永久错值不自愈（如关班无事件→历史日在岗永久虚增）。触发窗口微秒级，需验证（故障注入）                                                                                                                                                                               | storage 增事务方法（unchecked_transaction），上游三处一并                                                   | 新增 | 存储层          |
| P3-11 | SettingsPanel.vue:74-91 + ConfirmModal.vue:71-79 + TimerCard.vue:130-151 | 4    | PL006 令牌化漏网：主按钮三件套（accent 实底/hover/active）逐字复制三处，灰玻璃次按钮两处 + StatsView .arrow 第三变体                                                                                                                                                                                                                             | App.vue 全局层抽 .btn-primary/.btn-ghost                                                                    | 新增 | Vue 前端        |
| P3-12 | ConfirmModal.vue:16-18 + style.css:5                                     | 4/6  | 确认框是 .glass-card 兄弟节点，字体未继承 --font-stack 回落 system-ui，与设置浮层不一致、内部标题/按钮混排                                                                                                                                                                                                                                       | .floating-sheet 全局规则补 font-family 一行                                                                 | 新增 | Vue 前端        |
| P3-13 | App.vue:150 + StatsView.vue:23 + StatsCard.vue:30                        | 4    | 格式化助手重复：hhmm×2（pad×3）、fmt×2 逐字复制                                                                                                                                                                                                                                                                                                  | 收敛 ui/format.ts（pad/hhmm/fmt）                                                                           | 新增 | 跨组件          |
| P3-14 | session.rs:104-109 ↔ workday.rs:52-56                                    | 4    | persist_before_quit 与 close_running_segment 函数体逐字相同（quit 语义变更需双处同步）                                                                                                                                                                                                                                                           | 合一（pub(crate) 复用）                                                                                     | 新增 | 命令层          |
| P3-15 | workday.rs:52-73 + reminder.rs:49-51                                     | 5    | 死代码：clock_out_inner 返回 DutySpan 全部调用点丢弃；ReminderFire::last_fired() 生产零调用（仅测试）                                                                                                                                                                                                                                            | 返回改 ()；last_fired 加 #[cfg(test)]                                                                       | 新增 | 命令层/纯逻辑   |
| P3-16 | settings.rs:14-25,66-76                                                  | 13   | 白名单只登记"文件缺失回退默认"；既有文件缺字段时 serde default 静默补默认（threshold/sound/notify 三字段无登记）属第二类未登记容错，仅手改配置可达                                                                                                                                                                                               | 白名单补登记"字段缺失回退默认"，或去 default 严格报错                                                       | 新增 | 配置体系        |
| P3-17 | paths.rs:59-66                                                           | 10   | dev 恢复测试断言 core/ 目录存在，`cargo test --release` 时 runtime_root 走 exe 分支必失败（需验证，静态推演成立）                                                                                                                                                                                                                                | 按 cfg!(debug_assertions) 分支断言                                                                          | 新增 | 可测试性        |
| P3-18 | README.md:18 + AGENTS.md:5                                               | 6    | 状态行"77 项测试全绿"：全仓实际 78（lib 77 + tests/storage_probe.rs 1，探针随 cargo test 执行），口径未注明（需验证：实跑汇总）                                                                                                                                                                                                                  | 改 78 或注明口径                                                                                            | 新增 | 文档            |

> 观察项已于 2026-09-13 全量整理归并至「四、审计观察项豁免定案清单」（失效销项 + 重复归并 + 逐项重审定案），本报告不再保留平行副本。

### 三、亮点

- A001 九项修复零回退，FIX001 修复质量经两轮交叉验证
- 锁序纪律零违反：三路独立逐函数核对，workday → session → storage/settings/fire 单向无一处反向；生产代码零 unwrap/expect
- PL005/PL006 新增代码质量高：reduce_day 18 用例锚定、跨午夜 live 与 db 逐项吻合；令牌化后 TS 零 any、零空 catch、无 v-html
- 契约三方一致：generate_handler 11 命令、serde 四结构与 types.ts 逐字段对齐、事件名两端一致
- 命令核心逻辑全部可无窗口直测（test_support 模式），测试零真实等待零真实数据

> 总评：两轮对比代码基线显著变厚（打卡模型 + UI 重设计）但无 P0/P1；唯一 P2 是 PL005 接线时副作用次序偏离了自己定案的"库写先行"原则（数据丢失链，失败注入下确定）；P3 以前端反馈缺口（4 项）与合规口径（白名单 2 项）为主，无正确性错值。

---

## 附录 PL007：糖果玻璃材质层（2026-09-12 立项）

> 背景：PL006"苹果克制玻璃"交付后，用户提供 Liquid Glass UI Kit 参考图，期望整体观感趋向参考图的"糖果玻璃拟物"风。经拆解，参考图辨识度 = 五要素配方：轮廓光（顶缘亮白高光 + 彩色边缘泛光）、带色柔投影（元素浮于玻璃之上）、彩色半透明渐变底、虹彩渐变卡（粉→紫→青 pastel）、奶白基底——全部为 CSS 渐变/高光/阴影/模糊四件套，**零 WebGL 依赖**。
> 关键洞察：① PL006 令牌架构使材质升级 = 换令牌值 + 蒙皮类，布局与信息架构零改动；② 参考图不含真实折射/3D 透视成分，three.js 无不可替代价值；③ 糖果玻璃是浅色主打风格，深色做"暗夜霓虹"衍生版。
> 目标：全应用材质与色彩层对齐参考图——紫（主/工作）+ 薄荷（次/休息）pastel 色板、轮廓光、带色投影、虹彩浮层，深浅色同步。
> 状态：✅ 已完成（2026-09-13，V0.1.0.11；门禁全绿 + C4 对比度探针 16/16 通过（渐变描字达标，未启用回退类）+ live 启动冒烟正常；C1 深色观感视觉核验通过、C3 数据对照 live 通过，浅色随 PL008/PL009 双主题环节顺带复核；任务勾结见 x.progress.md「PL007」）

### 方向定案（2026-09-12 用户拍板）

1. 色板：单系统蓝 → pastel 虹彩（紫 = 主操作/工作块、薄荷 = 次操作/休息块、蜜桃/天蓝点缀），令牌名不变只换值与扩充
2. 数字渐变描字（background-clip: text），保留纯色回退类（对比度不达标即回退）
3. 提醒条琥珀保留原色；自动下班条绿 → 薄荷统一色板
4. Acrylic tint 调浅暖紫（238,233,246,130）——本 PL 唯一 Rust 触点（一行常量）
5. 深色主题同步维护："暗夜霓虹"衍生版（深紫灰底 + 降饱和 accent + 加强轮廓光）
6. three.js 不引入（YAGNI；可选实验降级至 PL009 时间盒）

### 实现措施（按阶段拆解）

- 阶段 A 令牌与工具类：App.vue `:root` 新增糖果令牌（--grad-primary 紫罗兰 165deg / --grad-mint / --iridescent 粉紫青 pastel / --edge-glow 彩色边缘泛光 / --shadow-candy 带色投影 / rim-light inset 0 1.5px rgba(255,255,255,.9) / 奶白 --glass-bg）；全局材质类 .glass-panel / .glass-chip / .iridescent；深色衍生版
- 阶段 B Acrylic：lib.rs apply_acrylic tint 常量调整
- 阶段 C 组件蒙皮：pill 与开始/暂停/继续（紫渐变胶囊）、重开/取消（薄荷）、分段滑块白色浮起、图谱玻璃轨道（work 紫渐变 / rest 薄荷半透明）、三值 chips、数字渐变描字、确认框/设置 iridescent 浮层、文案条（琥珀保留/薄荷统一）
- 阶段 D 收口：门禁 + 双主题 + 对比度 + 反向验证 + 回写

### 验证方案（全部可执行、可断言）

| #   | 层级 | 检验内容            | 手段与通过标准                                            |
| --- | ---- | ------------------- | --------------------------------------------------------- |
| C1  | live | 双主题糖果观感      | 深浅两侧截图：rim 光/投影/渐变/虹彩齐备且文字可读         |
| C2  | live | 保活红线 + 全链回归 | dock 前切页不回归；打卡→计时→暂停→下班全链正常            |
| C3  | live | 数据对照            | 统计页三值/图谱与 day_detail 返回一致（材质层不影响数据） |
| C4  | 实测 | 正文对比度          | 浅底深字 ≥ 4.5:1；数字渐变描字不达标即回退纯色            |
| —   | 门禁 | 七项全绿            | fmt/clippy/test/doc + prettier/vue-tsc/build              |

### 验收标准

1. C1–C4 全过 + 门禁全绿
2. 全应用零 WebGL 引用；布局/信息架构与 PL006 收口态一致（diff 仅材质/色彩）
3. 结论回写本附录 + x.progress 勾结 + README/AGENTS 状态行

### 明确不做（YAGNI 边界）

- three.js / 任何 WebGL → 不引入（PL009 时间盒除外）
- 布局/信息架构/窗口几何改动 → 不做（PL008 范围）
- 噪点纹理贴图、第三方图表/图标库 → 不引入

### 拆分 todo

见 x.progress.md「PL007」任务组。

---

## 附录 PL008：布局翻新（2026-09-12 立项）

> 背景：材质三连的第二段——整窗信息架构重排。参考图的"浮起卡片 + dock"语言替换现有单列排布；统计能力升级需为周视图腾出结构空间。本 PL 设计含量最重，**立项细化与执行全程启用 frontend-design skill**（脑暴 → token → 反模板审查 → 实现 → 截图自评）。
> 关键洞察：① 材质已令牌化，布局翻新 = 移动组件 + 新增 dock/环/周卡，蒙皮自动跟随；② 进度环口径 = 当日工作秒 ÷ (自动下班小时 × 3600)——展示计算零 Rust 改动；③ 周视图是唯一新增数据面，需 Rust 新命令 week_detail。
> 目标：380×560 可调窗内完成"计时（进度环主体）/ 统计（卡片化 + 周视图）"双页重排 + 底部 dock 导航。
> 状态：✅ 已完成（2026-09-13，V0.1.0.12；定稿线框 A 环主型·胶囊表盘经评审落地，L1–L5 live 验证过（L1 极值拖拽记部分验证，min 夹取 + 弹性布局保证），门禁 90 项全绿；任务勾结见 x.progress.md「PL008」，验证记录见 .temp/pl008-verification.md）

### 方向定案（2026-09-12 用户拍板）

1. 窗口几何：360×480 → 380×560，**resizable: true**（用户可自由拉伸，布局需弹性适配）
2. 导航：顶部分段 → **底部 dock**（lucide-vue-next 图标库，新增依赖——Timer/ChartColumn 等 ~6 枚）
3. 计时页主体：8h **工作日进度环**（SVG 圆环：糖果玻璃轨道 + 紫渐变进度弧，数字居环中 40px）；未上班环虚线置灰；pill 融合进环主体区
4. 统计页：日导航/图谱/三值/明细**四区卡片化**（浮起玻璃卡）+ **周视图卡**（7 日横向条形，今日高亮）
5. 深色主题同步维护（每组件随做随配暗夜衍生）
6. 设计 skill 全程驱动

### 定稿线框（2026-09-13 评审定稿：A 环主型·胶囊表盘，交互评审通过）

**计时页**（380×560，卡内区 356×536，纵向弹性栈）：

```
┌──────────────────────────────┐
│ CapsulePulse            ⚙   │ 顶栏 26
│ 今日0m｜本周5h34m｜累计5h34m │ 18
│                              │
│        ╭─────────╮          │
│     ╭──╯         ╰──╮       │
│    ╱    00:00:00    ╲       │ 环区 296
│   │   40px 渐变数字   │      │ 环⌀264 stroke12
│    ╲                 ╱       │ 弧=紫渐变(8h口径)
│     ╰──╮         ╭──╯       │ 轨道=玻璃
│        ╰─────────╯          │
│         ┌──────────┐         │
│         │  上 班   │         │ pill 36
│         └──────────┘         │
│    ┌────────┐  ┌────────┐    │
│    │  开始  │  │  重开  │    │ 46
│    └────────┘  └────────┘    │
│ ╭──────────────────────────╮ │
│ │ ⏱ 计时      ▤ 统计       │ │ dock 54
│ ╰──────────────────────────╯ │
└──────────────────────────────┘
```

- 环：外径 264、描边 12；轨道 = 玻璃（内凹轨道感 + rim 光），进度弧 = `--grad-primary`（stroke-dasharray 自 12 点顺时针）；环心数字 40px `--grad-digit` 描字 + tabular-nums；未上班 = 虚线置灰轨道 + 纯色数字
- pill / 按钮行：PL007 蒙皮原样迁入环主体区（pill 36 高、按钮 min-width 112）
- dock：玻璃胶囊条（左右 inset 12、高 54），两项 = Timer 计时 / ChartColumn 统计；激活 = 紫渐变胶囊 + 白 icon + edge-glow，非激活 = ink-2；⚙ 与 ‹› 换 lucide Settings/ChevronLeft/ChevronRight
- **计时页零卡片**：环/pill/按钮全为内容层直接悬于玻璃（大胆只花在环一处）

**统计页**（评审确认与 A 版同构：五卡纵流，卡间 6–8px）：

```
│ 今日0m｜本周5h34m｜累计5h34m │ 18
│ ╔══════════════════════════╗ │
│ ║   ‹    今 日    ›        ║ │ 导航卡 36
│ ╠══════════════════════════╣ │
│ ║ ▓▓▓▓▓░▓▓▓▓░░▓▓▓▓▓▓▓░▓▓▓ ║ │ 图谱卡 74
│ ║ 00:23             08:11  ║ │ (图谱+轴标)
│ ╠══════════════════════════╣ │
│ ║ (在岗)   (工作)   (休息) ║ │ 三值卡 66
│ ╠══════════════════════════╣ │
│ ║ 最近 7 日                 ║ │
│ ║ 一 ▓▓▓▓▓▓░   5h10m       ║ │ 周卡 130
│ ║ …  ▓▓░░      2h04m       ║ │ 7条横条形
│ ║ 今 ▓▓▓▓▓▓▓   紫渐变高亮  ║ │ 条长=占7日峰值
│ ╠══════════════════════════╣ │
│ ║ 00:23–01:00   37m  工作  ║ │ 明细卡 flex≥96
│ ║ …（内部滚动）             ║ │
│ ╚══════════════════════════╝ │
│ ╭──────────────────────────╮ │
│ │ ⏱ 计时      ▤ 统计       │ │ dock 54
│ ╰──────────────────────────╯ │
```

- 周卡：行 = 星期一字符 + 条形 + 时长；条长 = 当日 work_secs ÷ 7 日峰值；今日条紫渐变、其余玻璃底；数据来自 Rust week_detail（PL008.6）
- 明细卡 `flex: 1` + 内部滚动：窗口拉高时明细卡吃掉全部富余高度（弹性适配主承压面）

**令牌补齐**：零新增——环/周卡/dock 全部复用现有糖果令牌（`--grad-primary` / `--grad-digit` / `--glass-bg` / `--glass-highlight` / `--rim-light` / `--edge-glow` / `--shadow-candy` / `--r-pill` / `--r-card` / `--ease-spring`）。

**反模板审查结论**：① 计时页零卡片化、大胆只花在环一处，避开 SaaS 卡片套路；② 统计页五卡按内容定高、节奏各异，周卡唯一紫渐变只给今日条；③ 无眉标/全大写/编号装饰/中点元信息，dock 激活态为功能态；④ 动效零新增（PL009 范围）。

### 实现措施（按阶段拆解）

- 阶段 A 设计：设计 skill 脑暴两版线框（环主型/卡主型）→ 反模板审查 → 定稿线框（ASCII + 尺寸标注落本附录）→ 令牌补齐
- 阶段 B 几何：tauri.conf.json 尺寸 + resizable true；App.vue 弹性布局适配（拉伸不破相）
- 阶段 C dock：新组件 DockNav.vue（lucide 图标 + 文字、激活态糖果高光）；替换分段控件；v-show 保活接线原样迁移（保活红线）
- 阶段 D 进度环：新组件 ProgressRing.vue（SVG stroke-dasharray 进度弧）；数据 = day_detail.work_secs ÷ (get_settings.workday_auto_out_hours × 3600)，前端展示计算；未上班置灰虚线
- 阶段 E 卡片化：统计页四区 .glass-panel 化 + 明细卡内部滚动
- 阶段 F 周视图：commands/workday.rs 新增 week_detail 命令（今日为锚回溯 7 日循环 reduce_day → Vec<{date, work_secs, duty_secs}> DTO + types.ts 镜像）；前端周卡 7 条横向条形（今日高亮紫渐变）；验证含跨周边界用例
- 阶段 G 深色同步 + 阶段 H 收口

### 验证方案（全部可执行、可断言）

| #   | 层级  | 检验内容             | 手段与通过标准                                             |
| --- | ----- | -------------------- | ---------------------------------------------------------- |
| L1  | live  | 弹性布局             | 窗口拉伸至极限尺寸不破相、不溢出；默认 380×560 观感        |
| L2  | live  | dock 切页 + 保活红线 | dock 切换正常；统计页停留跨阈值提醒条仍触发                |
| L3  | live  | 进度环联动           | 打卡前虚线置灰；计时中环体随 work_secs 增长；下班后定格    |
| L4  | cargo | 周视图聚合           | week_detail 用例：跨周边界（周日锚）7 日切片正确、空日为零 |
| L5  | live  | 数据对照             | 周卡 7 条与逐日 day_detail 一致；三值/明细口径不变         |
| —   | 门禁  | 七项全绿             | 同 PL007                                                   |

### 验收标准

1. L1–L5 全过 + 门禁全绿
2. 布局信息架构按定稿线框落地；dock/环/周卡三新件齐备；保活红线守住
3. 结论回写本附录（含定稿线框）+ x.progress 勾结 + README/AGENTS 状态行

### 明确不做（YAGNI 边界）

- 月历视图、数据导出、三区以上导航 → 远期
- three.js 主线引入 → 不做（PL009 时间盒限定）
- 自定义主题色/皮肤系统 → 远期

### 拆分 todo

见 x.progress.md「PL008」任务组。

---

## 附录 PL009：光与生命感（2026-09-12 立项）

> 背景：材质（PL007）与布局（PL008）落定后的收尾段——让糖果玻璃"活"起来：光随指针、环境光呼吸、微交互，并以全形态审计收官。three.js 降级为本期**可选时间盒实验**（真折射原型，不进主线）。
> 目标：界面具备"光的生命感"（指针高光/呼吸/微交互），双主题全形态审计通过，three.js 实验出结论。
> 状态：✅ 已完成（2026-09-13，V0.1.0.13；M1 全过（高光跟随/呼吸/光轨抓拍实证）、M2/M3 调整验证（2026-09-13 用户定案禁止改电脑系统设置——代码走查 + 令牌机制确认替代系统开关实测，已登记 AGENTS）、M4 出结论（60fps/合成正确/不引入主线）；门禁 90 项全绿；任务勾结见 x.progress.md「PL009」，验证记录见 .temp/pl009-verification.md）

### 方向定案（2026-09-12 用户拍板）

1. 指针跟随高光：全局 pointermove 写 CSS 变量 --mx/--my，玻璃面板叠加 radial-gradient 高光层
2. 环境光呼吸：虹彩卡渐变位 8s 缓移；prefers-reduced-motion 全部退静态
3. 微交互：按压涟漪、打卡成功 pill 扫光一次、切页光轨
4. three.js：可选时间盒实验——真折射玻璃卡原型，产出效果/性能结论，**不默认合入主线**
5. 深浅双主题全形态审计（对比度/层次/一致性）收官

### 实现措施（按阶段拆解）

- 阶段 A 指针高光：App.vue pointermove（rAF 节流）写 --mx/--my；材质类叠 highlight 层
- 阶段 B 呼吸与微交互：@keyframes 渐变位缓移；涟漪/扫光/光轨（一次性 animation）
- 阶段 C three.js 时间盒：独立分支原型（折射玻璃卡），帧率 + 合成正确性结论落 z.plan；不合入主线
- 阶段 D 全形态审计 + 收口

### 验证方案（全部可执行、可断言）

| #   | 层级 | 检验内容                  | 手段与通过标准                                   |
| --- | ---- | ------------------------- | ------------------------------------------------ |
| M1  | live | 指针高光/呼吸/微交互      | 高光随指针、呼吸平滑、打卡扫光一次               |
| M2  | live | reduced-motion            | 系统开启后全部退静态                             |
| M3  | live | 双主题全形态              | 深浅两侧审计清单逐项过                           |
| M4  | live | three.js 时间盒（若执行） | 帧率 ≥ 55fps、合成正确、窗口透明无伪影；结论落档 |
| —   | 门禁 | 七项全绿                  | 同 PL007                                         |

### 验收标准

1. M1–M3 全过（M4 出结论即可，不强制合入）+ 门禁全绿
2. 动效全部尊重 reduced-motion；无新增运行时依赖（three.js 仅实验分支）
3. 结论回写本附录 + x.progress 勾结 + README/AGENTS 状态行

### 明确不做（YAGNI 边界）

- three.js 合入主线 → 实验结论支持且用户拍板前不做
- 布局再改动、新功能面 → 不做（纯观感动效层）
- 日志框架、状态管理库等新依赖 → 不引入

### 实验结论（PL009.4，2026-09-13）

- 原型：`.temp/webgl-probe/www/refraction.html`（裸 WebGL 双 pass：程序化背景纹理 → 圆角矩形 SDF 法线偏移折射采样 + RGB 色散 + 菲涅尔边缘 + 顶缘高光；以探针承载替代独立分支，零主线风险）
- 性能：**60 fps**（420×560 透明窗、DPR 缩放下，≥55 达标）
- 合成：卡外 `discard` → alpha 0 透出真实桌面，无伪影（延续 PL006 alpha 探针结论）
- 关键限制：WebView2 页面拿不到桌面像素——WebGL 折射只能作用于画布内合成内容；CSS `backdrop-filter` 则由系统合成器折射**真实**桌面且零依赖
- 去留建议：**three.js/WebGL 不引入主线**。主线的玻璃直接贴着真实桌面，backdrop-filter 天然折射真实内容；WebGL 折射只值一个"假背景"的价。探针保留 `.temp/webgl-probe` 供后续复核

### 拆分 todo

见 x.progress.md「PL009」任务组。

---

## 附录 PL010：材质重构·真实玻璃（2026-09-13 立项）

> 背景：PL009 收口后用户实机审查提出五项问题——①与参考图（Liquid Glass UI Kit）观感差距过大：参考是浅色环境 + 高透玻璃（体色不透明度 0.1–0.25）+ 有厚度的立体件，我们是深色环境 + 72% 不透明近黑面板 + 平面渐变件；②窗口拖拽只剩标题/数字两行可拖（bug：`data-tauri-drag-region` 只在"被点中元素自身"带属性时生效，PL008 弹性布局铺满后 main 无裸区可被点中，属性形同虚设）；③双层结构（72% 暗卡 + 浅暖紫 Acrylic 压深色壁纸成灰泥）灰层无意义；④失焦后 DWM 撤除 Acrylic 模糊只剩 tint（OS 既定行为，window-vibrancy 文档明示），透明态非常态；⑤内容双重贴边（卡占满窗、元件占满卡）。
> 关键洞察：①参考图的"四角弧度感"来自**亮色描边在转角处的绕曲**，不来自深色填充层——单层化不牺牲弧度；②WebView2 的 CSS backdrop-filter 采样不到桌面像素，玻璃磨砂必须由 DWM 材质承担，而 Mica 常驻不随失焦消失（Acrylic 会）→ 材质只此一种：Mica 铺满窗口矩形，"板外裸雾、板内薄纱"由页面透明度分层实现；③拖拽修复与材质无耦合，先行独立修；④用户系统 Win11（build 26200），Mica 可用。
> 目标：单层真实玻璃——Mica 常驻底 + 20px 大圆角玻璃板（薄纱体色 + 亮边描边，以边定义形状）+ 浅色高透令牌基准 + 呼吸边距 + 拖拽全窗恢复。
> 状态：✅ 已完成（2026-09-13，V0.1.0.14；路线三易——①Mica 废弃（暗色≈不透）②采集式自绘废弃（被遮挡像素不可采，1:1 校准实证递归镜像 2-3fps）③最终路线 = DWM 系统背板（Terminal 同款 DWMSBT_TRANSIENTWINDOW）：聚焦态真磨砂实证达成，失焦态经实证判定为 DWM 材质焦点绑定边界、如实登记不宣布达成，演进另立 PL011；门禁七项全绿 + 用户实机判定；验证记录见 .temp/pl010-verification.md）

### 方向修订（2026-09-13 二次拍板）

Mica 首施实测"暗色模式下 ≈ 不透明深板"（透明度较 Acrylic 退化）、Mica 铺满矩形致双层依旧——用户判定退化返工，材质路线改为 **C：采集式自绘常驻玻璃**（GDI 抓窗口周边桌面 → 自身矩形镜像修补 → 模糊 → 事件推帧 → 前端画布 + CSS blur；失焦常驻由自绘保证）；apply_mica、DWM 外框圆角均移除，单框由 26px 透明边距（露清晰桌面）+ 板 20px 圆角构成；其余定案（浅色基准/单层/拖拽必修/呼吸边距）不变。

### 方向修订二（2026-09-13 三次拍板，最终路线）

采集式自绘（C 路线）经用户实机判定**效果不符**：①被遮挡像素物理不可采集——背景只能由周边插值近似，天然"糊弄感"；②1:1 校准实验（CALIBRATION_1TO1）实证自身递归镜像 + 仅 2-3fps；③插值与延迟不可根除。**最终路线 = DWM 系统背板**（微软终端同款）：`DwmSetWindowAttribute(DWMWA_SYSTEMBACKDROP_TYPE = DWMSBT_TRANSIENTWINDOW)`——DWM 实时模糊窗口背后**真实内容**并常驻合成，失焦不消失、零应用开销、零采集延迟；应用侧仅叠薄纱调浓度（用户判定基准 ~10%）。采集管线（backdrop.rs + 前端画布）移除；"板/边距"双层语义删除——整窗一块磨砂，内容呼吸内缩；系统圆角 8px（与 20px 内板互斥，用户选删内层）。

### 方向定案（2026-09-13 用户拍板；3、4 已随方向修订二调整）

1. 主题路线：浅色高透为基准（贴参考），暗色为同配方低档衍生（低透深纱 + 浅字 + 投影保对比）
2. 层结构：单层化——删除 72% 暗卡；保留 20px 大圆角玻璃板（薄纱体色 + 亮边描边 + rim，以边定义形状）
3. Mica 常驻：apply_acrylic → apply_mica（暗/亮随系统主题启动时定）；接受"Mica 磨砂弱于 Acrylic，浓度不足处靠局部薄纱补"
4. 外窗口矩形加 DWM 8px 系统圆角（DWMWCP_ROUND），与板 20px 形成双圈圆角（用户拍板"可选细节"要）
5. 拖拽 bug 必修：全局 mousedown + startDragging 接线，交互元素白名单排除
6. 呼吸边距：窗口边距 12 → 26px，内容不再贴边
7. 布局骨架（dock/环/五卡/周视图）零重排；材质配方表走 frontend-design skill，用户审后冻结

### 收口结论（2026-09-13）

- **达成**：聚焦态真磨砂——DWM 实时模糊窗口背后真实内容（壁纸色彩透板、拖动背后窗口实时可见、零采集零延迟）；全窗单层（margin 0、radius 8px、10% 统一薄纱）；拖拽全窗恢复；浅色高透配方表落地。门禁七项全绿，用户实机判定通过。
- **如实登记的边界**：DWM 系统背板 Acrylic（DWMSBT_TRANSIENTWINDOW）是焦点绑定材质——失焦时 DWM 自动回退为不透明灰板，无任何受支持 API 可拦截（方向修订二所记"失焦不消失"为立项时预期，实测证伪）。核实微软终端源码与 issue 史：其窗口层系统背板仅 DWMSBT_MAINWINDOW（Mica），面板级 Acrylic 走 WinUI 私有 AcrylicBrush；1.19 的"失焦保磨砂"（PR #15923 unfocusedAppearance.useAcrylic）依赖该私有通道，WebView2 应用无等价物。**结论：Windows 第三方应用"失焦真磨砂"无受支持途径**，本任务不宣布达成该项。
- **演进**：用户拍板新路线 PL011 焦点联动材质——平时纯 alpha 透明（alpha 合成不绑定焦点、常驻不变），聚焦瞬间挂 Acrylic 背板、失焦即刻撤回透明（"平时透明，聚焦那一下给磨砂"）。方案见附录 PL011。

### 材质配方表（PL010.3，2026-09-13 交互评审冻结）

**全局令牌**（浅色基准 / 暗色衍生）：

| 令牌                                    | 浅色                                                                | 暗色                               | 说明                                                             |
| --------------------------------------- | ------------------------------------------------------------------- | ---------------------------------- | ---------------------------------------------------------------- |
| `--glass-bg` 板体纱                     | `rgba(255,255,255,0.12)`                                            | `rgba(24,18,40,0.38)`              | 透是第一属性；暗色 38% 深纱替代 72% 暗卡                         |
| `--glass-stroke` 亮边                   | `inset 0 0 0 1.5px rgba(255,255,255,0.78)`                          | `…rgba(255,255,255,0.28)`          | 板的形状由亮边定义                                               |
| `--rim-light` 顶缘                      | `inset 0 1.5px 0 rgba(255,255,255,0.9)`                             | `…0.32`                            | 保留                                                             |
| `--shadow-candy` 板影                   | `0 16px 40px rgba(80,60,120,0.25)`                                  | `0 16px 40px rgba(0,0,0,0.5)`      | 落在 Mica 上，悬浮感来源                                         |
| `--text-shadow`（新增）                 | `none`                                                              | `0 1px 3px rgba(0,0,0,0.4)`        | 暗色低透下浅字对比度补偿（`.digits` 排除，防渐变描字被投影穿透） |
| `--panel-bg` / `--panel-stroke`（新增） | `rgba(255,255,255,0.38)` / `inset 0 0 0 1px rgba(255,255,255,0.65)` | `rgba(255,255,255,0.07)` / `…0.14` | 板上浮起件（五卡）                                               |
| `--btn-cast`（新增）                    | `rgba(90,70,140,0.35)`                                              | `rgba(0,0,0,0.45)`                 | 按钮落影                                                         |
| accent / mint / 三组渐变 / `--chip-bg`  | 不变                                                                | 不变                               | PL007 资产保留                                                   |

**逐件对号**（体色纱，浅/暗）：板 12%/38%；统计五卡 38%/6%；dock 42%/9%（`--chip-bg` 维持）；chips 50%/10%（维持）；浮层虹彩不变、浅色体色 +20% 白纱（渐变三 stop 各混 20% 白）；文案条不变。

**按钮厚度三件套**（主/次统一）：顶部白光层（`linear-gradient(180deg, rgba(255,255,255,0.28), transparent 38%)` 叠加各自体色）+ 底缘暗线（`inset 0 -2px 0 rgba(0,0,0,0.18)`）+ 落影（`0 8px 20px var(--btn-cast)`）。

**结构参数**：窗口边距 12 → 26px；板 radius 20px；外框 DWM 8px；`.digits` 排除 text-shadow；`.glass-card`/`.glass-panel` 移除无效的 backdrop-filter（页面树后无内容，磨砂由 Mica 承担）。

### 实现措施（按阶段拆解）

- 阶段 A 拖拽修复（独立先行）：App.vue script 增全局 `mousedown` 处理——`e.target.closest("button, input, textarea, a, .dock, .floating-sheet, .pill")` 命中即忽略，否则 `getCurrentWindow().startDragging()`；移除 main/title/digits 三处 data-tauri-drag-region；核对 capabilities 含 core:window:allow-start-dragging（旧属性方案已生效，权限应在）
- 阶段 B 窗口装配（lib.rs）：apply_acrylic → apply_mica（dark = window.theme() 判定）；新增 `#[link(name = "dwmapi")] extern "system"` 的 DwmSetWindowAttribute 设 DWMWA_WINDOW_CORNER_PREFERENCE = DWMWCP_ROUND（外框 8px 圆角）；失败严格抛错；**零新依赖**（dwmapi 系统库 extern 直连）
- 阶段 C 材质配方表（frontend-design skill，**用户审后冻结**）：参考图五要素（透/亮/边/影/厚度）→ 逐件令牌值表：板/统计卡/dock/chips/浮层/文案条/环/按钮 × 浅色基准 + 暗色衍生，落本附录"材质配方表"节
- 阶段 D 令牌与结构落地（App.vue :root + scoped + StatsView/DockNav/TimerCard）：按冻结配方表换血；.glass-card 改唯一玻璃板（margin 26px、radius 20、纱+亮边+rim）；五卡降级轻浮起；.temp 对比度探针更新 pl010 版（新配方双主题正文 ≥4.5 断言）
- 阶段 E live 审计：V1–V5 + 参考图五要素逐项对照，问题即改
- 阶段 F 收口回写

### 验证方案（全部可执行、可断言）

| #   | 层级 | 检验内容     | 手段与通过标准                                            |
| --- | ---- | ------------ | --------------------------------------------------------- |
| V1  | live | 拖拽恢复     | 非交互区按住任意位置可拖动窗口（修复前仅标题/数字两行）   |
| V2  | live | 失焦常驻     | 点其它窗口再回看：玻璃仍在、无灰泥带（Mica 不随焦点消失） |
| V3  | live | 单层同心圆角 | 外 8px 系统圆角 + 板 20px 圆角；暗卡与灰泥双层对比消失    |
| V4  | 实测 | 对比度       | pl010 探针双主题正文 ≥4.5:1                               |
| V5  | live | 参考图五要素 | 透/亮/边/影/厚度逐项对照参考图                            |
| —   | 门禁 | 七项全绿     | 同 PL007                                                  |

### 验收标准

1. V1–V5 + 门禁全绿
2. 零新依赖（dwmapi extern 直连）；布局骨架零重排；保活不回归；数据口径零变化
3. 结论回写本附录（含配方表冻结稿）+ x.progress 勾结 + README/AGENTS 状态行

### 明确不做（YAGNI 边界）

- 信息架构/组件布局零重排（PL008 骨架保留）
- three.js 不引入（PL009 结论维持）
- Mica 明暗运行中热切换不做（随系统主题启动时定，重启生效——登记已知小瑕疵）
- SetWindowRgn / 三方圆角 crate 不引入（锯齿 / 依赖负资产）

### 拆分 todo

见 x.progress.md「PL010」任务组。

## 附录 PL011：焦点联动材质·平时透明聚焦磨砂（2026-09-13 立项）

> 背景：PL010.7 实证 DWM 系统背板 Acrylic 为焦点绑定材质——聚焦真磨砂完美，失焦回退不透明灰板且无受支持 API 可拦（详见附录 PL010 收口结论）。用户讨论后拍板：不与 OS 边界对抗，把两种机制各用其长——**透明**（alpha 像素合成，OS 从不没收，失焦常驻）承担常态，**磨砂**（DWM Acrylic 背板）作为聚焦态点睛，"平时透明，聚焦那一下给磨砂"。
> 状态：✅ 已完成（2026-09-13，V0.1.1.1；用户目验通过——平时透明常驻/聚焦真磨砂/失焦即刻回透明无灰板/反复切换无残留；两轮观感调校 + 分态纱浓度定案；版本推进 0.1.0 → 0.1.1；门禁全绿）

### 方向定案（2026-09-13 用户拍板）

1. 平时（含失焦）：窗口纯 alpha 透明，无 DWM 背板——观感与 PL009 失焦态同族（壁纸直透 + 全窗薄纱）
2. 聚焦瞬间：挂 DWMSBT_TRANSIENTWINDOW Acrylic 背板，真磨砂浮现；失焦即刻撤回（DWMSBT_NONE），回透明
3. 实现零新依赖：复用 extern dwmapi 直连；Rust 侧切换即全部改动，前端结构与布局零重排
4. 可读性保底：透明态下全窗 10% 纱与文字对比度复核，不足只调浓度不动结构

### 实现措施（按层拆解到文件/函数级）

- **core/src/lib.rs（Windows 装配）**：现 setup 内的 DWM 背板挂载块抽局部闭包 `set_backdrop(hwnd: isize, kind: u32)`（内部 DwmSetWindowAttribute(hwnd, 38, &kind, 4)，HRESULT 非零严格抛错）；setup 改为**默认不挂背板**（删除启动时 kind=3 调用，平时透明）；`.on_window_event` 增 `WindowEvent::Focused(focused)` 分支——focused=true 调 set_backdrop(3)（DWMSBT_TRANSIENTWINDOW）、false 调 set_backdrop(0)（DWMSBT_NONE）；窗口事件在主线程回调，DWM 属性切换可直调无需投递
- **ui/App.vue（按需，PL011.2 判定后）**：仅当透明态可读性不足时调 --glass-bg 浓度或 --glass-stroke 强度；结构、布局、事件监听零改动

### 验证方案（全部可执行、可断言）

- live 焦点循环：点窗口聚焦 → 真磨砂浮现（壁纸透板）；切走 → 即刻回透明；反复 10 轮无闪烁、无灰板残留、无内存/句柄泄漏迹象
- live 常态：平时窗口透明透出桌面，文字/数字/统计卡可读（双主题跟随系统，禁改系统设置）
- live 常驻回归：托盘关闭隐藏、Alt+Shift+P/S 热键唤起后焦点态材质正确；拖拽全窗仍可用
- 门禁七项：cargo fmt --check / clippy -D warnings / test / doc + npm run build + vue-tsc + prettier

### 验收标准

- 平时透明常驻（失焦不变化）；聚焦真磨砂；切换瞬时无残留
- 布局骨架（dock/环/五卡/周视图）零重排；数据口径零变化；零新依赖

### 明确不做（YAGNI 边界）

- 不做失焦真磨砂（OS 无受支持途径，已实证登记 PL010 收口结论）
- 不做壁纸自糊假磨砂（底下列表已弃，用户选定 alpha 透明路线）
- 不做未公开 ACCENT 变体（未文档化、随版本变动）
- 不换 UI 框架/引擎（WinUI 3 重写代价不成立）

### 收口结论（2026-09-13）

- **达成**：平时纯 alpha 透明常驻（失焦不变，与 PL009 失焦态同族观感）；聚焦瞬间 DWM Acrylic 真磨砂浮现，失焦即刻撤回（DWMSBT_TRANSIENTWINDOW=3 ↔ DWMSBT_NONE=1 焦点联动）；反复切换无闪烁无灰板残留。用户目验通过。
- **实施定案两则**：①失焦撤回用 `DWMSBT_NONE=1` 而非 0——0 是 `DWMSBT_AUTO`，会让 DWM 自行决定材质（微软文档核实）；②观感两轮调校 + 分态纱浓度（用户拍板）：透明态 30% 纱（浅色白纱 / 暗色**纯黑**纱，原暗紫废弃），磨砂态 0% 纱（材质本体已足够）；实现 = Rust Focused 分支加发 `window-focus` 事件 + 前端 `focused` class + `isFocused()` 启动兜底。
- **验证方式说明**：live 判定以用户目验为准（CUA 桌面控制服务中断，PowerShell 自动探针因 Windows 前台锁 + 提权窗口 UIPI 限制废弃，探针脚本留档 .temp/）。

### 拆分 todo

见 x.progress.md「PL011」任务组。

---

## 附录 A003：全量代码审计报告（第3轮，2026-09-13）

> 状态：✅ 已修复（2026-09-13 收口，V0.1.1.2；FIX003 十一条勾结见 x.progress.md，反向验证与死锁 TDD 实证记录见该文件条目注记；storage_probe.rs 维持保留——A002-O4 豁免继续，删除选项保留；FIX003.1 live 用户目验移交收尾确认）
> 范围：core/ 全部 .rs + Cargo.toml + tauri.conf.json + capabilities/default.json + ui/ 全部前端（11 文件）+ 根配置 + 静态资源引用；不审计 node_modules / dist / target / .temp / .agents / 第三方依赖与生成代码。
> 方式：主会话回归复核（A002 十五项逐项 grep + git 行级对比）+ 三路并行逐文件通读（Rust 纯逻辑组 / Tauri 集成组 / 前端组 explore 子任务）+ 门禁实测（cargo fmt --check / clippy -D warnings / test 90 / doc + vue-tsc / prettier 全绿）。基线 2b9e380（V0.1.1.1）。
> 本轮专项（用户指定）：清理死代码与已作废功能——设独立死代码专项清单（7 项确认），并对全部命令/依赖/组件/CSS 变量/选择器/TS 导出做对账式存活核对。

### 零、上轮修复复核清单（A002 → FIX002，V0.1.0.10）

| 上轮条目                                                           | 现状                                                                                                                                                  | 证据                                                                    |
| ------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------- |
| P2-1 三处回滚（pause/start/resume）                                | ✅ 在位且带 3 条锚测试                                                                                                                                | commands/session.rs:75-130、476-512                                     |
| P3-3 段末回填口径 / P3-9 async 化 / P3-10 storage 事务化           | ✅ 在位（四读命令 async、三对事务方法 + 测试）                                                                                                        | stats.rs:34、session.rs:252、workday.rs:180/189；storage.rs:213/235/259 |
| P3-11 按钮类收敛 / P3-12 字体继承 / P3-13 format.ts                | ✅ 在位                                                                                                                                               | App.vue:473-524、467；format.ts 三函数四组件消费                        |
| P3-14 persist/close 合一 / P3-16 白名单登记 / P3-17 paths 测试分支 | ✅ 在位                                                                                                                                               | session.rs:148-161；AGENTS 白名单 ⑤；paths.rs:67-77                     |
| **P3-15 死代码（last_fired 加 cfg(test)）**                        | ⚠️ **半漏改**：clock_out_inner 已改 ()，但 `ReminderFire::last_fired()` 的 `#[cfg(test)]` 未加（git 证实 d7a85cb 未触 reminder.rs）——登记已修实际未修 | reminder.rs:49                                                          |
| 材质废弃路线残留（vibrancy/采集管线/Mica API）                     | ✅ 源码零残留（Cargo/imports/CSS 全净；残留仅注释与文档层，见 P3-4/5/6）                                                                              | 全仓 grep                                                               |

结论：15 项中 14 项完好、1 项半漏改；FIX002 之后历经 PL008–PL011 四轮大改，零回退、零新引入回归（既有观察项全部维持原状无恶化）。

### 一、P0-P3 修复清单（按严重度）

无 P0/P1；P2 一项、P3 十二项。性质标注：上轮已列未修 = 遗留，其余 = 新增。

| #     | 文件:行号                                                | 类型 | 描述                                                                                                                                                                                                                                                                                      | 建议                                                                                                                                   | 性质                        | 影响面         |
| ----- | -------------------------------------------------------- | ---- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------- | --------------------------- | -------------- |
| P2-1  | App.vue:65-66（根因）+ App.vue:293 + ConfirmModal.vue:15 | 1    | 拖拽白名单 `DRAG_INTERACTIVE` 不含 `.overlay`：设置浮层与确认框均为 `.overlay` + `@click.self`（点外关闭）——点遮罩空白处先触发 `startDragging()` 进入 OS 拖拽循环，click 大概率被吞，点外关闭面板/取消确认框疑似失效且意外拖窗（PL010.1 引入的交互回归；需 live 验证确认 click 派发行为） | 白名单补 `.overlay`                                                                                                                    | 新增                        | Vue 前端       |
| P3-1  | reminder.rs:49                                           | 5    | `ReminderFire::last_fired()` 生产零调用仅本模块测试用，`#[cfg(test)]` 未落地                                                                                                                                                                                                              | 加 `#[cfg(test)]` 一行；x.progress FIX002 该条结论更正                                                                                 | 遗留（上轮漏改）            | 死代码         |
| P3-2  | workday.rs:65-70,107-112                                 | 5    | `DutySpan` 只产不读：生产唯一调用点（commands/workday.rs:51）`?` 丢弃返回值，"供落库关行"职责已被 OnDuty.id + 事务方法取代，字段 doc 过时                                                                                                                                                 | `WorkdayState::clock_out` 返回改 `Result<(), WorkdayError>`，删 `DutySpan`（workday.rs:358 测试同步）                                  | 遗留（P3-15 同根残留）      | 纯逻辑 API     |
| P3-3  | storage.rs:82,123,134                                    | 5    | `add_session`/`workday_open`/`workday_close` 在 FIX002 事务化后生产调用归零，仅测试在用——测试专用 API 未隔离                                                                                                                                                                              | 加 `#[cfg(test)]`（与 session_count 同款）或删除并迁测试到事务版                                                                       | 新增                        | 存储层封装     |
| P3-4  | lib.rs:6                                                 | 6    | 模块头注释"DWM 系统背板**常驻**真磨砂（PL010.7）"——描述已废弃的 PL010 常驻架构，PL011 焦点联动后失实                                                                                                                                                                                      | 改"焦点联动：平时透明、聚焦挂 DWM Acrylic（PL011）"                                                                                    | 新增                        | 文档           |
| P3-5  | App.vue:312,347                                          | 6    | 注释"磨砂由 Mica 承担"×2——Mica 路线 PL010 二次拍板已弃，与现实现（PL011 焦点联动 DWM Acrylic）矛盾                                                                                                                                                                                        | 改"DWM Acrylic 背板承担（焦点联动）"                                                                                                   | 新增                        | 文档           |
| P3-6  | AGENTS.md:15,37                                          | 6    | 技术栈表"window-vibrancy（macOS vibrancy / Windows acrylic / Linux blur）"与架构要点"apply_acrylic/apply_vibrancy"——依赖已于 V0.1.0.14 移除，文档未随收口                                                                                                                                 | 改为 extern dwmapi 直连 + 焦点联动描述                                                                                                 | 新增                        | 文档           |
| P3-7  | commands/mod.rs:30                                       | 6    | 注释"三个读命令已 async 化"——实为四个（week_detail PL008.6 起 async）                                                                                                                                                                                                                     | 数字更正                                                                                                                               | 新增                        | 文档           |
| P3-8  | lib.rs:253-255,259-261                                   | 13   | 背板切换失败落日志继续、window-focus emit 失败落日志继续——两处未登记容错（白名单 6 项外，违反"新增容错须先登记"）；且 window-focus 与同体系 reminder-due（commands/reminder.rs:22）emit 失败严格报错双策略并存                                                                            | 白名单登记第 ⑦ 项三要素（场景=焦点联动 DWM 切换/事件发送失败；降级=落日志维持前态；理由=材质为纯装饰层，运行时焦点事件不可中断主流程） | PL011 新增                  | 错误策略合规   |
| P3-9  | commands/reminder.rs:35                                  | 13   | 白名单 ② 登记降级行为"错误落日志"，实际落 `eprintln`——release GUI 下无处可落等于零记录，登记的降级行为落空                                                                                                                                                                                | 改 `crate::diag::log(...)`                                                                                                             | 遗留（FIX002.8 覆盖面缺口） | 容错白名单合规 |
| P3-10 | lib.rs:41,44,47,57,69,80,133,207                         | 10   | 托盘/热键路径窗口操作失败、计时切换失败、"已恢复在岗状态"提示仍只走 eprintln——release 不可见（diag 通道已建未接全；207 行为用户可感知关键状态）                                                                                                                                           | 关键失败路径（toggle_session 失败、恢复在岗）补 diag::log，eprintln 保留作 dev 输出                                                    | 遗留（同 P3-9 根因）        | 可观测性       |
| P3-11 | commands/session.rs:166-183                              | 2    | restart 留痕失败无回滚：`reset()+start()` 后 `mark_segment` 失败直接返回 Err，新段 Running 但 SegmentStart 事件丢失——与 FIX002.1 为 start/resume/pause 建立的回滚原则不一致；后果 = 时间图谱该段缺起点（reduce_day 容错吸收三值不差错），可达 = 存储持久故障（低频）                      | 与 FIX002.1 同款补回滚（restore 到 reset 前态），或注释声明"restart 留痕失败不回滚"的理由                                              | 新增                        | events 完整性  |
| P3-12 | App.vue:339                                              | 5    | `--r-ctrl: 13px` 孤儿变量——全 ui/ 定义零消费（分段控件被 PL008 dock 取代）                                                                                                                                                                                                                | 删除一行                                                                                                                               | 新增                        | Vue 前端       |
| P3-13 | StatsView.vue:32-36,88,123 + App.vue:97-112              | 13   | ① week_detail 失败仅 console.error，周卡静默消失（与 day_detail 的 loadError 不对称，FIX002 只覆盖了单日）；② 首拉失败时 loadError 红条与"本日无打卡记录"空文案同屏；③ session_stats 失败仅 console，统计行静默冻结旧值（环口径 day_detail 失败沿用旧值属有意降级但未登记）               | 周卡失败并入 loadError；loadError 时隐藏 empty 行；session_stats 维持现状登记观察                                                      | 新增                        | Vue 前端       |

### 死代码专项清单（用户本轮指定重点，7 项确认）

| 符号/位置                                                                      | 判定依据                                                                    | 建议动作                                   |
| ------------------------------------------------------------------------------ | --------------------------------------------------------------------------- | ------------------------------------------ |
| `ReminderFire::last_fired()`（reminder.rs:49-51）                              | 生产零调用，仅同模块 tests（79/99 行）                                      | `#[cfg(test)]`                             |
| `Storage::add_session`/`workday_open`/`workday_close`（storage.rs:82,123,134） | 事务化后生产零调用（生产走 *_with_event），仅 tests                         | `#[cfg(test)]` 或删除迁测试                |
| `DutySpan`（workday.rs:65-70）                                                 | 只产不读（唯一生产调用点丢弃）                                              | 删除（随 P3-2）                            |
| `--r-ctrl`（App.vue:339）                                                      | 定义零消费孤儿                                                              | 删除（随 P3-12）                           |
| `core/tests/storage_probe.rs` 整文件                                           | PL002 依赖探针，自注"收口时若无复用价值随任务注记决定去留"；删后测试总数 89 | **留用户决断**：删（简练）或留（集成冒烟） |
| 过时注释五处（lib.rs:6 / App.vue:312,347 / mod.rs:30 / AGENTS.md:15,37）       | 描述已废弃架构（常驻磨砂/Mica/vibrancy/三个读命令）                         | 随 P3-4/5/6/7 更正                         |
| `saturate(1.6)`（App.vue:328，--glass-blur 内）                                | PL006 玻璃配方遗产，现唯一消费点 .iridescent 浮层                           | 观察项，可评估精简                         |

对账为"活"的高频嫌疑项（免重审）：12 个 Tauri 命令全部有前端 invoke 对账（generate_handler ↔ ui/ grep）；Cargo.toml 10 依赖零未用；package.json 零幽灵依赖；7 组件（含 StatsCard.vue 36 行）全部在引用链；33 个 CSS 变量除 --r-ctrl 外全部有消费；零孤儿选择器；零孤儿 TS（ref/导出/props/emits 全接线）；settings.rs 四字段全消费；period.rs/diag.rs 无未用能力；assets 两资源全引用；data-tauri-drag-region/backdrop 画布/CALIBRATION 零残留。

> 观察项已于 2026-09-13 全量整理归并至「四、审计观察项豁免定案清单」（失效销项 + 重复归并 + 逐项重审定案），本报告不再保留平行副本。

### 三、亮点

- FIX002 十五项经 PL008–PL011 四轮大改零回退；唯一漏改（last_fired cfg(test)）一行可补
- 锁序纪律逐函数复核零违反（三路独立交叉）；生产代码零 unwrap/expect；SQL 全参数化零拼接
- 材质路线三易其稿但废弃源码清理彻底（vibrancy/采集管线/Mica API 零源码残留），残留仅在注释与文档层——"不长期保留废弃方案"原则在代码层执行到位
- 三方契约（12 命令 / serde 结构 / types.ts / 事件名）逐字段对齐；设置默认值单一来源（default_* 函数族）无漂移；reduced-motion 退避完整覆盖（PL011 分态纱为瞬时切换不属运动，无需退避）
