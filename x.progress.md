# 进度追踪（x.progress.md）

> 文件职责：任务清单与进度追踪，与 `z.plan.md`（方案与审计归档）配套：方案在 z.plan 展开，执行拆条在本文件勾选。结构：**`## 已完成 ✅` 区在前、`## 未完成` 区在后**，任务完成后整组移动位置。
> 格式速查：任务组 `### PL{NNN}: {标题} [来源引用]`（来源引用：[plan#Phase N] 计划书 / [problems#N] 问题备忘录 / [audit#A{NNN}] 审计报告），组内用 `#### {小节}` 分层；子任务 `- [ ] PL{NNN}.{序号} {标题} —— {做法}；验证：{检验方式}`；审计修复任务组 FIX{NNN} 由 audit-report 归档环节生成，条目格式 `- [ ] FIX{NNN}.{序号} [P{级别}] {标题} —— {做法}；验证：{检验方式}`，编号规则见 `.agents/skills/audit-report`。
> 勾选注记：条目完成时改 `[x]`，并在句尾追加 `（YYYY-MM-DD 已验证：{一句话结论}）`——结论如实，不达标不降级宣布。

## 已完成 ✅

### PL001: 玻璃壳与最小计时闭环 [plan#Phase 0/1/2]

> 结果：玻璃栈判定通过（G1–G4 全过，Windows Acrylic 实测）+ 计时状态机/命令层 TDD 全绿（11/11）+ 玻璃 UI 计时闭环交付；U1 三轮验收通过，终版显示定案 = 十分秒位 HH:MM:SS.d。全组 15 条勾结。详见 z.plan.md 附录 PL001。

#### 阶段 A：工程骨架

- [x] PL001.1 工具链探测与 git 仓库 —— 盘点 cargo/rustc/node/npm/Tauri CLI 在位与版本，缺口安装清单交用户安装；git init 由用户执行（.gitignore 已就位）；验证：`cargo --version`/`node --version` 等全在位且版本满足 AGENTS.md 要求（2026-09-08 已验证：cargo 1.96.1 / Node 26.7.0 / VS Build Tools 18 全在位零缺口；Tauri CLI 决策走 npm devDep `@tauri-apps/cli` 免 cargo install 长编译，探测记录 `.temp/pl001a-toolchain.md`；git 仓库未初始化，待用户执行）
- [x] PL001.2 Tauri 2 工程骨架 —— `src-tauri/`（Cargo.toml + tauri.conf.json 透明无边框 360×480 + main.rs 空壳）+ `src/`（Vue3+TS+Vite + App.vue 占位）；验证：开发窗口能打开、控制台无错误（长驻 dev 不适合自动化，人工或等价验证）（2026-09-08 已验证：npm build + cargo build 全绿；等价验证 = debug exe 启动 5 秒探测 MainWindowTitle=[CapsulePulse]、进程存活后受控关闭；无 devUrl 自包含形态规避系列已知的 debug exe 连 dev server 白屏坑；占位图标由 `.temp/gen-icon.mjs` 程序生成；依赖版本锚定 typescript 5.9.3——npm 默认解析到 TS 7.0.2 与 vue-tsc 3.3 不兼容 ERR_PACKAGE_PATH_NOT_EXPORTED，降级定案）
- [x] PL001.3 门禁四件套接入与首跑 —— cargo fmt/clippy -D warnings/check + prettier/vue-tsc/npm build 全部接入；验证：首跑全绿并记录耗时基线（回写 z.plan 附录 PL001）（2026-09-08 已验证：四件套首跑全绿——fmt <1s / clippy 2m10s（含全量依赖编译）/ check 5s / cargo build 2m12s / npm build 4s；基线已回写 z.plan 附录 PL001）

#### 阶段 B：玻璃可行性打样（★ 判定点，先于一切 UI 功能）

- [x] PL001.4 透明窗口 + Acrylic —— window-vibrancy `apply_acrylic`（#[cfg(windows)]）+ tauri.conf `transparent: true` + `decorations: false`；验证：G1——透明生效、无黑底、无可接受度以下的闪烁（人工，桌面背景透过可见）（2026-09-08 已验证：**G1 过**——用户确认边缘透明 Acrylic 生效、无黑底无闪烁；apply_acrylic tint (32,32,32,125)，setup 失败严格抛错）
- [x] PL001.5 玻璃卡片与深浅色跟随 —— 卡片半透明底 + `backdrop-filter: blur()`；`prefers-color-scheme` 双主题样式；验证：G2 毛玻璃观感生效 + G3 切换系统主题卡片跟随可读（人工）（2026-09-08 已验证：**G2 过 + G3 过**——用户确认卡片毛玻璃质感成立、深浅色主题跟随实现）
- [x] PL001.6 无边框拖动 —— `data-tauri-drag-region` 拖动区覆盖 + 窗口固定 360×480；验证：G4 拖动流畅、无文字选中副作用、尺寸固定（人工）（2026-09-08 已验证：**G4 首测不过 → 修复后复测过**——根因 = `core:window:default` 不含 `allow-start-dragging`，ACL 静默拒绝拖动 IPC（症状同 CapsuleRetro listen() 静默拒）；capabilities 显式补授 `core:window:allow-start-dragging` 后用户复测拖动正常、无文字误选、尺寸固定；陷阱已沉淀 w.study §3.1 + AGENTS 素材与环境陷阱）
- [x] PL001.7 玻璃判定书 —— G1–G4 逐项结论登记本条目；判死条件 = Acrylic 不可用或闪烁不可接受 → 降级"半透明纯色"并重议产品形态，书面结论回写 z.plan 附录；验证：判定书完成（全过或降级，二选一有书面结论）（2026-09-08 已定案：**全过，不触发降级**——G1 透明/G2 毛玻璃/G3 深浅色/G4 拖动（修复后）全部用户确认通过，判定书见 z.plan 附录 PL001 阶段 B 开展结论）

#### 阶段 C：计时状态机（TDD：先 FAIL 后 PASS）

- [x] PL001.8 Clock 注入与状态骨架 —— `core/session.rs`：`trait Clock`（测试注入手拨假钟）+ 三态 Idle/Running/Paused（enum 或 WorkSession 持态，实现时取简）+ `SessionError`（thiserror）；验证：cargo clippy/check 过（2026-09-08 已验证：WorkSession\<C: Clock = RealClock\> 持态 + SessionState 三态 Copy enum + thiserror 三变体（NotIdle/NotRunning/NotPaused）；Clock 契约 = 单调不减，违约 panic 属程序错误并文档化）
- [x] PL001.9 四操作 TDD —— 先写测试确认 FAIL 再实现：start（仅 Idle）/pause（仅 Running）/resume（仅 Paused，继承累计）/total()（Running 现算、Paused 返累计、Idle 零）+ 多轮循环累计用例；验证：S1 用例全绿（2026-09-08 已验证：红灯 E0405/E0433（仅测试模块编译失败）→ 实现后转绿；实现定案新增第五操作 reset()（任意态回 Idle 清零）——U1"暂停态重开归零"场景所需的最小扩展，start 保持"仅 Idle"严格语义）
- [x] PL001.10 边界与严格抛错用例 —— Idle 下 pause/resume、Running 下重复 start 返回 `Result<_, SessionError>`；零时长会话；假钟任意拨动无 panic；验证：S2/S3 用例全绿，测试零真实 sleep（2026-09-08 已验证：7 用例全绿——含零步进/超大步进混合百轮循环；绿灯阶段修两处测试侧编译错（matches! 守卫 &Duration 绑定、Instant 无 Default 实现改手写），业务代码零 unwrap/expect）
- [x] PL001.10a 结构定案（阶段内决策）—— dead_code 门禁暴露 bin crate 形态问题后转 Tauri 2 标准骨架：`lib.rs`（pub mod core + run() 装配）+ `main.rs` 薄入口——lib pub 项即公开 API，永久消除 dead_code 误报，阶段 D commands.rs 落 lib 侧；验证：门禁全套过（2026-09-08 已验证：fmt/clippy --all-targets -D warnings/check/doc 0 告警/test 7 过/build 全绿）
- [x] PL001.10b 目录风格改造（用户拍板）—— `src-tauri/`→`core/`（整体改名，框架文件与 Cargo.toml 同住已源码实证）、`src/`→`ui/`、内层 `core/` 模块摊平（session.rs 上移 src 根，lib.rs 改 `pub mod session;`，原 mod.rs 层职责并入 lib.rs 模块注释）、`configs/` 预建占位；同步 .gitignore/AGENTS/README/w.study/两 skill 路径；验证：V1–V6 全过（2026-09-08 已验证：npm build 绿；cargo fmt/clippy/check/test 7 过/doc 0 告警/build 绿；exe 启动 TITLE=[CapsulePulse]；`npx tauri info` 从新目录读出版本清单（CLI 探测实证）；prettier 收口。cargo clean 一次——target 缓存烘焙旧绝对路径搬家后失效，属预期代价）

#### 阶段 D：UI 接线（最薄闭环）

- [x] PL001.11 命令层 —— `commands.rs`：session_start/pause/resume/status 四命令（会话存 `Mutex<WorkSession>`，status 返回 state + total_secs）；验证：U2——命令状态逻辑 cargo test 覆盖，不经窗口不依赖前端（2026-09-08 已验证：TDD 红→绿；SessionHandle(Mutex\<WorkSession\>) 经 .manage() 注册，命令核心抽为接收 &SessionHandle 的自由函数脱离 tauri::State 直测；**四命令之外新增 session_restart**（reset + start 同锁原子，U1"重开归零"承载，实现决策记录）；CommandError（Session 透传/Poisoned）严格报错跨 IPC 序列化；DTO 仅暴露 state 标识 + total_ms（内部计时字段不出 IPC）；U2 4 用例，全组 cargo test 11/11）
- [x] PL001.12 TimerCard 与 App 布局 —— 大计时器等宽数字（HH:MM:SS）+ 按钮状态切换（开始 ↔ 暂停/继续）；setInterval 1s invoke status、组件卸载清理；玻璃卡片沿用 B 阶段定案；验证：vue-tsc 过 + U1 人工完整流程（开始→数字走→暂停→停走→继续→续走→重开归零）（2026-09-08 已验证：vue-tsc/npm build 绿；TimerCard 等宽数字（tabular-nums）+ 按钮三态（开始/暂停/继续+重开）+ tick 拉取卸载清理 + 动作后立即刷新；**U1 三轮**：轮1 秒进位最坏迟到 2s（1s 轮询相位错配 + 秒截断）判不可接受 → tick 250ms；轮2 确认 1.0~1.25s 属 HH:MM:SS 秒表语义、静止感仍在 → 用户拍板显示定案变更 **HH:MM:SS → HH:MM:SS.d 十分秒位**（计划书 §4 偏差，z.plan 为权威定案）+ tick 100ms + DTO total_secs→total_ms；轮3 用户确认通过）
- [x] PL001.13 PL001 收口 —— 门禁四件套全绿；G/S/U 实测结论回写 z.plan 附录 PL001；README/AGENTS 状态行回改；验证：门禁全绿 + 文档一致性核对（2026-09-08 已验证：收口轮门禁全绿（fmt --check/clippy -D warnings/test 11/doc 0 告警/npm build）+ exe 启动探测；U1–U3 结论回写 z.plan 阶段 D 开展结论 + 收口结论；README/AGENTS 状态行改"PL001 已完成"；反向验收与过程记录 `.temp/pl001-verification.md`）

## 未完成

（暂无——下一个大件：计划书 Phase 1 存储与统计聚合（PL002 候选）或 Phase 3 提醒调度，未立项）
