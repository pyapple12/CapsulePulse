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

## 未完成

（暂无——Phase 5 打包分发未立项）
