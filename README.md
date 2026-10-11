# ⏳ CapsulePulse — 玻璃质感的工作计时看板

**简体中文** | [English](#english)

[![Version](https://img.shields.io/badge/Version-0.1.2.6-blue.svg)](core/Cargo.toml)
[![Platform](https://img.shields.io/badge/Platform-Windows_10%2F11-0078D6.svg)](#下载)
[![Rust](https://img.shields.io/badge/Rust-1.96-orange.svg)](https://www.rust-lang.org)
[![Vue](https://img.shields.io/badge/Vue-3-42b883.svg)](https://vuejs.org)
[![License](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)

<!-- 待补：CI 徽章（建 .github/workflows/ci.yml 后启用）
[![CI](https://github.com/pyapple12/CapsulePulse/actions/workflows/ci.yml/badge.svg)](../../actions/workflows/ci.yml)
-->

<!-- 待补：预览图（放 images/Github-Preview-Image.jpg 后启用）
<p align="center">
  <img src="images/Github-Preview-Image.jpg" alt="CapsulePulse —— 玻璃质感的工作计时看板" />
</p>
-->

---

## 中文版

一款玻璃质感的工作计时看板——上班/下班打卡开启一天，班内计时 = 工作、空隙 = 休息，下班出在岗/工作/休息三值与时间图谱；连续工作超阈值时声音 + 系统通知提醒休息。

### ✨ 功能亮点

- ⏱ **打卡出勤** —— 上班/下班打卡（双向确认框），未上班禁用计时，重启恢复在岗
- 🕹 **班内计时** —— 一个大按钮，计时 = 工作、空隙 = 休息；自动下班（默认 8h 可调，回填记账）
- 📊 **统计视图** —— 日翻看三值（在岗/工作/休息）+ 时间图谱 + 自然周周卡 + 全历史总日均（相对平均的高低指示）
- 🔔 **休息提醒** —— 连续工作达阈值（默认 50 分钟，可配置）→ 声音 + 系统通知 + 卡片文案条
- 🫧 **玻璃 UI** —— CT 分态纱：纯 alpha 透明窗（失焦 30% 纱 / 聚焦 0% 桌面直透）+ 浮板毛玻璃
- 🛰 **常驻后台** —— 托盘常驻、全局快捷键唤起、关闭最小化到托盘
- 🎒 **绿色便携** —— 数据全部落在同级 `configs\` 与 `data\`（备份/迁移 = 拷走这两个文件夹）

### 📥 下载

> ⏳ 尚未发布——打包分发排期见 `CapsulePulse_plan.md` Phase 6。
>
> 发布后此处给出：绿色版 zip 名 / 解压即用说明 / 备份与迁移（拷走 exe 同级 `configs\` + `data\`）。

### 🧱 技术栈

| 层      | 选型                                       | 说明                                                                                |
| ------- | ------------------------------------------ | ----------------------------------------------------------------------------------- |
| 📦 壳   | Tauri 2（Rust + 系统 WebView）             | 包体小、系统能力原生接入                                                            |
| 🦀 核心 | 纯 Rust——状态机 / SQLite / 业务规则        | 全部业务逻辑在 Rust 侧，`cargo test` 直测                                           |
| 🖼️ 前端 | Vue 3 + TypeScript + Vite                  | 只做展示，零业务逻辑                                                                |
| 🗄️ 存储 | SQLite（rusqlite）                         | 零配置本地库 `data/pulse.db`（设置 `configs/config.json`，便携布局）                |
| 🫧 玻璃 | CT 分态纱（纯 CSS）                        | 恒纯 alpha 透明窗 + `::before` 纱层（失焦 30% / 聚焦 0%），三端同构、无 OS 材质依赖 |
| 🔔 通知 | tauri-plugin-notification + 前端 `<audio>` | 通知与声音降级互不依赖                                                              |

### 🛠️ 开发

```bash
npm install         # 首次
cargo test          # Rust 核心全量测试（状态机 / 统计 / 存储 / 提醒，不依赖 UI）
npm run tauri dev   # 开发运行（透明玻璃窗；前端为静态产物，改前端后先 npm run build）
npm run dev         # 纯前端开发（Vite；无 Tauri 外壳时走 mock-invoke 假 IPC 基座）
npm run build       # 前端构建（含 vue-tsc 类型检查）
```

> 🔧 需要 Rust 工具链与 Node.js 26+；dev 模式运行时数据落项目根 `configs/` 与 `data/`（release = exe 同级）。

### 🏗️ 架构一句话

计时状态机（Idle/Running/Paused）、工作日事件归约、SQLite 统计聚合、提醒调度、托盘/热键运行时——全部业务逻辑为纯 Rust 并有测试覆盖；Vue 层只做渲染与命令转发。

### 🎯 项目目标（除产品外）

- **学习 Rust**：全部业务逻辑放 Rust 侧（状态机、统计、持久化、提醒调度），前端只做展示——Rust 占比高、可单测
- 系列定位：系列中第一个以 Rust 为主的项目 🦀

### 📁 项目结构

```
CapsulePulse/
├── core/                      # 🦀 Tauri 2 后端（版本单一来源：Cargo.toml；原框架默认名 src-tauri）
│   ├── src/
│   │   ├── lib.rs             # 应用装配：窗口 / 托盘 / 全局热键 / 插件 + 命令注册
│   │   ├── main.rs            # 薄入口
│   │   ├── session.rs         # 计时状态机（纯逻辑，可单测）
│   │   ├── workday.rs         # 工作日状态机 + 事件归约（打卡 / 图谱纯逻辑，可单测）
│   │   ├── storage.rs         # SQLite 仓储（sessions / workdays / events 三表，参数化 SQL）
│   │   ├── period.rs          # 统计周期边界纯函数（本地零点 / 周一零点）
│   │   ├── reminder.rs        # 提醒评估器（阈值触发 + 5 分钟重发）
│   │   ├── settings.rs        # 设置持久化（JSON 原子写）
│   │   ├── paths.rs           # 运行时数据双落址（configs/ + data/）
│   │   ├── diag.rs            # 诊断日志（data/pulse.log）
│   │   └── commands/          # Tauri 命令层（薄封装；核心可脱离 Tauri 直测）
│   ├── tests/                 # 集成测试（存储探针：真实文件库往返）
│   └── tauri.conf.json        # 窗口 / 打包配置
├── ui/                        # 🖼️ Vue 3 前端（只做展示，零业务逻辑）
│   ├── App.vue                # 骨架：topbar + 计时页 / 统计页 + 浮层
│   ├── components/            # TimerCard / StatsView / ConfirmModal / SettingsPanel
│   ├── src/styles/            # 样式层（glass / topbar / timer / hourglass / stats / graph / boards / controls）
│   ├── src/dev/               # mock-invoke（无 Tauri 外壳时的 DEV 假 IPC 基座）
│   ├── types.ts               # IPC DTO 类型镜像（与 Rust serde 同源）
│   └── format.ts              # 展示格式化共享助手
├── design/                    # 🎨 ui2.0 设计原型（冻结归档，映射的事实参照）
├── configs/                   # ⚙️ 运行时设置 config.json（gitignore，运行时自建）
├── data/                      # 🗄️ 运行时数据 pulse.db / pulse.log（gitignore，运行时自建）
├── assets/                    # 🔊 提示音素材
├── AGENTS.md                  # 📐 工程规范与协作纪律
├── CapsulePulse_plan.md       # 🗺️ 总体规划与分期
├── z.plan.md                  # 📋 专题方案与全量审计归档
├── x.progress.md              # ✅ 任务清单与进度
├── y.problems.md              # 🐞 问题与远期改进备忘录
├── w.study.md                 # 🔬 项目分析报告
├── LICENSE                    # 📄 MIT 许可
└── .agents/skills/            # 🤖 项目自建 skill（audit-project / audit-report / progress-task）
```

### 🗺️ 路线图

- ✅ Phase 0–5 —— 玻璃计时闭环 → 存储统计 → 提醒设置 → 托盘常驻与全局快捷键 → 工作日/打卡模型与统计视图 → UI 与材质迭代（`design/` 原型 → APP 化映射）
- ⏳ Phase 6 —— 打包分发（绿色版 zip / 安装包）
- ⏳ Phase 7 —— macOS / Linux 三端适配

### 📄 许可

以 [MIT License](LICENSE) 开源。

### 🧬 系列

Capsule 系列第三作：CapsuleRetro（包装游戏）→ CapsulePlan（落定计划）→ **CapsulePulse（记录时间）** → CapsuleTODO（桌面清单）。

---

<a id="english"></a>

## English

A glassmorphic work-time board — clock in to open a day, in-session time counts as work and the gaps as rest; when you clock out you get duty/work/rest tri-values plus a timeline graph, and a sound + system notification nudges you to take a break once continuous work passes a configurable threshold.

### ✨ Highlights

- ⏱ **Clock in / out** — punch in and out with a confirmation dialog; the timer is disabled until you clock in, and an open shift is restored on restart
- 🕹 **In-session timing** — one big control: running time is work, gaps are rest; auto clock-out (default 8 h, configurable, backfilled accounting)
- 📊 **Stats board** — browse any day's tri-values (duty/work/rest) + timeline graph + natural-week bars + all-history daily average (above/below-average indicator)
- 🔔 **Break reminder** — configurable threshold (default 50 min) → sound + system notification + an in-card banner
- 🫧 **Glass UI** — CT dual-state veil: a pure-alpha transparent window (30 % veil when idle, 0 % — desktop straight through — when focused) plus frosted overlay panels
- 🛰 **Tray-native** — lives in the tray, summoned by a global hotkey; closing hides it instead of quitting
- 🎒 **Portable** — all data stays in `configs\` + `data\` beside the exe (backup/migrate = copy those two folders)

### 📥 Download

> ⏳ Not released yet — packaging is scheduled as Phase 6 in `CapsulePulse_plan.md`.
>
> Once released, this section will list the portable zip name, unzip-and-run steps, and backup/migration (copy `configs\` + `data\` next to the exe).

### 🧱 Built With

| Layer      | Choice                                      | Why                                                                               |
| ---------- | ------------------------------------------- | --------------------------------------------------------------------------------- |
| 📦 Shell   | Tauri 2 (Rust + system WebView)             | small footprint, native OS capabilities                                           |
| 🦀 Core    | Pure Rust — state machines / SQLite / rules | all logic lives in Rust, directly unit-tested (`cargo test`)                      |
| 🖼️ UI      | Vue 3 + TypeScript + Vite                   | presentation only, zero business logic                                            |
| 🗄️ Storage | SQLite via rusqlite                         | zero-config local database `data/pulse.db` (settings `configs/config.json`)       |
| 🫧 Glass   | CT dual-state veil (pure CSS)               | alpha-transparent window + `::before` veil (30 % idle / 0 % focused), OS-agnostic |
| 🔔 Notify  | tauri-plugin-notification + `<audio>`       | notification and sound degrade independently                                      |

### 🛠️ Develop

```bash
npm install         # once
cargo test          # Rust core: full test suite (state machines / stats / storage / reminders), no UI required
npm run tauri dev   # dev run (transparent glass window; static frontend bundle — rebuild via npm run build after UI edits)
npm run dev         # frontend-only dev (Vite; falls back to the mock-invoke fake IPC harness without a Tauri shell)
npm run build       # frontend build incl. vue-tsc type check
```

> 🔧 Requires the Rust toolchain and Node.js 26+. Runtime data lands in `configs/` + `data/` at the project root in dev mode (beside the exe in release).

### 🏗️ Architecture in One Line

The timer state machine (Idle/Running/Paused), workday event reduction, SQLite aggregation, reminder scheduling and the tray/hotkey runtime are all pure Rust with test coverage; the Vue layer only renders and forwards commands.

### 🎯 Goals (Beyond the Product)

- **Learn Rust** — every piece of business logic (state machines, stats, persistence, reminder scheduling) lives on the Rust side; the frontend is presentation only
- The first Rust-first project in the series 🦀

### 📁 Project Structure

```
CapsulePulse/
├── core/                      # 🦀 Tauri 2 backend (version source of truth: Cargo.toml; formerly src-tauri)
│   ├── src/
│   │   ├── lib.rs             # app wiring: window / tray / global hotkeys / plugins + command registry
│   │   ├── main.rs            # thin entry point
│   │   ├── session.rs         # timer state machine (pure logic, unit-tested)
│   │   ├── workday.rs         # workday state machine + event reduction (clock-in/graph pure logic)
│   │   ├── storage.rs         # SQLite repository (sessions / workdays / events, parameterized SQL)
│   │   ├── period.rs          # period boundary pure functions (local midnight / Monday midnight)
│   │   ├── reminder.rs        # reminder evaluator (threshold trigger + 5-minute re-fire)
│   │   ├── settings.rs        # settings persistence (atomic JSON write)
│   │   ├── paths.rs           # runtime dual-location paths (configs/ + data/)
│   │   ├── diag.rs            # diagnostic log (data/pulse.log)
│   │   └── commands/          # Tauri command layer (thin shell; cores are Tauri-free & directly testable)
│   ├── tests/                 # integration tests (storage probe: real-file round trip)
│   └── tauri.conf.json        # window / bundle config
├── ui/                        # 🖼️ Vue 3 frontend (presentation only, zero business logic)
│   ├── App.vue                # shell: topbar + timer page / stats page + overlays
│   ├── components/            # TimerCard / StatsView / ConfirmModal / SettingsPanel
│   ├── src/styles/            # style layer (glass / topbar / timer / hourglass / stats / graph / boards / controls)
│   ├── src/dev/               # mock-invoke (fake IPC harness for browser-only dev)
│   ├── types.ts               # IPC DTO type mirror (same source as Rust serde)
│   └── format.ts              # shared display formatters
├── design/                    # 🎨 ui2.0 design prototype (frozen archive, source of truth for looks)
├── configs/                   # ⚙️ runtime settings config.json (gitignored, auto-created)
├── data/                      # 🗄️ runtime data pulse.db / pulse.log (gitignored, auto-created)
├── assets/                    # 🔊 notification sound assets
├── AGENTS.md                  # 📐 engineering conventions & collaboration discipline (Chinese)
├── CapsulePulse_plan.md       # 🗺️ product plan & phases (Chinese)
├── z.plan.md                  # 📋 specs & full audit archive
├── x.progress.md              # ✅ task list & progress
├── y.problems.md              # 🐞 issues & future improvements memo
├── w.study.md                 # 🔬 project analysis report
├── LICENSE                    # 📄 MIT license
└── .agents/skills/            # 🤖 project-built skills (audit-project / audit-report / progress-task)
```

### 🗺️ Roadmap

- ✅ Phase 0–5 — glass timer loop → storage & stats → reminder settings → tray & global hotkeys → workday/clock-in model & stats board → UI and material iterations (`design/` prototype → APP mapping)
- ⏳ Phase 6 — packaging & distribution (portable zip / installer)
- ⏳ Phase 7 — macOS / Linux adaptors

### 📄 License

Released under the [MIT License](LICENSE).

### 🧬 Series

Part of the **Capsule** series: CapsuleRetro (game wrapper) → CapsulePlan (planner) → **CapsulePulse (time tracker)** → CapsuleTODO (desktop list).
