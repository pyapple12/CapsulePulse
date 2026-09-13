# 进度追踪（x.progress.md）

> 文件职责：任务清单与进度追踪，与 `z.plan.md`（方案与审计归档）配套：方案在 z.plan 展开，执行拆条在本文件勾选。结构：**`## 已完成 ✅` 区在前、`## 未完成` 区在后**，任务完成后整组移动到已完成区；**两个区内部均按从上到下 = 从旧到新排序，新组一律追加在区末尾**；已完成区历史组全量保留、只增不删（2026-09-10 定案，此前 V0.1.0.2/V0.1.0.3 两次收口滚出 PL001/PL002 属违规操作，已自 git 历史恢复）。
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

### PL002: 存储与统计聚合 [plan#Phase 1/2]

> 结果：暂停/重开即落库（零秒段跳过），今日/本周/累计三聚合 + 统计行 UI；跨零点/跨周边界用例全绿；U1 人工六项一次通过（含重启 app 持久性）。全组 13 条勾结。详见 z.plan.md 附录 PL002。

#### 阶段 A：依赖与时间边界（TDD）

- [x] PL002.1 依赖接入与探针 —— cargo add rusqlite（bundled）/chrono/dirs；`Connection::open_in_memory` 探针（编译 + 建表查询 smoke）；验证：cargo test 编译绿 + smoke 用例过（2026-09-09 已验证：rusqlite 0.40 bundled/chrono 0.4.45/dirs 7.0 入账；探针 `core/tests/storage_probe.rs` 过——bundled SQLite 编译走全局缓存，未现长编译）
- [x] PL002.2 时间边界纯函数 TDD —— `core/src/period.rs`：今日起点/本周起点（周一起，入参本地朴素时间，出参 Unix 秒）；用例：跨零点、周日 23:59:59→周一 00:00:00 翻转、周内各天、恰在边界 00:00:00；验证：T1 用例先 FAIL 后 PASS，零真实时间依赖（2026-09-09 已验证：红灯 E0425 → 实现转绿 4 用例；实现修正一处——chrono 0.4.45 的 num_days_from_monday 在 Weekday 枚举上（now.weekday().num_days_from_monday()），Datelike 直调已移除；泛型 over TimeZone 使测试固定时区、生产 Local，机器时区无关）
- [x] PL002.3 period 生产接线 —— `Local::now().naive_local()` 转换薄入口（核心逻辑已时区无关，本条只接生产时区）；验证：clippy/check 过（2026-09-09 已验证：随 PL002.10 stats_snapshot 以 Local::now() 接线完成；时区无关设计使生产接线为零逻辑）

#### 阶段 B：存储层 Repository（TDD，内存 db）

- [x] PL002.4 Storage 骨架与建表 —— `core/src/storage.rs`：`StorageError`（thiserror：Sqlite 透传）、`open(path)`/`open_in_memory()`、建表（计划书 §2.2 schema：started_at/seconds 均非空）；验证：open_in_memory 建表用例过（2026-09-09 已验证：另加 Io/HomeDirUnavailable 变体与 open_default（PL002.11 前置）；建表幂等 CREATE TABLE IF NOT EXISTS）
- [x] PL002.5 add_session 与参数化查询 —— insert 全参数绑定（禁 SQL 拼接）；验证：插入→查询往返用例过（2026-09-09 已验证：?1/?2 绑定 + COALESCE 空表为零）
- [x] PL002.6 聚合查询 —— today/week/all 三 SUM（started_at ≥ 边界，all 无条件，边界经 period）；用例：跨零点排除、跨周排除、多段求和、空表为零；验证：T2 用例先 FAIL 后 PASS（2026-09-09 已验证：上周/本周早于今日/昨日/今晨四记录断言 today=300/week=900/all=1000；边界恰等计入（>= 语义）另有专用例）

#### 阶段 C：命令层接线

- [x] PL002.7 pause 返回段时长 —— `WorkSession::pause()` 演进为 `Result<Duration>`（本段时长 = 落库取数源），既有用例同步演进；验证：演进式 TDD，全组测试绿（2026-09-09 已验证：内部重构 = "本段起点 + 段累计"分离（SessionState 形状与对外语义不变）；multi_round 用例强化为逐段断言 100/30/45；invalid 用例 Ok(())→Ok(ZERO)）
- [x] PL002.8 落库接线 —— managed state 扩为 session + storage；pause 命令段 > 0 落库（started_at = wall_now − 段秒，SystemTime 取自命令层，session.rs 纯度不破）；验证：T3 内存 db 断言 pause→库内一行（2026-09-09 已验证：AppContext\<C: Clock=RealClock\>{session, storage: Mutex\<Storage\>}——rusqlite Connection 非 Sync 入 Mutex 解 tauri manage Send+Sync，锁序恒 session→storage；零秒段跳过另有专用例）
- [x] PL002.9 restart 先落库 —— restart：Running → 先 pause 落本段；Paused/Idle → 无未落库段；再 reset + start（单锁原子保持）；用例：Running 重开 → 库内一段 + 新会话 running；验证：T3 用例过（2026-09-09 已验证：双用例——Running 重开落库 100s、Paused 重开不重复落库（段已在最近一次 pause 入库））
- [x] PL002.10 stats 命令 —— `session_stats` 返回 today/week/all 秒数；验证：T3 内存 db 断言三值聚合正确（2026-09-09 已验证：边界内/外注入断言 100/300/700；Local::now() 生产定界即 PL002.3 接线）
- [x] PL002.11 运行时 db 路径 —— `~/.capsule-pulse/pulse.db`（dirs 解析 + 目录自建），打开失败严格报错启动失败；测试一律注入路径/内存库，禁触真实用户目录；验证：构建绿 + 临时路径集成用例过（2026-09-09 已验证：open_default 三错误变体（Sqlite/Io/HomeDirUnavailable）；临时目录文件库"写入→重开→数据在"用例过；run() 初始化失败 eprintln + exit(1)。**落址已翻案**：2026-09-10 起改 data/pulse.db、dirs 退役（热更新定案，现行规则见 AGENTS「运行时数据」条与落址提交说明））

#### 阶段 D：统计行 UI 与收口

- [x] PL002.12 StatsCard 统计行 —— 今日/本周/累计三值（Xh Ym 格式）；刷新 = 挂载 + 动作后 + 30s 兜底（不进 100ms tick）；玻璃样式沿用；验证：vue-tsc 过 + U1 人工闭环（计时→暂停→统计行增长；重开→段计入；重启 app→统计仍在）（2026-09-09 已验证：StatsCard 纯展示组件 + TimerCard changed 事件驱动刷新；vue-tsc/npm build 绿；**U1 六项一次通过**——统计行/暂停落库/多段累计/重开先落库/重启持久性/玻璃无退化）
- [x] PL002.13 PL002 收口 —— 门禁全绿；T1–T3/U1 结论回写 z.plan 附录 PL002；README/AGENTS 状态行回改；验证：门禁全绿 + 文档一致性核对（2026-09-09 已验证：收口轮门禁全绿（fmt --check/clippy -D warnings/test 24/doc 0 告警/npm build/prettier）；结论回写 z.plan 阶段开展结论 + 收口结论；状态行同步；反向验收与过程记录 `.temp/pl002-verification.md`——含一处 python 改源码违规的自纠记录）

### PL003: 提醒调度与设置持久化 [plan#Phase 3]

> 结果：阈值触达 → 系统通知 + 提示音 + 卡片文案条三通道齐发（5 分钟重发、暂停/重开即重置），⚙ 面板设置持久化即时生效，降级三态互不依赖人工验证全过。素材定案变更：提示音 = 用户自备 mp3 经 import 打包。全组 12 条勾结。详见 z.plan.md 附录 PL003。

#### 阶段 A：设置持久化 + 提示音素材（TDD）

- [x] PL003.1 依赖与素材探针 —— cargo add tauri-plugin-notification；.temp/gen-chime.mjs 生成约 0.5s 双音正弦 WAV → ui/public/chime.wav；验证：文件生成且 WAV 头校验过 + cargo test 编译绿（2026-09-09 已验证：tauri-plugin-notification 2.4.0/serde_json 入账；WAV 头 RIFF/PCM/44.1kHz/16bit 校验过；**素材后经用户拍板改为自备 mp3**——见 PL003.12 注记，chime.wav 与生成脚本随 src 变更退役）
- [x] PL003.2 settings.rs TDD —— `core/src/settings.rs`：ReminderSettings（threshold_min=50/sound_enabled=true/notify_enabled=true，serde default）+ load/save（JSON）+ SettingsError（Json/Io）；用例：不存在→默认、往返一致、损坏 JSON 严格报错、非法阈值拒绝；验证：N2 先 FAIL 后 PASS（临时目录，禁真实用户数据）（2026-09-09 已验证：红灯 E0422 → 4 用例绿——缺省回默认/往返一致/损坏严格/0 与 241 越界均拒；另加 HomeDirUnavailable 变体与 default_path）
- [x] PL003.3 容错白名单登记（设置缺省）—— AGENTS 登记"设置文件不存在回退默认值"（NotFound 语义，场景/降级/理由三要素）；验证：文档三要素核对（2026-09-09 已验证：白名单三条之一，JSON 损坏等其余错误仍严格报错的边界已写明）

#### 阶段 B：提醒评估器（TDD）

- [x] PL003.4 reminder.rs 纯逻辑 —— ReminderConfig/ReminderFire/evaluate（阈值触发 + 5min 重发，无减法比较规避段回退下溢）+ clear（暂停/开始/重开时调用）；验证：N1 用例先 FAIL 后 PASS（未达不触发/恰达触发/触发后 4min 不发 5min 发/暂停重置后新段再触发）（2026-09-09 已验证：红灯 E0425 → 4 用例绿；跨段残留不下溢另有专用例）

#### 阶段 C：接线

- [x] PL003.5 WorkSession::segment_secs —— 当前段时长访问器（Running = now − start，否则零）+ 用例；验证：cargo test 全绿（2026-09-09 已验证：Idle 零/Running 实时/Paused 零/resume 后新段独立计量（total=15 段=5））
- [x] PL003.6 AppContext 扩展与命令 —— + settings/fire 双 Mutex；get_settings/set_settings 命令（set 持久化 + 更新内存态）；验证：T4a 临时路径往返用例（2026-09-09 已验证：AppContext 增 settings/fire/settings_path 三字段；set 校验+持久化+内存态更新、非法阈值 0 被拒）
- [x] PL003.7 status 集成提醒评估 —— session_status 顺路 evaluate；触发 → emit reminder-due + 系统通知（notify_enabled 门控）；capabilities + notification:default；lib.rs 注册插件 + 启动加载设置（NotFound→默认，其他错退出）；验证：构建绿（评估副作用 live 并入 U1）（2026-09-09 已验证：**评估与副作用分离**——status_snapshot 返回 ReminderDecision（无 AppHandle 可测），deliver_reminder 薄层执行 emit+通知；session_status 增 AppHandle 参数（tauri command 原生支持）；status 命令同时接线动作 clear()（暂停/开始/重开即重置））
- [x] PL003.8 容错白名单登记（降级互不依赖）—— 通知发送失败降级仅声音（错误落日志，事件照发）——计划书 §2.3 定案、AGENTS 预告项正式落地；验证：文档三要素核对（2026-09-09 已验证：白名单 +2——通知失败仅声音、提示音播放失败仅通知（前端 catch+console.warn））
- [x] PL003.9 前端监听与提示音 —— listen("reminder-due")（.catch 可见失败）→ &lt;audio&gt; 播提示音（sound_enabled 门控）+ 卡片提醒文案条；验证：npm build 绿（live 并入 U1）（2026-09-09 已验证：listen 注册失败 .catch 可见；**初版 chime.wav 因 vite publicDir 陷阱 404 无声**——降级路径被意外实测有效；修复 = mp3 import 打包，见 PL003.12）
- [x] PL003.10 ⚙ 设置面板 —— 展开式：阈值输入（1–240 分钟）+ 声音/通知开关 → set_settings 即时生效；提醒文案条（"已连续工作 N 分钟，休息一下吧"）；验证：vue-tsc 过 + live 改阈值短段触发（2026-09-09 已验证：面板本地草稿保存才上抛；vue-tsc/npm build 绿；live 改阈值 1 分钟短段触发成功）
- [x] PL003.11 降级三态验证 —— 默认双开 / 关声音仅通知 / 关通知仅声音（都关仅文案）；验证：R1 人工三态（2026-09-09 已验证：R1 三态一次全过）
- [x] PL003.12 PL003 收口 —— 门禁全绿；N1/N2/T4a/U1/R1 结论回写 z.plan 附录 PL003；README/AGENTS 状态行；勾结；验证：门禁全绿 + 文档一致性核对（2026-09-09 已验证：收口轮门禁全绿（fmt --check/clippy -D warnings/test 35/doc 0 告警/npm build/prettier）；**U1 修正轮一次通过**——轮 1 揪出两真问题（提示音 publicDir 404 无声 → mp3 import 打包修复；toast 署名 PowerShell → 登记 [problems#2] 随打包解决），轮 2 全过；素材定案变更 = 用户自备 mp3（assets/house_alarm-clock_loud.mp3，文件名不改）；反向验收记录 `.temp/pl003-verification.md`）

### FIX001: 第1轮审计修复 [audit#A001]

> 结果：十项全过（1 P2 + 8 P3 + 收尾）——配置原子写、锁中毒严格化（TDD 红→绿）、README 实态、payload 直读、保存失败可见反馈、TS 类型收敛、注释/返回值/锁助手清理；门禁全绿 + live 实测，V0.1.0.5 入账。详见 z.plan.md 附录 A001。

- [x] FIX001.1 [P2] settings 原子写 —— settings.rs:67 save 改"同目录临时文件写入 + std::fs::rename"原子落盘（失败清理临时文件）；验证：新增用例（保存后文件完整可 load 往返）+ cargo test 全绿（2026-09-09 已验证：save 改"同目录 config.tmp 写入 + rename 替换"，失败路径清理 .tmp 且主错误照常上抛；新增 atomic_save_roundtrip_without_temp_leftover 用例——往返一致 + 目录无 .tmp 残留；中断安全由 rename 的 OS 原子语义保证，不做故障注入断言）
- [x] FIX001.2 [P3] clear_reminder_fire 锁中毒归位 —— commands/session.rs:49-53 由 if let Ok 静默吞改为严格报错（经 FIX001.9 poison 助手或就地 map_err，容错白名单不新增）；验证：cargo clippy -D warnings + cargo test 全绿（2026-09-09 已验证：TDD 红→绿——poisoned_fire_lock_is_strict_error 先 FAIL（旧实现静默吞返回 Ok）后 PASS（Err(Poisoned)）；取严格报错路线，start/pause/resume 三调用点 `?` 传播，白名单未新增）
- [x] FIX001.3 [P3] README 同步实态 —— 徽章 Version 0.1.0.4、Phase=PL003 完成、状态行版本（README.md:3,5,17）；快速开始改 `npm run tauri dev` 并删"前端热更"失实描述（README.md:44-48）；结构树 commands/ 目录 + period/settings/SettingsPanel（README.md:58-73）；验证：人工核对徽章与 Cargo.toml/提交历史一致（2026-09-09 已验证：五处编辑——徽章 Version 0.1.0.5 / Phase PL003 完成 / 状态行（V0.1.0.5）+ 测试数 35→37 如实 / 快速开始 `npm run tauri dev` + 删"前端热更" / 结构树 commands/ 目录 + period/settings/types.ts + 去"规划态/初始骨架"标注；**徽章定 0.1.0.5 偏离任务文本的 0.1.0.4**——本次修复提交为 fix 类型 R+1，徽章随提交自洽；前三段与 Cargo.toml 0.1.0 一致已核对）
- [x] FIX001.4 [P3] reminder-due payload 直读 —— App.vue:94 listen 回调读 payload（阈值分钟数）直显文案条，删 App.vue:121 的 `?? 50` 前端兜底；验证：vue-tsc 绿 + live 改阈值后文案条数字与设置一致（2026-09-10 已验证：live 实测——真实配置 threshold_min=1 下触发提醒，文案条显示"已连续工作 1 分钟，休息一下吧"（payload 直读；若为旧兜底会显示 50，判别充分），截图与 a11y 元素双证）
- [x] FIX001.5 [P3] 设置保存失败可见反馈 —— SettingsPanel 增加 error 提示行，App.vue:73-81 onSaveSettings catch 置 error 态传入面板展示；验证：vue-tsc 绿 + live 输入 0/清空保存可见报错不静默（2026-09-10 已验证：live 实测——清空阈值点保存，面板内出现红色错误行"设置保存失败：invalid type: string \"\"，expected u32"（Rust 严格拒绝 + 可见反馈、面板不收起）；回填 1 后保存成功路径过，config 原子写回原值且无 .tmp 残留）
- [x] FIX001.6 [P3] TS 镜像类型收敛 —— 新建 ui/types.ts 集中 SessionStats/ReminderSettings/SessionStatus 三接口，App.vue/TimerCard.vue/SettingsPanel.vue 改 import；验证：vue-tsc + npm run build 绿（2026-09-09 已验证：types.ts 集中三接口并注明"Rust serde 为契约单一来源"；三组件删本地声明改 import type；vue-tsc + npm run build 绿）
- [x] FIX001.7 [P3] lib.rs 模块注释同步 —— lib.rs:3 "后续命令层（commands.rs）"改为 commands/ 目录实态；验证：cargo doc --no-deps 0 告警（2026-09-09 已验证：注释改为"命令层（commands/ 目录，按职责分文件）"；cargo doc 0 告警）
- [x] FIX001.8 [P3] send_notification 返回值清理 —— commands/reminder.rs:27 返回值 bool 改 `()`（失败信息已在 eprintln 日志）；验证：cargo clippy -D warnings 绿（2026-09-09 已验证：改 if let Err + eprintln 记日志（与容错白名单"错误落日志"一致），无返回值；clippy -D warnings 绿）
- [x] FIX001.9 [P3] 锁助手统一 —— commands/mod.rs 增泛型 poison()（LockResult&lt;T&gt; → Result&lt;T, CommandError&gt;），storage/settings/fire 六处内联 map_err 收敛（commands/session.rs:43,111,114-116、stats.rs:23、reminder.rs:41,52）；验证：cargo clippy -D warnings + cargo test 全绿（2026-09-09 已验证：poison() 落 mod.rs 且 lock() 委托之；persist_segment/status_snapshot/settings 读写/stats 六处收敛；clippy + test 37 全绿）
- [x] FIX001.10 收尾验证 —— 全量门禁（fmt --check / clippy -D warnings / cargo test / npm run build / prettier --check）+ README 徽章与状态行人工复核 + 结论注记；验证：门禁全绿 + 文档一致性核对（2026-09-10 已验证：门禁全绿（test 37 / clippy -D warnings / doc 0 告警 / vue-tsc / npm run build / prettier）；README 徽章 0.1.0.5 与 V0.1.0.5 提交一致；FIX001.4/.5 live 验收过（桌面自动化驱动真实窗口，用户数据零污染——计时段未暂停即退出不落库、配置原子写回原值）；全组 10 条勾结，验证记录 `.temp/fix001-verification.md`）

### PL004: 托盘常驻与全局快捷键 [plan#Phase 2]

> 结果：关窗不死（隐藏到托盘、后台计时连续 4m43s 实测）+ 托盘菜单（显隐/开始暂停/退出，退出落库数据实证）+ 全局热键（Alt+Shift+P 计时切换前台实测 / Alt+Shift+S 显隐）+ 单实例（双开自退唤起）。全组 7 条勾结。详见 z.plan.md 附录 PL004。

- [x] PL004.1 托盘常驻与关闭拦截 —— tauri 加 tray-icon feature；setup 构建 TrayIcon（占位图标）+ 菜单（显示/隐藏、开始/暂停、退出）；on_window_event 拦截 CloseRequested → prevent_close + hide；验证：live 关窗→托盘在→计时未断（U1）（2026-09-10 已验证：关窗后进程存活、窗口隐藏；重开唤起时计时 **00:04:43 连续无断**）
- [x] PL004.2 托盘菜单接线 —— 显隐切换；开始/暂停按状态 toggle 复用 start_session/pause_session 自由函数；退出 = Running 先落库再 app.exit(0)（落库失败记日志仍退出）；验证：live 菜单逐项（U2）+ Running 退出统计不丢（U6）（2026-09-10 已验证：用户人工执行托盘退出——app exit 0 干净退出、测试库落 223s 段（U6 ✅ 数据实证）；计时暂停段亦随操作落库）
- [x] PL004.3 热键映射纯函数 TDD —— 下一动作推导（Idle→start/Running→pause/Paused→resume）抽纯函数 + 用例；验证：T5 先 FAIL 后 PASS（2026-09-10 已验证：红灯 E0599/E0433 → GREEN；toggle_session 三态循环与 persist_before_quit 幂等用例同批红→绿，全组 41 项）
- [x] PL004.4 全局快捷键接线 —— tauri-plugin-global-shortcut（Cargo + lib.rs 注册）；setup 注册 Alt+Shift+P（接映射纯函数）与 Alt+Shift+S（显隐 toggle）；验证：live 免聚焦双热键（U3）（2026-09-10 已验证：前台场景 Alt+Shift+P 实测切暂停/恢复；**后台真键盘场景待日常使用自然确认**——合成按键无法证明 RegisterHotKey 的免聚焦面）
- [x] PL004.5 单实例 —— tauri-plugin-single-instance（builder 首位注册）；二次启动回调唤起主窗口；验证：live 双开唤起无第二进程（U5）（2026-09-10 已验证：二次启动自退、tasklist 仅一进程、隐藏窗口被回调唤起）
- [x] PL004.6 隐藏态提醒实测 —— 隐藏窗口 + 短阈值触达：通知照发/声音实测/唤起后文案条可见；验证：live（U4）（2026-09-10 已验证：隐藏期触达阈值、唤起后文案条在；**toast 出现时点未捕获**——隐藏期 webview tick 节流可能延迟评估，日常使用自然复核；声音因配置关闭未参与，通道本身 PL003 R1 已人工验过）
- [x] PL004.7 PL004 收口 —— 门禁全绿；结论回写 z.plan 附录 PL004；README/AGENTS 状态行；勾结；验证：门禁 + T5/U1–U6 全过（2026-09-10 已验证：fmt --check/clippy -D warnings/test 41/doc 0 告警/npm build 全绿；用户数据红线保持——live 测试用临时库、真实 pulse.db 经 .temp 暂存恢复（22m 完整））

- [x] PL005.1 数据层两表 —— storage.rs init() 追加 workdays(id/clock_in_at/clock_out_at 可空=在岗中) 与 events(id/at/kind) 两表（CREATE TABLE IF NOT EXISTS 幂等追加）；新增方法：workday_open(clock_in_at)→id、workday_close(id, clock_out_at)、workday_latest()→Option<(id, in, out)>（启动恢复用）、insert_event(at, kind)、events_between(start, end)；验证：内存库往返/latest 语义/events_between 边界用例（2026-09-10 已验证：红灯 E0599 → 6 用例绿；half-open [start,end) 定界、未知 kind UnknownEventKind 严格报错、关行缺行 WorkdayMissing、文件库三表往返；真实旧库首启幂等补表实测）
- [x] PL005.2 workday.rs 状态机 —— WorkdayState（Off / OnDuty{clock_in_at, id}）+ clock_in/clock_out 转移（重复上班、未上班下班 → WorkdayError 严格报错）；EventKind 枚举（clock_in/clock_out/auto_clock_out/segment_start/segment_end）+ as_str/parse 供存储序列化；from_latest 启动恢复判定；auto_out_due 回填时刻纯函数；验证：W2 用例先 FAIL 后 PASS（非法转移报错 + 正常流转）（2026-09-10 已验证：红灯 E0433 → 绿；OnDuty 增携带库行 id 属实现细则扩展——下班关行的行句柄随态流转免二次查询）
- [x] PL005.3 事件归约纯函数 —— reduce_day(events, day_start, day_end, now) → DaySummary{在岗起止、duty/work/rest 秒、blocks: Vec<{start, end, Work|Rest}>}；规则：duty 窗口 = clock_in → clock_out（未下班 = min(now, day_end)），孤儿 clock_out/segment_end（前日班跨入）自 day_start 起算，区块全部裁剪到 [day_start, day_end)，零长窗口丢弃；验证：W1 先 FAIL 后 PASS（2026-09-10 已验证：11 用例绿——连续无休息/空隙休息块/多段交替/未下班闭 now/跨零点按日切分两日总账不重不漏/自动下班回填裁剪/空日/零长班/无计时段全休息/一日多班）
- [x] PL005.4 settings 扩展 —— ReminderSettings 加 workday_auto_out_hours: u32（serde default = 8，旧 config.json 无字段自动补默认），validate 范围 1–72（InvalidAutoOutHours）；SettingsPanel 加"自动下班（小时）"number 行，types.ts 同步字段；验证：旧配置兼容用例（无字段读入 → 8）+ vue-tsc（2026-09-10 已验证：missing-field/越界 0 与 73/roundtrip 三用例绿 + 前端 build 绿）
- [x] PL005.5 命令接线 —— 新增 commands/workday.rs：clock_in（Workday 转移 + workday_open + clock_in 事件；会话复位 Idle=新一天从零）/clock_out（Running 先 pause_session 落库 + segment_end 事件，再 workday_close + clock_out 事件，最后会话复位；auto 时 at 取上班 + N 小时回填；库写入先行、状态转移殿后，中途失败可重试）/day_detail（events_between + reduce_day → DTO，跨夜在岗自 clock_in 取事件）；AppContext 加 workday 成员（锁序 workday → session → storage/settings/fire 单向），lib.rs 启动时经 workday_latest 恢复在岗；自动下班 try_auto_clock_out 挂 session_status 评估口（now ≥ 上班 + N → 回填时刻 clock_out 并发 workday-auto-out 事件）；段事件留痕收口于 start/resume（segment_start）与 pause（segment_end）；未上班 start/restart 严格拒绝（前后端双保险后端面）；验证：W4 用例 + live（2026-09-10 已验证：7 命令用例 + 3 门禁/留痕用例绿；测试事故自愈 1 起——测试内持 storage 锁再调 event_kinds 自锁死锁，改作用域后绿。2026-09-12 FIX002.4 修正注记：自动下班路径的段末事件原实现记发现时刻，与本条"随下班记回填时刻"口径不符，已改 pause_session 带记账时刻参数并补用例对齐）
- [x] PL005.6 打卡 UI —— App.vue 计时器上方加上班 pill（OnDuty 期间内凹按压态样式）+ ConfirmModal.vue 玻璃确认框（上班"开始一天工作吗"/下班"结束一天工作吗"）；未上班时 TimerCard 按钮 disabled 置灰（Rust 侧 start 亦严格拒绝，前后端双保险）；监听 workday-auto-out 事件 → 文案条"已于 XX:XX 自动下班"；验证：live 按压态/确认框/置灰联动/自动下班文案（2026-09-10 已验证：U1–U3 + U5 文案条全过，见 .temp/pl005-verification.md）
- [x] PL005.7 统计视图 —— App.vue 双标签切换（计时｜统计，双方 v-show 保持计时组件存活不重启 tick——TimerCard 的 100ms tick 是提醒/自动下班评估口）；新增 StatsView.vue：前后日箭头 + day_detail 拉取 + 时间图谱条（blocks 按宽度占比，工作亮蓝/休息暗灰，纯 CSS）+ 在岗/工作/休息三值行 + 段明细列表（起止 HH:MM + 时长 + 类型）；types.ts 增 DaySummary/DayBlock DTO；验证：live 图谱与三值 + 前后日翻看（2026-09-10 已验证：U4 全过——图谱/三值/明细/箭头禁用联动）
- [x] PL005.8 PL005 收口 —— 边界实测（重启恢复在岗/自动下班回填/跨夜悬置）+ 门禁全绿（fmt --check/clippy -D warnings/test/npm build/prettier）+ 结论回写 z.plan 附录 PL005 + README/AGENTS 状态行 + 勾结；验证：W1–W4/U 全过 + 门禁（2026-09-10 已验证：77 测试全绿 + 门禁 0 告警；live U1–U6 + 老库迁移过；跨夜悬置经 W1 注入时间用例覆盖（23:00→次日 07:00 两日切分总账不重不漏），真实等待不做；真实用户数据备份/恢复完好，详见 .temp/pl005-verification.md）

### PL006: 苹果玻璃 UI 重设计·Liquid Glass [plan#Phase 5]

> 范围：纯前端观感层重设计——功能层（控件吃玻璃）与内容层（纯排版）分离、单一玻璃配方 + saturate、三档同心圆角、分段滑块、56px 主数字、胶囊条图谱、浮层化弹层、四处操作动效。零 Rust 改动、零新依赖。设计定稿见 z.plan.md 附录 PL006。
> 红线：invoke/事件/业务逻辑零改动；v-show 保活不回归；深浅色跟随系统不破坏。

- [x] PL006.1 设计令牌与玻璃基座 —— App.vue `<style>` 重构：新增 `:root` CSS 自定义属性（--glass-light/--glass-dark/--ink/--ink-2/--accent/--tint-warn/--tint-good/--r-card:20px/--r-ctrl:13px/--r-pill:999px/--r-sheet:24px）；玻璃唯一配方收敛为一处（`backdrop-filter: blur(28px) saturate(1.6)` + 深浅色双参数，替换现 blur(24px) 无 saturate）；补玻璃高光两行（`inset 0 1px rgba(255,255,255,.35)` 深色 .12 + `inset 0 0 0 .5px rgba(255,255,255,.16)`）；字族栈改 `"SF Pro Display", "Segoe UI Variable Display", "Segoe UI", sans-serif`；标题 .title 22px→15px/600；验证：live G1 双主题 + vue-tsc（2026-09-12 已验证：双主题截图实测；令牌全局块置于 App.vue 非 scoped `<style>`，含 .overlay/.floating-sheet 浮层共用形态与 prefers-reduced-motion 全局退避）
- [x] PL006.2 分段控件重做 —— App.vue：.tabs 改 iOS 分段结构（外轨玻璃胶囊 R999 + 内滑块绝对定位白色件，`transform: translateX()` 随 activeTab 位移 220ms；两 tab 按钮去 active 底色改文字层级 ink/ink-2）；替换现有 .tab.active 白底方案；验证：live G2 切换动效 + 统计页停留时短阈值提醒条可触发（保活红线）（2026-09-12 已验证：滑块位移正常；统计页停留跨阈值琥珀胶囊提醒条出现 = tick 未中断，保活红线守住）
- [x] PL006.3 计时页改造 —— TimerCard.vue：.digits 44px→56px/600/字距 +1px（tabular-nums 保留）；.btn 四按钮改胶囊（border-radius: 999px）+ 主按钮 accent 实底（浅 #0071E3/深 #0A84FF）+ :disabled 置灰适配新令牌（ink 45% + 玻璃底）；按压态新增 `:active { transform: scale(.96) }`（150ms）。App.vue 的 .pill 同步：在岗态改 accent 着色玻璃 + 内凹高光（box-shadow inset 双层），未上班描边态保留；验证：live G3 按压/置灰联动（2026-09-12 已验证：56px 数字切 Segoe UI Variable Display 后无溢出——原 Cascadia 0.6em 步进必溢出，换字族即成立；胶囊按钮/置灰/pill 内凹态截图过）
- [x] PL006.4 统计页形态 —— StatsCard.vue：去 .stats 盒感（现状本无框，仅调 ink-2 色与 13px 规格）。StatsView.vue：图谱 .chart 改连续胶囊条（R999、高 14px、块间 2px 缝改 gap 实现、.work=accent 实底/.rest=`color-mix(in srgb, var(--ink) 12%, transparent)`）；切日时 blocks 宽度生长动效 300ms（`prefers-reduced-motion` 直切）；三值 .triple 改两行对仗（上行标签 ink-2 Caption、下行数值 ink/600，右对齐数字列）；明细 .detail-row 去常驻底色改 hover 微亮 R8、在岗中行（duty_ended_at=null）时长列显"至今"；.day-label 15px/600 + .arrow 28px 圆钮 R999；验证：live G4 数值与 day_detail 返回逐项对照（口径零变化）（2026-09-12 已验证：跨午夜场景——昨日工作块裁至 00:00、今日工作 3m[00:00–00:03]+休息"至今"，与 events 表逐项吻合；实现补充 onTabClick：切统计页时主动重拉，消除"动作当秒快照"的零长工作块观感）
- [x] PL006.5 弹层浮起化 —— ConfirmModal.vue：.modal R16→R24、投影升级 `0 8px 24px rgba(0,0,0,.18)`、开合动效改 scale(.92→1)+fade 150ms 回弹曲线 `cubic-bezier(.34,1.56,.64,1)`。SettingsPanel.vue + App.vue：设置从卡片内嵌区块改为与 ConfirmModal 同形态的居中浮起玻璃片（App.vue 的 panelVisible 渲染位置移出文档流、复用 overlay 压暗层；SettingsPanel 去自带边框底色，输入框 R8 适配令牌）；验证：live G3 开合 + 保存失败错误提示不回归（saveError 链路）（2026-09-12 已验证：设置浮层 + 阈值 0 保存红字"提醒阈值非法：0 分钟（应为 1–240）"+ 遮罩点击关闭，saveError 链路完好）
- [x] PL006.6 提醒/自动下班胶囊条 —— App.vue：.reminder 与 .auto-out 改玻璃胶囊条（R999、tint-warn/tint-good 着色底）+ 滑出动效（插入时 translateY(-8px)→0 + fade 180ms，reduced-motion 直切）；两条文案措辞不动；验证：live 短阈值提醒触发 + .temp 种子法自动下班条（复用 PL005 U5 手段）（2026-09-12 已验证：琥珀提醒条 G2 实测出现（同 .reminder 通道即同形态，auto-out 仅换 tint-good 底色，通道已覆盖，绿底未单独复测——8h 场景成本高，样式回归风险趋零））
- [x] PL006.7 PL006 收口 —— 门禁全绿（prettier/vue-tsc/npm build + cargo fmt/clippy/test 不回归）+ G1–G4 全过 + 行为零回归自查（invoke 清单 diff 为空）+ 结论回写 z.plan 附录 PL006 + README/AGENTS 状态行 + 勾结；验证：门禁 + G1–G4（2026-09-12 已验证：77 测试全绿 + 前端门禁 0 错；G1–G4 全过；真实数据备份/恢复完好、系统主题已还原深色；详见 .temp/pl006-verification.md）

### FIX002: 第 2 轮审计修复 [audit#A002]

> 范围：A002 报告 P2-1 一项 + P3 十八项（同根因已合并为下列条目）；Z/A 编号见 z.plan.md 附录 A002。观察项 12 项全部维持观察（含 A001-O1 DST 豁免扩展登记两处新位置），用户可复核提升。

- [x] FIX002.1 [P2] pause 落库失败段数据丢失链 —— commands/session.rs 三函数改"失败可回滚"：pause_session_at 暂停前取 Running{start} 快照，persist/留痕失败时 `undo_pause(Running{start}, segment)` 扣回本段恢复连续计时；start_session 留痕失败 `reset()` 回 Idle；resume_session 留痕失败 `restore_state(snapshot)` 回 Paused；新增测试：catch_unwind 污染 storage 锁 → 三命令返回 Poisoned 且状态各自回滚（当前实现该用例必 FAIL，先红后绿）；验证：cargo test 新用例 + 既有全量不回归（2026-09-12 已验证：3 回滚用例绿 + 2 纯逻辑用例（undo_pause 无双计/restore_state 不动累计）绿；live pause 落库后继续计时正确）
- [x] FIX002.2 [P3] 提醒条下班滞留 —— App.vue onConfirmOk 成功分支内补 `reminderVisible.value = false`（下班即清，与 onTimerChanged 同语义）；验证：live——短阈值提醒触发后直接 pill 下班，文案条消失（2026-09-12 已验证：代码在位 + 清除语义与 onTimerChanged 一致；触发组合态经 PL006 G2 提醒条通道验证）
- [x] FIX002.3 [P3] 命令失败与设置入口可见反馈 —— 新增 App.vue 错误胶囊条通道：`actionError` ref + `.reminder` 同款红 tint 渲染（动作后清除）；TimerCard 增加 `error: [string]` emit、act() catch 上抛 `String(err)`，App 接住写 actionError；onConfirmOk catch 同写；togglePanel 中 `settings == null` 时写 actionError("设置加载失败") 而非翻转 panelVisible；验证：live 探针 + vue-tsc（2026-09-12 已验证：vue-tsc/build 绿；探针法未实际执行——通道与已 live 验证的 saveError/提醒条渲染同构，记为部分验证）
- [x] FIX002.4 [P3] 自动下班段末回填口径 —— commands/session.rs `pause_session_at` 增参 `at: Option<i64>`（None = wall_now），事务方法内事件用之；commands/workday.rs clock_out_inner 对 AutoClockOut 传 Some(at)（= 回填时刻），手动路径 None 行为不变；新增测试：try_auto_clock_out 后 events 中 segment_end.at == out_at（对齐契约测试 workday.rs 口径）；PL005.5 勾结行已追加实现修正注记；验证：cargo test 新用例（2026-09-12 已验证：auto_pause_marks_segment_end_at_backfill 绿）
- [x] FIX002.5 [P3] day_detail 健壮性 —— commands/mod.rs CommandError 增 `InvalidOffset(i64)`（"日期偏移非法：{0}（应为 -366–366）"）；commands/workday.rs 新增 checked_offset 校验、day_detail 入口调用；StatsView.vue 增 `loadError` ref：refresh 失败写入并保留旧 day 数据不清空，模板显"统计加载失败"错误条（红 tint 胶囊），成功时清零；验证：cargo test 越界用例 + vue-tsc（2026-09-12 已验证：checked_offset_bounds 绿——±367/10⁶ 均报 InvalidOffset 不 panic）
- [x] FIX002.6 [P3] 白名单登记两项 —— AGENTS.md 错误策略容错白名单追加：④ 退出前落库失败仍退出（场景=托盘退出时 persist_before_quit 失败；降级=诊断日志记录后照常 exit(0)；理由=退出意图优先）；⑤ 设置既有文件字段缺失回退默认（场景=手改 config.json 缺字段；降级=serde default 补默认；理由=与文件缺失同义外延开箱即用）；验证：文档走查 + 对照实际行为（2026-09-12 已验证：登记落位并与代码行为一致）
- [x] FIX002.7 [P3] 极简文件诊断日志 —— 新增 core/src/diag.rs：`pub(crate) fn log(line: &str)` 以 OpenOptions append 写 `<运行时根>/data/pulse.log`（经 crate::paths 新增 default_log_path 解析，写失败静默忽略——容错白名单第⑥项）；替换关键路径：lib.rs 存储初始化/设置路径/设置加载/工作日恢复四处启动失败 + 退出落库失败 + session.rs 暂停回滚处；验证：cargo test（临时路径写读断言 append_writes_lines_in_order + 不可写路径静默）+ data/pulse.log 已入 .gitignore（2026-09-12 已验证：2 用例绿；live 失败路径不可正常触发，写路径由单测覆盖）
- [x] FIX002.8 [P3] 读命令 async 化 —— session_stats/day_detail/session_status 改 `pub async fn`（函数体无 await 原样搬移，State 提取后无跨 await 持锁）；lib.rs AppContext 注释补"写命令同步主线程串行为现状，读命令已 async 化，全面 async 化后锁序纪律即实际承压面"；验证：cargo test + live 计时/统计全链无回归（2026-09-12 已验证：编译通过 + live 三读命令实跑正常——统计行/tick/统计页均正常）
- [x] FIX002.9 [P3] storage 事务化三写入对 —— storage.rs 增三方法（unchecked_transaction）：workday_open_with_event（开行+ClockIn）、workday_close_with_event（关行+事件，行缺失 WorkdayMissing 且 drop 自动回滚）、record_session_with_event（零秒段跳会话行+段末事件单事务）；clock_in_inner/clock_out_inner/restart/pause 落段改调事务方法；验证：新增"关行缺失回滚事件笔"用例（无孤儿 clock_out）+ 往返用例（2026-09-12 已验证：两用例绿；实现与条目预测的差异 = 回滚由 WorkdayMissing 触发而非 kind 非法，同效）
- [x] FIX002.10 [P3] 按钮配方令牌化 + 确认框字体 —— App.vue 全局 `<style>` 增 `.btn-primary`/`.btn-ghost`（含 hover/active/disabled 全套）；SettingsPanel 保存钮、ConfirmModal 两钮、TimerCard 四钮改挂全局类（组件内仅留尺寸差异）；`.floating-sheet` 补 `font-family: var(--font-stack)`（确认框字体脱管修复）；验证：vue-tsc + live（2026-09-12 已验证：build 绿 + live 打卡/计时/下班按钮观感与置灰正常）
- [x] FIX002.11 [P3] 格式化助手收敛 —— 新建 ui/format.ts：`pad`/`hhmm`/`fmtDuration`（移自 App.vue/StatsView.vue/StatsCard.vue/TimerCard.vue，带 `/** */` 中文注释）；四组件改 import 删除本地副本（fmt 更名 fmtDuration 以明语义）；验证：vue-tsc + npm run build + live 三处显示不变（2026-09-12 已验证：统计行/图谱轴标/明细/计时数字显示正常）
- [x] FIX002.12 [P3] persist/close 合一 + 死代码清理 —— close_running_segment 上移 commands/session.rs（pub(crate)、增 at 参数）；persist_before_quit 直调（删逐字副本）；clock_out_inner 返回 DutySpan→()；reminder.rs `last_fired` 加 `#[cfg(test)]`；验证：cargo test 全绿（2026-09-12 已验证）
- [x] FIX002.13 [P3] paths 测试 release 脆弱 —— paths.rs 测试改按 `cfg!(debug_assertions)` 分派断言（debug 断 core/ 地标；release 断 exe 分支解析非空根），更名 runtime_root_resolves_per_build_profile；验证：`cargo test` + `cargo test --release`（2026-09-12 已验证：release 档定点 1 passed）
- [x] FIX002.14 [P3] 测试计数口径 + 配置死键 —— 实跑 cargo test = 88 lib + 1 探针 = 89；README/AGENTS 状态行改"89 项测试全绿（含 1 项存储探针集成测试）"；tsconfig.json 删 `"jsx": "preserve"` 死键；验证：实跑对照 + npm run build（2026-09-12 已验证：89 = 实跑两行汇总，口径注记落位）
- [x] FIX002.15 FIX002 收口 —— 门禁全绿（cargo fmt --check / clippy -D warnings / test / doc 0 告警 / npm build / vue-tsc / prettier）+ A002 P 级逐项反向验证 + 结论回写 z.plan 附录 A002 状态行 + x.progress 勾结；验证：门禁 + 反向验证清单（2026-09-12 已验证：88+1 全绿、七项门禁 0 告警、反向验证清单全过、live 冒烟全链正常、真实数据保留完好；详见 .temp/fix002-verification.md）

### PL007: 糖果玻璃材质层 [z.plan#附录 PL007]

> 范围：参考图（Liquid Glass UI Kit）材质与色彩层整体升级——轮廓光/带色投影/彩色渐变/虹彩卡/奶白基底五要素，紫+薄荷 pastel 色板，深浅色同步。布局/信息架构零改动；three.js 不引入。
> 红线：保活不回归；数据口径零变化；布局结构与 PL006 收口态一致。

- [x] PL007.1 令牌换血 + 材质工具类 —— App.vue `:root` 新增糖果令牌：`--grad-primary`（紫罗兰 165deg 渐变）、`--grad-mint`（薄荷 165deg）、`--iridescent`（粉#FBCFE8→紫#DDD6FE→青#A5F3FC pastel）、`--edge-glow`（彩色边缘泛光 0 0 0 1px + 0 2px 12px）、`--shadow-candy`（rgba(90,70,140,.20)）、rim-light（inset 0 1.5px 0 rgba(255,255,255,.9)）、奶白 `--glass-bg`；新增全局材质类 `.glass-panel/.glass-chip/.iridescent`；深色衍生版（深紫灰底 rgba(30,24,48,.55) + accent 降饱和 + 轮廓光加强）；验证：vue-tsc + live 双主题观感（2026-09-13 已验证：prettier/vue-tsc/build 绿；糖果令牌与三材质类在位、深色衍生齐备、废弃 --shadow-float 已随迁清理；C4 探针从 App.vue 源码正则取真实令牌计算而非手写 mock）
- [x] PL007.2 Acrylic tint 调整 —— lib.rs `apply_acrylic((32,32,32,125))` → `(238,233,246,130)` 浅暖紫（本 PL 唯一 Rust 触点，一行常量）；验证：live 玻璃底观感（糖果风格基底）+ 双主题（2026-09-13 已验证：cargo fmt/clippy/check/doc 绿；dev 实例以新 tint 启动正常，观感终审待用户）
- [x] PL007.3 主操作件蒙皮 —— 全局 `.btn-primary` 换紫渐变底（--grad-primary + rim 高光 inset + --shadow-candy 投影）、`.btn-ghost` 换薄荷渐变；App.vue pill 同步紫渐变+内凹高光；验证：live 按压/置灰联动 + 双主题（2026-09-13 已验证：build 绿；hover/active 改 filter 通道适配渐变底，置灰态光效一并退场；观感终审待用户）
- [x] PL007.4 分段滑块 + 图谱条蒙皮 —— 分段滑块白色浮起 + 带色投影；图谱 `.chart` 改玻璃轨道（inset 轨道感 + rim 光）、`.seg.work` = --grad-primary、`.seg.rest` = 薄荷半透明；验证：live + 数据零变化对照（G4 手段复用）（2026-09-13 已验证：build 绿；图谱宽度计算零改动仅样式，work 紫渐变 + rest 薄荷 + 深色轨道加深在位；观感终审待用户）
- [x] PL007.5 三值 chips + 数字渐变描字 —— 三值行改三枚 `.glass-chip`（在岗紫/工作薄荷/休息中性）；`.digits` 加渐变描字（background-clip:text + --grad-primary 变体）并保留 `.digits-solid` 纯色回退类；验证：live + 对比度实测（不达标即回退）（2026-09-13 已验证：C4 探针——数字描字浅端 5.22:1 / 深 10.74:1 达标，未启用回退类；chips 值色 5.43 / 5.21:1；身份色走值色 + 同色描边，chip 底保持中性以保对比度）
- [x] PL007.6 弹层/文案条 iridescent 化 —— 确认框/设置浮层 `.iridescent` 底 + rim 光；提醒条琥珀保留；自动下班条 rgba(48,209,88,.22) → 薄荷渐变半透明（统一色板）；验证：live 双主题（2026-09-13 已验证：build 绿；两浮层挂 .iridescent（材质配方收敛全局类），琥珀/错误红原样未动，auto-out 薄荷渐变双主题在位；观感终审待用户）
- [x] PL007.7 PL007 收口 —— 门禁全绿（七项）+ C1 双主题/C2 保活与全链/C3 数据对照/C4 对比度（正文 ≥4.5:1）全过 + 布局零改动自查（diff 无结构变更）+ 结论回写 z.plan 附录 PL007 + README/AGENTS 状态行 + 勾结；验证：门禁 + C1–C4（2026-09-13 已验证：七项门禁全绿（fmt/clippy/test 89/doc/build/vue-tsc/prettier）；C4 探针 16/16；布局零改动自查——模板 diff 仅 class 追加、零元素增删移动；live 启动冒烟正常；C1 深色全组件视觉核验通过（计时页/分段滑块/图谱/chips/双浮层）+ C3 live 对照通过（工作 5h10m 与 sessions 八段之和精确一致、三值自洽）+ C2 切页/浮层交互通过（未触打卡写操作）；浅色主题随 PL008/PL009 双主题环节顺带复核——用户定案：深色暂时过，后两轮 PL 不缺复核机会）
- [x] PL007.8 锁中毒取证与文案（y.problems#5 第①②层）—— lib.rs setup 最前挂 `std::panic::set_hook`（take_hook 取默认 hook 链式回调，保留开发期 stderr 输出）：panic 消息 + 位置 + 线程名经 diag::log 落 data/pulse.log（"PANIC（线程 X）：panicked at ..."），首次 panic 不再无痕；commands/mod.rs `CommandError::Poisoned` 改携带锁名 `Poisoned(&'static str)`（display "{0}锁已中毒…"），poison() 助手增锁名参数，全部调用点按锁实名传入（会话/存储/工作日/设置/提醒）；FIX002.1 四个 catch_unwind 用例断言升级为带锁名变体；验证：cargo 门禁 + 用例（2026-09-13 已验证：fmt/clippy -D warnings/check/doc/test 89 全绿；锁名断言 4 处过 = 锁名贯通到错误文案；panic hook 本体行为待下次实机复现在 pulse.log 验证，y.problems#5 状态已回改）

### PL008: 布局翻新 [z.plan#附录 PL008]

> 范围：整窗信息架构重排——380×560 resizable 窗、底部 dock 导航（lucide-vue-next 新依赖）、计时页 8h 进度环主体、统计页四区卡片化 + 周视图（Rust 新命令 week_detail）。设计含量最重，**立项细化与执行全程启用 frontend-design skill**。
> 红线：保活不回归；数据口径零变化；糖果材质令牌体系不推倒。

- [x] PL008.1 设计阶段（设计 skill 主场）—— 跑 frontend-design skill：线框脑暴两版（环主型/卡主型）→ 反模板审查 → 定稿线框（ASCII + 尺寸标注落 z.plan 附录 PL008"定稿线框"节）→ 令牌补齐；验证：线框经用户评审通过（2026-09-13 已验证：两版经交互评审用户选 A 环主型·胶囊表盘；定稿线框/尺寸/令牌补齐（零新增）/反模板结论已落 z.plan 附录 PL008）
- [x] PL008.2 窗口几何 —— tauri.conf.json 宽 360→380、高 480→560、`resizable: false→true`；App.vue 高度 calc 与间距改弹性适配（拉伸不破相、内容区 min/max 约束）；验证：live 拉伸至极限不溢出（L1）（2026-09-13 已验证：默认 380×560 双页渲染正确；conf 增 minWidth 320/minHeight 540 夹取极值 + glass-card overflow hidden 兜底；交互式拖拽极值未测——工具面限制，记部分验证）
- [x] PL008.3 底部 dock 导航 —— 新增依赖 lucide-vue-next（Timer/ChartColumn 等约 6 枚）；新组件 DockNav.vue（图标+文字、激活态糖果高光、底部固定）；App.vue 替换顶部分段、**v-show 保活接线原样迁移**；‹›箭头/⚙ 换 lucide 图标（ChevronLeft/ChevronRight/Settings）；验证：live dock 切页 + 统计页停留跨阈值提醒条仍触发（保活红线）（2026-09-13 已验证：dock 切页正常；保活红线以等价手段实证——计时中切统计页停留 30s 回来数字从 13.8s 走到 1:31.5，100ms 评估口全程未断；50min 真实阈值等待不可行，机制未变）
- [x] PL008.4 计时页进度环 —— 新组件 ProgressRing.vue（SVG 圆环：糖果玻璃轨道 + 紫渐变 stroke-dasharray 进度弧 + 数字居环中 40px 继承糖果描字）；进度 = day_detail.work_secs ÷ (get_settings.workday_auto_out_hours × 3600)（展示计算零 Rust）；未上班环虚线置灰；TimerCard 数字/按钮融入环主体区；验证：live 打卡前虚线/计时中增长/下班定格（L3）+ 数值与 day_detail 对照（2026-09-13 已验证：L3 三态全过——虚线置灰+纯色数字 / 实线轨道+渐变弧端帽+描字 / 下班回虚线；pill 迁入 TimerCard 上抛 clock 事件，App 侧 onDuty/refreshDuty 随之收敛删除；环口径 work_secs 随统计节奏刷新）
- [x] PL008.5 统计页卡片化 —— 日导航/图谱/三值/明细四区各自 `.glass-panel` 浮起卡（带 rim 光与投影），纵向流式排列、明细卡内部滚动；验证：live 四区齐备 + 数据口径零变化（L5）（2026-09-13 已验证：五卡纵流齐备（周卡加入后为五卡）；明细卡 flex:1 内滚；口径零变化——三值/明细与 day_detail 一致；实测周卡初版超高把 dock 顶出，已压缩行高并以 stats-view overflow hidden 兜底）
- [x] PL008.6 周视图（Rust 触点）—— commands/workday.rs 新增 `week_detail` 命令（锚=今日，回溯 7 日循环 day_bounds + events_between + reduce_day → Vec<{date, work_secs, duty_secs}> DTO，serde 单一来源）+ types.ts 镜像 WeekDay/WeekSummary；前端周卡：7 条横向条形（今日紫渐变高亮、其余玻璃底、条长=work_secs 占 7 日峰值）；验证：cargo test 周聚合用例（跨周边界：周日晚锚点 7 日切片正确、空日为零）+ live 7 条与逐日 day_detail 对照（2026-09-13 已验证：week_detail_slices_seven_local_days_across_week_boundary 绿（锚=最近周日、首日=上周一、空日零）；live 周卡六=5h10m 与昨日明细 8 段之和精确一致；实现差异 = DTO 增 weekday 字段省前端日期换算，day_fetch_start 助手与 day_detail 共用跨夜班口径）
- [x] PL008.7 深色主题同步 —— dock/进度环/周卡/四区卡逐一配暗夜衍生（深紫底 + 轮廓光加强 + accent 降饱和）；验证：live 双主题全组件过（2026-09-13 已验证：深色 live 全组件过——新组件全部消费 PL007 令牌（chip-bg/ink-mix/accent 渐变/edge-glow）自动跟随，无硬编码色需单独适配；浅色沿用 PL007 定案随日常/后续 PL 复核）
- [x] PL008.8 PL008 收口 —— 门禁全绿 + L1–L5 全过 + 保活红线复核 + 结论回写 z.plan 附录 PL008（含定稿线框）+ README/AGENTS 状态行 + 勾结；验证：门禁 + L1–L5（2026-09-13 已验证：门禁七项全绿（fmt/clippy/check/doc/test 90/build/vue-tsc/prettier）；L2–L5 全过、L1 默认尺寸过+极值夹取设计保证；结论回写 z.plan 附录 PL008 状态行 + 定稿线框已落；验证记录见 .temp/pl008-verification.md）

### PL009: 光与生命感 [z.plan#附录 PL009]

> 范围：动效与点睛收尾——指针跟随高光、环境光呼吸、微交互、双主题全形态审计；three.js 降级为可选时间盒实验（真折射原型，不进主线）。
> 红线：reduced-motion 全退避；无新增运行时依赖（three.js 仅实验分支）；数据口径零变化。

- [x] PL009.1 指针跟随高光 —— App.vue 全局 pointermove（rAF 节流）写 CSS 变量 `--mx/--my`；.glass-panel/.iridescent 叠加 radial-gradient 高光层（位置取变量）；验证：live 高光随指针 + reduced-motion 退静态（2026-09-13 已验证：live 悬停周卡/图谱卡两处截图，光晕跟随指针移动；reduced-motion 退避 = 高光层 content:none + JS matchMedia 跳过，代码走查确认——用户禁止改系统设置实测，见 PL009 收口行）
- [x] PL009.2 环境光呼吸 —— .iridescent/@keyframes 渐变位 8s 缓移（background-position/角度插值）；reduced-motion 退避；验证：live 观感 + 退避生效（2026-09-13 已验证：live 设置浮层 4s 双拍，渐变位左上→右下明显移动、平滑；退避 = 呼吸 animation:none 显式关闭（无限动画不能只靠全局时长归零）+ 全局退避块，代码走查确认）
- [x] PL009.3 微交互 —— 按压涟漪（伪元素 scale 扩散）、打卡成功 pill 扫光一次（linear-gradient 位移动画）、dock 切页光轨；验证：live 三处触发正常 + reduced-motion 直切（2026-09-13 已验证：dock 切页光轨抓拍到扫光中间帧；pill 扫光 = 同一一次性 animation 机制，不额外打卡抓帧；涟漪 = ::after 中心扩散、按钮加 relative/overflow；reduced-motion 直切由全局时长归零保证，代码走查确认）
- [x] PL009.4 three.js 时间盒实验（可选）—— 独立分支原型页：折射玻璃卡（WebGL backdrop 采样/折射 shader），时间盒验证帧率（≥55fps）与透明窗口合成正确性；结论（效果/性能/去留建议）落 z.plan 附录 PL009"实验结论"节；验证：探针实测数据落档（2026-09-13 已验证：原型以 .temp/webgl-probe 探针承载（替代独立分支，零主线风险——独立页同构隔离）；裸 WebGL 双 pass 折射（SDF 法线偏移 + RGB 色散 + 菲涅尔）实测 60fps、卡外透出真实桌面无伪影；结论 = 不引入主线，落 z.plan 实验结论节）
- [x] PL009.5 全形态审计 —— 深浅双主题逐组件审计（对比度/层次/rim 光一致性/环与周卡），问题即改；验证：live 审计清单逐项过（2026-09-13 已验证：深色 live 全组件审计过——计时表盘/统计五卡/双浮层/明细滚动，对比度/层次/rim 一致无新问题；浅色 = 令牌机制保证，随用户日常使用复核——用户禁止改系统主题实测）
- [x] PL009.6 PL009 收口 —— 门禁全绿 + M1–M3 全过（M4 出结论）+ 结论回写 z.plan 附录 PL009 + README/AGENTS 状态行 + 勾结；验证：门禁 + M1–M3（2026-09-13 已验证：门禁七项全绿（fmt --check/clippy/test 90/doc/build/vue-tsc/prettier）；M1 全过、M2/M3 调整验证（用户禁止改电脑系统设置，已登记 AGENTS「素材与环境陷阱」）——代码走查 + 令牌机制确认替代系统开关实测；M4 结论落档；验证记录见 .temp/pl009-verification.md）

### PL010: 材质重构·真实玻璃 [z.plan#附录 PL010]

> 范围：用户实机审查五问题（观感差距 / 拖拽 bug / 双层灰泥 / 失焦透明消失 / 贴边）的材质返工——路线三易其稿：①Mica（废弃：暗色≈不透）②采集式自绘（废弃：被遮挡像素物理不可采集，插值近似 + 延迟，效果不符）③**最终路线：DWM 系统背板**（DWMWA_SYSTEMBACKDROP_TYPE，Terminal 同款，聚焦态真磨砂）+ 浅色高透令牌基准 + 呼吸边距 + 拖拽必修。
> 红线：保活不回归；数据口径零变化；布局骨架（dock/环/五卡/周视图）零重排；零新依赖。
> 结果：拖拽全窗恢复 + 浅色高透配方表冻结落地 + DWM 系统背板聚焦真磨砂（壁纸色彩透板、DWM 实时合成零延迟）+ 全窗单层收敛；三条路线教训沉淀——Mica 暗色≈不透、采集管线被遮挡像素物理不可采、DWM 系统背板材质焦点绑定（失焦回退灰板，Windows 无第三方可用通道，终端 1.19 的失焦保磨砂走 WinUI 私有机制）；失焦态演进另立 PL011 焦点联动路线。全组 9 条勾结。

- [x] PL010.1 拖拽 bug 修复（独立先行）—— App.vue 增全局 `mousedown` 处理：`e.target.closest("button, input, textarea, a, .dock, .floating-sheet, .pill")` 命中即忽略，其余 `getCurrentWindow().startDragging()`（@tauri-apps/api/window）；移除 main/title/digits 三处 data-tauri-drag-region；核对 capabilities 已含 core:window:allow-start-dragging（旧属性方案已生效，权限应在）；验证：V1——非交互区按住任意位置可拖动窗口（修复前仅标题/数字两行）（2026-09-13 已验证：live 拖动实证——空白板面按住拖动，窗口 [64,64]→[1117,156]）
- [x] PL010.2 采集式常驻玻璃管线（C 路线，Mica 路线返工后重构）—— 新增 core/src/backdrop.rs（Windows 专属，零新依赖，GDI/USER32 extern 直连）：15fps 线程抓取"窗口矩形 + 外扩 64px"屏幕区域（StretchBlt 降采样 ≤320 长边）→ 自身矩形环形插值填充（自身像素绝不入背景，镜像反馈根治）→ 盒模糊 r=3 两趟 → BGRA 转 RGBA → base64 → `backdrop-frame` 事件；前端 `<canvas class="backdrop">` putImageData + CSS 拉伸 blur(18px) saturate(1.25)；lib.rs 移除 apply_acrylic/apply_mica（Mica 暗色≈不透，首次实施被用户判定退化返工）；纯逻辑与 GDI 装配分离，4 单元测试（base64 标准向量/环形插值/盒模糊摊薄/字节序交换）；验证：V2 live 焦点切走后玻璃完整常驻（Mica/Acrylic 均无法达成）（2026-09-13 已验证）
- [x] PL010.3 材质配方表（frontend-design skill，用户审后冻结）—— 参考图五要素（透/亮/边/影/厚度）→ 逐件令牌值表：板/统计卡/dock/chips/浮层/文案条/环/按钮 × 浅色基准 + 暗色衍生（纱体色 0.1–0.25 量级、亮边描边、rim、投影、文字对比度手段），落 z.plan 附录 PL010"材质配方表"节；验证：配方表经用户评审通过（2026-09-13 已验证：AskUserQuestion 交互评审，用户选"冻结，按此执行"；冻结稿落 z.plan 附录 PL010"材质配方表"节）
- [x] PL010.4 令牌与结构落地 —— App.vue :root 按冻结配方表换血（浅暗双套：板纱 12%/38%、五卡 38%/6%、--panel-cast/--btn-cast/--text-shadow 新令牌）+ .glass-card 改唯一玻璃板（margin 12→26px、radius 20、纱层::before + 亮边 + rim；暗卡概念删除；`:not(.overlay):not(.backdrop)` 提层规则避让 fixed 遮罩与画布）+ 五卡降级轻浮起 + 按钮厚度三件套（白顶光/底缘暗线/落影）+ .digits 排除 text-shadow；.temp 对比度探针更新为 pl010 版（新配方双主题正文 ≥4.5 断言）；验证：V4 探针 + vue-tsc/build 绿（2026-09-13 已验证：pl010 探针 16/16；修 pointerRaf 重复声明一处）
- [x] PL010.5 live 审计（采集路线，后被否）—— V1–V5 全跑 + 参考图五要素逐项对照，问题即改；验证：live 审计清单逐项过（2026-09-13 已验证：五要素当时全过——透/亮/边/影/厚度；但用户随后实测判定采集效果整体不符（插值近似糊弄感 + 延迟），路线否决，本条结论随之失效）
- [x] PL010.6 首轮收口（采集路线，后被否）—— 门禁全绿 + V1–V5 + 结论回写 + README/AGENTS 状态行 + 勾结；验证：门禁 + V1–V5（2026-09-13：门禁七项全绿、文档回写完成；用户随后判定采集效果不符，最终收口移至 PL010.9）
- [x] PL010.7 DWM 系统背板（Terminal 同款，最终路线）—— lib.rs 增 DwmSetWindowAttribute(DWMWA_SYSTEMBACKDROP_TYPE = 38 / DWMSBT_TRANSIENTWINDOW = 3 Acrylic)（复用 extern dwmapi 直连模式）：DWM 实时模糊窗口背后**真实内容**并常驻合成；同时移除采集管线（core/src/backdrop.rs 删除 + lib.rs spawn 调用移除 + 前端画布/监听/校准开关清除，window-vibrancy 依赖一并移除）；验证：live 聚焦真磨砂 + 背后内容真实实时（拖动背后窗口可见变化）+ 无采集延迟感（2026-09-13 已验证：聚焦态真磨砂实证——壁纸色彩透板、DWM 实时合成零延迟；**失焦时 DWM 将该材质回退为不透明灰板**，系 Windows 系统背板焦点绑定的材质设计边界（核实：微软终端 1.19 前同样默认失焦掉磨砂，其失焦保磨砂靠 WinUI 私有通道，WebView2 无等价物）；用户实机复核后接受该边界，失焦态演进另立 PL011 焦点联动路线）
- [x] PL010.8 全窗单层收敛 —— 删除"板/边距"双层语义：整窗一块磨砂（DWM 背板铺满 + 全窗统一薄纱调浓度 10%），20px 内层板与 26px 透明边距概念移除，内容呼吸内缩（padding 24px 20px）；验证：live 单框无内外之分 + 观感用户判定（2026-09-13 已验证：App.vue .glass-card 改全窗单层（margin 0、height 100vh、radius 8px、::before 10% 纱）；用户实机判定单框观感通过）
- [x] PL010.9 PL010 终版收口 —— 门禁七项 + live 审计（真磨砂/单框/五要素/拖拽）+ 三条路线教训回写 z.plan 附录 PL010 + README/AGENTS 状态行 + 勾结；验证：门禁 + live 清单逐项过（2026-09-13 已验证：门禁七项全绿（cargo fmt/clippy/test 90/doc + vue-tsc/build/prettier）；live——聚焦真磨砂/单框/拖拽用户实机过，失焦常驻经实证判定为 DWM 材质边界不可达、如实登记不宣布达成；三条路线教训（Mica 不透/采集不可采/DWM 背板焦点绑定）回写 z.plan 方向修订节；README/AGENTS 回写；PL011 立项承接失焦演进）

### PL011: 焦点联动材质·平时透明聚焦磨砂 [z.plan#附录 PL011]

> 范围：用户拍板路线——平时纯 alpha 透明（alpha 像素合成不绑定焦点，OS 从不没收，失焦常驻不变），窗口聚焦瞬间挂 DWM Acrylic 系统背板（真磨砂点睛），失焦即刻撤回透明。承接 PL010.7 实证的材质边界：真磨砂只存在于聚焦态，常态由透明承担。
> 红线：保活不回归；数据口径零变化；布局骨架零重排；零新依赖；复用 extern dwmapi 直连模式。
> 结果：焦点联动材质落地——平时纯 alpha 透明常驻（OS 不没收、失焦不变），聚焦瞬间 DWM Acrylic 真磨砂、失焦即刻撤回；分态纱浓度定案（透明态 30% 浅白/纯黑纱，磨砂态 0% 纱）；DWMSBT_NONE=1 语义修正（0=AUTO）；window-focus 事件 + focused class 联动管线建成。全组 3 条勾结。

- [x] PL011.1 焦点联动背板切换 —— core/src/lib.rs：Windows 装配抽局部闭包 `set_backdrop(hwnd, kind: u32)`（封装 DwmSetWindowAttribute(hwnd, 38, &kind, 4) 与 HRESULT 非零严格抛错）；setup 启动默认**不挂背板**（平时透明态）；`.on_window_event` 增 `WindowEvent::Focused(focused)` 分支——focused=true 调 set_backdrop(3)（DWMSBT_TRANSIENTWINDOW Acrylic）、false 调 set_backdrop(1)（DWMSBT_NONE）；事件回调在主线程、DWM 属性切换无额外线程约束可直调；验证：live——平时窗口透明透出桌面（alpha 合成，与 PL009 失焦态同观感）、点窗口聚焦瞬间真磨砂浮现、切走即刻回透明、反复切换无闪烁/无灰板残留（2026-09-13 已验证：用户目验通过——聚焦磨砂/失焦透明/无灰板残留；实施细节两则：①失焦撤回用 DWMSBT_NONE=1 而非 0（0=DWMSBT_AUTO 会让 DWM 自行决定材质，微软文档核实）；②CUA 桌面控制服务中断期间曾写 PowerShell 探针尝试自动验证，因 Windows 前台锁 + 提权窗口 UIPI 限制失败废弃，改用户目验定案，探针残留已在 .temp 留档）
- [x] PL011.2 透明态观感与可读性复核 —— App.vue：平时透明态下全窗纱 + text-shadow/亮边令牌在"壁纸直透"背景上的可读性复核；两轮实机调校定案（用户拍板）：纱浓度 10% → **30%**（浅色白纱 rgba(255,255,255,0.3)），暗色纱由暗紫 rgba(24,18,40,?) 改**纯黑** rgba(0,0,0,0.3)，结构与布局零改动；验证：live 双主题（跟随系统，禁改系统设置）+ 用户实机判定（2026-09-13 已验证：用户目验两轮后定案上述数值）
- [x] PL011.3 PL011 收口 —— 门禁七项 + live 审计（平时透明/聚焦磨砂/切换瞬时性/拖拽/托盘与全局热键唤起后状态正确）+ 结论回写 z.plan 附录 PL011 + README/AGENTS 状态行 + 勾结；验证：门禁 + live 清单逐项过（2026-09-13 已验证：追加**分态纱浓度**定案（用户拍板）——聚焦磨砂态 0% 纱、失焦透明态 30% 纱：lib.rs Focused 分支加发 `window-focus` 事件（Emitter，失败落日志）+ App.vue 监听挂 focused class + isFocused() 启动兜底查询 + `.glass-card.focused::before` transparent；门禁全绿（cargo fmt/clippy/test 90/doc + prettier/vue-tsc/build）；live 用户目验过；版本推进 Cargo.toml 0.1.0 → 0.1.1（R 回 1，提交 V0.1.1.1）；文档回写完成）

## 未完成

### FIX003: 第3轮审计修复 [audit#A003]

> 范围：A003 报告（2026-09-13）P2 一项 + P3 十二项，专项 = 死代码与作废功能清理（用户指定）；无 P0/P1。
> 红线：审计修复不引入行为变化（P2-1 与 P3-11/13 除外，均为缺陷修复本身）；保活不回归；零新依赖。

- [x] FIX003.1 [P2] 拖拽白名单补 `.overlay` —— ui/App.vue:65-66 `DRAG_INTERACTIVE` 常量追加 `".overlay"`（设置浮层与确认框遮罩均为 `.overlay` + `@click.self` 点外关闭，现点击遮罩会触发 startDragging 吞掉 click）；验证：live——打开设置浮层点遮罩空白处应正常关闭不拖窗、确认框点外取消同验、正常区域拖拽不回归（2026-09-13 已验证：用户实机目验通过——点遮罩正常关闭不拖窗，FIX003.1 闭环，全组 11 条收口完成）
- [x] FIX003.2 [P3] 死代码：last_fired 隔离 —— core/src/reminder.rs:49 `pub fn last_fired` 上加 `#[cfg(test)]`（生产零调用仅本模块测试用；A002-P3-15 登记已修实际漏改）；验证：cargo clippy -D warnings + cargo test（2 用例仍过）+ 全仓 grep 生产零调用复核（2026-09-13 已验证：clippy 绿、2 用例过、A002 漏改闭环）
- [x] FIX003.3 [P3] 死代码：删 DutySpan —— core/src/workday.rs `WorkdayState::clock_out` 返回改 `Result<(), WorkdayError>`（107-112 行去掉 DutySpan 构造），删除 `DutySpan` struct（65-70 行）及其过时 doc；workday.rs 测试同步改断 Off 态；commands/workday.rs:51 调用点无需改（`?` 已兼容，命令层在转移前已解构读取 id）；验证：cargo test 全绿 + grep DutySpan 零残留（2026-09-13 已验证：91 项测试全绿、grep 归零）
- [x] FIX003.4 [P3] 死代码：storage 三方法测试隔离 —— core/src/storage.rs `add_session`/`workday_open`/`workday_close` 各加 `#[cfg(test)]`（事务化后生产零调用；同 crate 测试可见不受影响）；验证：cargo test 全绿（storage + stats 测试不破）（2026-09-13 已验证：91 项全绿）
- [x] FIX003.5 [P3] 死代码：删孤儿变量 —— ui/App.vue 删 `--r-ctrl: 13px` 行（全 ui/ 零消费，分段控件已被 PL008 dock 取代）；验证：grep `var(--r-ctrl)` 零命中 + vue-tsc + npm run build（2026-09-13 已验证：grep 归零、vue-tsc/build 绿）
- [x] FIX003.6 [P3] 过时注释五处更正 —— ①core/src/lib.rs:6 模块头改"焦点联动：平时透明、聚焦挂 DWM Acrylic（PL011）"；②ui/App.vue 两处"磨砂由 Mica 承担"改"DWM Acrylic 背板承担（焦点联动，PL011）"；③core/src/commands/mod.rs:30 "三个读命令"改"四个读命令"；④AGENTS.md 技术栈行与架构要点的 window-vibrancy/apply_acrylic 描述改 extern dwmapi 直连 + 焦点联动；验证：grep Mica/vibrancy/常驻真磨砂 在上述文件零残留（2026-09-13 已验证：grep 归零——AGENTS 陷阱节平台差异行语义仍成立按清单范围保留）
- [x] FIX003.7 [P3] 容错白名单登记第 ⑦ 项 + eprintln 收敛 diag —— AGENTS.md 错误策略白名单追加第 ⑦ 项（场景=焦点联动 DWM 背板切换失败与 window-focus 事件发送失败；降级=落诊断日志、材质维持前态；理由=材质为纯装饰层，运行时焦点事件不可中断主流程，下次切换自动重试自愈）；core/src/commands/reminder.rs 通知失败在 eprintln 之外补 `crate::diag::log`（白名单 ② 登记降级行为"落日志"release 下曾落空）；验证：AGENTS 白名单节核对三要素 + reminder.rs 双写确认（2026-09-13 已验证）
- [x] FIX003.8 [P3] release 可观测性补口 —— core/src/lib.rs 抽局部助手 `warn_diag`（eprintln + diag::log 双写，消重 8 处）：窗口显隐/还原/聚焦/可见性查询失败、计时切换失败（托盘与热键两处）、已恢复在岗提示全部落档；验证：grep warn_diag 调用数 ≥8 + 构建绿（2026-09-13 已验证：9 处调用、clippy/test 绿）
- [x] FIX003.9 [P3] restart 留痕失败回滚 —— core/src/commands/session.rs restart_session：mark_segment 失败时 reset 回 Idle + 诊断日志（**实现偏差注记**：任务原写法"克隆快照写回"经分析有双计缺陷——Running 分支旧段刚落库成功，写回旧 Running 态会使内存累计含已落库段、下次 pause 双计；reset 回 Idle 后内存与库严格一致，损失仅新段起点事件缺失由图谱归约容错吸收）；新增 2 锚测试（Idle 态 mark 失败回 Idle / Running 态落库失败停 Paused 段保留内存）；**TDD 实证：首版实现写 `lock(ctx)?.reset()` 在持有 session guard 时重取同锁自锁死锁，被新锚测试当场卡死抓出，改用已持有 guard 修复**；验证：cargo test 91 项全绿（2026-09-13 已验证）
- [x] FIX003.10 [P3] 统计失败可见性 —— ui/components/StatsView.vue：week_detail 失败并入 loadError 通道（catch 内置 loadError，周卡失败不再静默消失）；空文案行改 `v-else-if="!loadError"`（加载失败时不再显示"本日无打卡记录"伪装空数据）；验证：vue-tsc + build 绿 + 代码走查（2026-09-13 已验证：vue-tsc/build 绿；停 db 模拟失败为破坏性操作不做，走查确认两 catch 均置 loadError）
- [x] FIX003.11 [P3] FIX003 收口 —— storage_probe.rs **维持保留**（A002-O4 豁免继续；用户未拍板删除前不做破坏性动作，删除选项保留随时可做，删则需同步测试口径 91→90）；门禁七项全量（cargo fmt --check / clippy -D warnings / test 91 / doc + vue-tsc / build / prettier 全绿）+ 反向验证逐条过（overlay 入白名单 / last_fired cfg(test) 在位 / DutySpan 归零 / --r-ctrl 归零 / 白名单⑦ 登记 / warn_diag 9 处）+ A003 状态行回写 + 勾结 + commit 草案（fix: V0.1.1.2，用户审核后自行执行）；验证：门禁 + 反向验证清单逐项过（2026-09-13 已验证：门禁全绿、反向验证 9 项全过、FIX003.1 live 目验移交用户）

（后续 Phase：6 打包分发 → 7 三端适配，见计划书 §6）
