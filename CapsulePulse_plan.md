# CapsulePulse — 项目计划书

> 玻璃质感的工作计时看板：开始/暂停记录工作时间，超时声音+通知提醒休息。三端（W/M/L），Tauri 2 + Rust。
> 文档状态：2026-09-10 按实际落地全量回写（完成项勾选、定案偏差更新、新增设计补录）；后续偏差仍随 z.plan.md 附录留档，本文件保持与实现同步。

## 1. 项目概述

CapsulePulse 是一款极简的工作计时看板应用：

- **核心交互**：一个大的开始/暂停按钮，记录工作时间
- **统计展示**：今日总时长、本周累计、历史累计（纯时长聚合，v1 不做标签/项目）
- **休息提醒**：连续工作达到阈值（默认 50 分钟，可配置）→ 声音 + 系统通知
- **UI**：玻璃质感（vibrancy/Acrylic/blur 三端方案），当前 Windows Acrylic 实测通过，macOS/Linux 延后（problems#1）
- **常驻**：托盘常驻 + 关闭隐藏到托盘 + 全局快捷键 + 单实例（PL004 已落地）

系列定位：CapsuleRetro（包装游戏）→ CapsulePlan（落定计划）→ **CapsulePulse（记录时间）**。

### 项目目标（除产品外）

- **学习 Rust**：全部业务逻辑放 Rust 侧（状态机、统计、持久化、提醒调度），前端只做展示——Rust 占比高、可单测
- 用户背景：Python 熟练，Rust 新手（已确认 `use`≈`import`、`fn`≈`def`、`pub`=公开等对应关系，接受 Rust 学习曲线）

## 2. 核心设计

### 2.1 计时状态机（纯 Rust，可单测）

```
SessionState:
  Idle                    未开始
  Running { start }       进行中（Instant 计时）
  Paused { elapsed }      暂停（累计时长保留）
```

- `start / pause / resume / total()` 四操作，无外部依赖，`cargo test` 直接测
- 暂停后 `total()` 返回累计值；恢复时 `Instant::now() - elapsed` 继承累计
- ✅ 已落地（PL001）：实现演进两笔——`reset()` 第五操作（重开归零）；内部"段起点/段累计"分离，`pause()` 返回本段时长（落库取数源）

### 2.2 数据模型（SQLite，rusqlite）

```sql
CREATE TABLE sessions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    started_at INTEGER NOT NULL,   -- Unix 秒
    seconds INTEGER NOT NULL       -- 会话时长
);
```

- 存储位置（2026-09-10 翻案）：双落址——config.json 落 `configs/`、pulse.db 落 `data/`；dev=项目根、release=exe 同级（杜绝机器用户目录，paths.rs 解析）
- v1 查询：今日 SUM、本周 SUM、全部 SUM
- 会话落库时机：暂停 / 重开 / 退出前（Running 先落库）各写入一条（零秒段跳过，崩溃最多丢当前段）

### 2.3 提醒调度

- 阈值可配置（默认 50 分钟，设置界面修改，持久化）
- 触发：`tauri_plugin_notification` 系统通知 + 声音播放；达阈值后每 5 分钟重发；卡片文案条为第三通道
- 声音：前端 `<audio>` 播放提示音（采用用户自备 mp3 经打包内置；Rust 侧 rodio 备选未启用）
- 降级：通知不可用（部分 Linux）时仅声音；反之亦然
- ✅ 已落地（PL003）

### 2.4 玻璃效果（window-vibrancy crate）

| 平台    | 实现                                                | 效果                                    |
| ------- | --------------------------------------------------- | --------------------------------------- |
| macOS   | `apply_vibrancy(NSVisualEffectMaterial::HudWindow)` | 真玻璃（延后，problems#1）              |
| Windows | `apply_acrylic`                                     | 系统 Acrylic（✅ 实测通过，主验证平台） |
| Linux   | `apply_blur` + 半透明                               | 依赖 compositor，妥协方案（延后）       |

- 编译期分支：`#[cfg(target_os = "...")]`，三端互不影响
- 前端玻璃卡片：CSS `backdrop-filter: blur()` 叠加系统级模糊

### 2.5 工作日/打卡模型（2026-09-10 定案，待实现）

- 两层状态机：现有计时器之上加工作日层 `Off → OnDuty（上班，确认框）→ Off（下班，确认框）`
- 新增两表：`workdays`（clock_in/clock_out，跨重启在岗）+ `events`（每次按下的事件流，时间图谱数据源）
- 语义：OnDuty 期间的计时段 = 工作；段间空隙 = 休息；在岗 = 下班 − 上班；三值仅在统计视图按打卡区间呈现
- 未上班禁用计时按钮；忘记下班 → 满 N 小时（默认 8h，⚙ 可调）自动下班，记账回填至上班 + N 小时（发现可迟到、账目准时）
- 统计视图：主界面双标签切换（计时｜统计）+ 时间图谱（工作/休息区块的起止时刻，几点几分粒度）+ 在岗/工作/休息三值 + 历史翻看
- 口径：现有"今日/本周/累计"自然日口径不动；工作日三值双口径并存不混用

## 3. 技术栈

| 层     | 选型                           | 理由                                                         |
| ------ | ------------------------------ | ------------------------------------------------------------ |
| 框架   | Tauri 2（Rust + 系统 WebView） | 三端玻璃插件成熟、包体小（~10MB）                            |
| 语言   | Rust（后端全部业务）           | 学习目标 + 状态机/SQLite 表达自然                            |
| 前端   | Vue 3 + TypeScript + Vite      | 与系列一致，组件化玻璃卡片                                   |
| 存储   | rusqlite（SQLite）             | 零配置本地库                                                 |
| 玻璃   | window-vibrancy                | 三端统一 API                                                 |
| 通知   | tauri-plugin-notification      | 系统通知                                                     |
| 托盘   | Tauri 内置 tray API            | 常驻 + 菜单（PL004 已落地）                                  |
| 热键   | tauri-plugin-global-shortcut   | 全局快捷键（PL004 已落地）                                   |
| 单实例 | tauri-plugin-single-instance   | 防双开（PL004 已落地）                                       |
| 测试   | cargo test（Rust 单测）        | 41 项核心逻辑全测；vitest 前端测试未启用（前端纯展示，可选） |

## 4. 界面设计（玻璃艺术）

```
┌─────────────────────────────────────┐
│  CapsulePulse                    ⚙  │  ← 无边框透明窗口 + ⚙ 设置面板
│                                     │
│  今日 2h 35m ｜ 本周 12h 20m ｜ 累计 210h │  ← 统计行（三值，玻璃卡片）
│                                     │
│  已连续工作 50 分钟，休息一下吧       │  ← 提醒文案条（触达阈值时显示）
│                                     │
│           02:35:42.7                │  ← 大计时器（十分秒位，等宽字体）
│                                     │
│    [ 暂停 ]   或   [ 继续 ][ 重开 ]   │  ← 状态切换按钮（大圆角）
└─────────────────────────────────────┘
```

- 无边框 + `transparent: true` + Acrylic，卡片用半透明 + backdrop-filter
- 深色/浅色跟随系统（✅ 已实现，prefers-color-scheme）
- 窗口 360×480 固定尺寸，全窗可拖动摆放（data-tauri-drag-region）
- 显示定案（2026-09-08）：HH:MM:SS → **HH:MM:SS.d 十分秒位**

## 5. 工程结构

```
capsule-pulse/
├── core/                   # Tauri 2 后端（原 src-tauri 整体改名）
│   ├── Cargo.toml          # 版本单一来源（三段式 X.Y.Z）
│   ├── tauri.conf.json     # version 省略回落 Cargo.toml
│   ├── capabilities/       # ACL 权限（core:default + 拖动 + 通知）
│   ├── icons/              # 应用图标（占位，正式图标属打包期）
│   ├── tests/              # Rust 集成探针
│   └── src/
│       ├── lib.rs          # 应用装配：玻璃/托盘/全局热键/单实例
│       ├── main.rs         # 薄入口
│       ├── session.rs      # 计时状态机（纯逻辑，可单测）
│       ├── storage.rs      # SQLite Repository
│       ├── period.rs       # 统计周期边界纯函数
│       ├── reminder.rs     # 提醒评估器（纯逻辑）
│       ├── settings.rs     # 提醒设置持久化（JSON 原子写）
│       ├── paths.rs        # 运行时数据双落址解析
│       └── commands/       # Tauri 命令层（mod/session/stats/reminder）
├── ui/                     # Vue 前端
│   ├── App.vue             # 玻璃卡片布局 + 提醒监听 + 提示音
│   ├── types.ts            # IPC DTO 镜像类型（单一来源 = Rust serde）
│   ├── main.ts / style.css
│   └── components/
│       ├── TimerCard.vue       # 大计时器 + 开始/暂停/继续/重开
│       ├── StatsCard.vue       # 今日/本周/累计统计行
│       └── SettingsPanel.vue   # ⚙ 设置面板（阈值/声音/通知）
├── configs/                # 用户参数 config.json（运行时写入）+ 固定参数占位
├── data/                   # 运行时数据 pulse.db（gitignore，运行时自建）
├── assets/                 # 提示音 mp3
└── .agents/skills/         # 项目自建 skill（audit-project/audit-report/progress-task）
```

## 6. 实现路径

### Phase 0 — 环境与 Rust 热身（约 1 周）

- [x] Windows 实机装 Rust 工具链 + Tauri CLI（npm devDep）+ Node（平台改向定案：macOS→Windows）
- [x] `cargo test` 跑通核心状态机单测
- [x] Tauri 空壳：透明窗口 + Acrylic 玻璃效果跑通
- [x] 验收：玻璃窗口能显示，`cargo test` 通过（2026-09-08，G1–G4 全过）

### Phase 1 — Rust 核心逻辑（约 1.5 周）

- [x] WorkSession 状态机 + 单测（start/pause/resume/total 边界）
- [x] Storage：建表、会话落库、今日/本周/累计聚合查询 + 单测
- [x] Reminder：阈值判定逻辑 + 单测
- [x] 验收：纯 Rust 层测试全绿（不依赖 UI）（2026-09-09）

### Phase 2 — Tauri 集成与前端（约 1.5 周）

- [x] 命令层：session_start/pause/resume/restart/status + session_stats + 设置读写
- [x] 前端：玻璃卡片 UI、大计时器（100ms tick 十分秒位）、按钮状态切换
- [x] 托盘常驻 + 关闭隐藏到托盘（PL004）
- [x] 验收：Windows 完整计时流程（开始→暂停→统计更新→重启恢复）（2026-09-10）

### Phase 3 — 提醒系统（约 1 周）

- [x] 通知 + 声音触发（达到阈值时），5 分钟重发 + 文案条第三通道
- [x] 设置界面（阈值分钟数、声音/通知开关），持久化配置
- [x] 验收：阈值可配置，触发时通知+声音正常，可关闭（2026-09-09，R1 降级三态过）

### Phase 4 — 体验打磨（已完成）

- [x] Windows：Acrylic 验证（打包移交 Phase 6）
- [x] 快捷键（全局计时切换/窗口显隐）、深色浅色跟随系统（PL004/PL001）
- [x] 数据迁移/备份（单文件 db 拷贝即备份；落址翻案后的手动迁移已实测）

### Phase 5 — 工作日/打卡模型与统计视图（✅ 2026-09-10 PL005 落地收口）

- [x] workdays/events 两表 + 工作日状态机与事件归约纯函数（TDD）（PL005.1–.3，W1–W3 红绿）
- [x] 上班/下班按钮（双向确认框）+ 未上班禁用计时 + 重启恢复在岗（PL005.5–.6，live 过）
- [x] 自动下班（默认 8h 可调，记账回填至上班 + N 小时）（PL005.5，live 实证 clock_out = 上班 + N）
- [x] 统计视图：双标签切换、时间图谱（几点几分到几点几分的工休区块）、在岗/工作/休息三值（PL005.7，live 过）
- [x] 验收：打卡→计时→下班→图谱与三值全链路，统计行口径不受影响（2026-09-10，77 测试全绿 + 门禁全绿）

### Phase 6 — 打包与分发（约 1 周）

- [ ] Windows 打包（形态待拍板：绿色版 vs NSIS/MSI 安装器——双落址与 AUMID 署名的权衡）
- [ ] 正式应用/托盘图标、版本发布流程
- [ ] 自动更新（tauri-updater，可选）
- [ ] 验收：从零安装可运行（Windows；跨端产物随 Phase 7）

### Phase 7 — 三端适配（延后，problems#1）

- [ ] macOS：vibrancy 真玻璃验证 + .app/dmg 打包
- [ ] Linux：blur 妥协验证 + AppImage/deb 打包
- [ ] 验收：三端可运行（Windows/Linux 用 CI 或借机验证）

## 7. 时间预估

**全职投入：约 6-8 周**（含 Rust 学习曲线）；**实际：PL001–PL005 核心功能全量于 2026-09-08 至 09-10 三天完成**（预估按学习曲线保守，Rust 上手快于预期）

| Phase   | 内容             | 时间                      |
| ------- | ---------------- | ------------------------- |
| Phase 0 | 环境与热身       | 1 周（✅）                |
| Phase 1 | Rust 核心逻辑    | 1.5 周（✅）              |
| Phase 2 | Tauri 集成与前端 | 1.5 周（✅）              |
| Phase 3 | 提醒系统         | 1 周（✅）                |
| Phase 4 | 体验打磨         | ✅ 已完成                 |
| Phase 5 | 工作日/打卡模型  | ✅ 已完成（PL005）        |
| Phase 6 | 打包分发         | 1 周（下一步）            |
| Phase 7 | 三端适配         | 1.5 周（延后 problems#1） |

**兼职投入：约 2.5-3.5 个月**（按每天 2-3 小时，含 Rust 学习）

关键里程碑：Phase 1 = Rust 学习达标（✅ 全绿）；Phase 2 = 可用闭环（✅ Windows）；Phase 4 = 体验打磨（✅）；Phase 5 = 工作日/打卡模型（✅ PL005）；Phase 6 = 可分发（下一步）；Phase 7 = 三端可跑（延后 problems#1）。

## 8. 风险与对策

| 风险                          | 等级 | 对策                                                                                                                 |
| ----------------------------- | ---- | -------------------------------------------------------------------------------------------------------------------- |
| Rust 借用检查学习曲线         | 高   | 状态机/存储先写纯逻辑+单测，小步迭代；Mutex/生命周期报错及时求助                                                     |
| Linux 玻璃效果依赖 compositor | 中   | 文档声明局限；半透明+模糊退化为可接受方案                                                                            |
| 提醒声音三端差异              | 低   | 前端 audio 统一播放；通知走插件，降级互不依赖                                                                        |
| 崩溃丢失当前段数据            | 低   | 暂停/重开/退出前即落库；自动下班按"上班 + N 小时"回填记账；工作日 events 事件表已落地（PL005），每次按下时间点全留痕 |

## 9. 与系列项目的关系

| 项目             | 职责           | 技术                      |
| ---------------- | -------------- | ------------------------- |
| CapsuleRetro     | DOS 游戏包装   | Tauri + Rust + DOSBox     |
| CapsulePlan      | 计划落定 agent | Pi 扩展 + TypeScript      |
| **CapsulePulse** | 工作时间看板   | **Tauri + Rust + SQLite** |

CapsulePulse 是系列中第一个**以 Rust 为主**的项目，其核心逻辑（状态机/存储/调度）可复用模式到 CapsuleRetro 的 Rust 侧。
