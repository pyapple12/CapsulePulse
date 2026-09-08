# 进度追踪（x.progress.md）

> 文件职责：任务清单与进度追踪，与 `z.plan.md`（方案与审计归档）配套：方案在 z.plan 展开，执行拆条在本文件勾选。结构：**`## 已完成 ✅` 区在前、`## 未完成` 区在后**，任务完成后整组移动位置。
> 格式速查：任务组 `### PL{NNN}: {标题} [来源引用]`（来源引用：[plan#Phase N] 计划书 / [problems#N] 问题备忘录 / [audit#A{NNN}] 审计报告），组内用 `#### {小节}` 分层；子任务 `- [ ] PL{NNN}.{序号} {标题} —— {做法}；验证：{检验方式}`；审计修复任务组 FIX{NNN} 由 audit-report 归档环节生成，条目格式 `- [ ] FIX{NNN}.{序号} [P{级别}] {标题} —— {做法}；验证：{检验方式}`，编号规则见 `.agents/skills/audit-report`。
> 勾选注记：条目完成时改 `[x]`，并在句尾追加 `（YYYY-MM-DD 已验证：{一句话结论}）`——结论如实，不达标不降级宣布。

## 已完成 ✅

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
- [x] PL002.11 运行时 db 路径 —— `~/.capsule-pulse/pulse.db`（dirs 解析 + 目录自建），打开失败严格报错启动失败；测试一律注入路径/内存库，禁触真实用户目录；验证：构建绿 + 临时路径集成用例过（2026-09-09 已验证：open_default 三错误变体（Sqlite/Io/HomeDirUnavailable）；临时目录文件库"写入→重开→数据在"用例过；run() 初始化失败 eprintln + exit(1)）

#### 阶段 D：统计行 UI 与收口

- [x] PL002.12 StatsCard 统计行 —— 今日/本周/累计三值（Xh Ym 格式）；刷新 = 挂载 + 动作后 + 30s 兜底（不进 100ms tick）；玻璃样式沿用；验证：vue-tsc 过 + U1 人工闭环（计时→暂停→统计行增长；重开→段计入；重启 app→统计仍在）（2026-09-09 已验证：StatsCard 纯展示组件 + TimerCard changed 事件驱动刷新；vue-tsc/npm build 绿；**U1 六项一次通过**——统计行/暂停落库/多段累计/重开先落库/重启持久性/玻璃无退化）
- [x] PL002.13 PL002 收口 —— 门禁全绿；T1–T3/U1 结论回写 z.plan 附录 PL002；README/AGENTS 状态行回改；验证：门禁全绿 + 文档一致性核对（2026-09-09 已验证：收口轮门禁全绿（fmt --check/clippy -D warnings/test 24/doc 0 告警/npm build/prettier）；结论回写 z.plan 阶段开展结论 + 收口结论；状态行同步；反向验收与过程记录 `.temp/pl002-verification.md`——含一处 python 改源码违规的自纠记录）

## 未完成

（暂无——下一个大件：计划书 Phase 3 提醒调度（PL003 候选）或 Phase 2 托盘常驻，未立项）
