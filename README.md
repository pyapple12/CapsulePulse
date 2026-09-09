# CapsulePulse — 玻璃质感的工作计时看板

[![Version](https://img.shields.io/badge/Version-0.1.0.7-blue.svg)](core/Cargo.toml)
[![Rust](https://img.shields.io/badge/Rust-1.96-orange.svg)](https://www.rust-lang.org)
[![Phase](https://img.shields.io/badge/Phase-PL003_完成-brightgreen.svg)](z.plan.md)

极简的工作计时看板：一个大的开始/暂停按钮记录工作时间，连续工作超阈值时声音 + 系统通知提醒休息。三端（Windows / macOS / Linux）通用。

| 卖点     | 说明                                                      |
| -------- | --------------------------------------------------------- |
| 极简计时 | 一个大按钮，开始/暂停即全部交互                           |
| 时长统计 | 今日/本周/累计纯时长聚合，v1 不做标签/项目                |
| 休息提醒 | 连续工作达阈值（默认 50 分钟，可配置）→ 声音 + 系统通知   |
| 玻璃 UI  | macOS vibrancy / Windows Acrylic / Linux blur，透明无边框 |
| 常驻后台 | 托盘常驻、全局快捷键唤起、关闭最小化到托盘                |

> 当前状态：**PL001–PL004 已完成**（V0.1.0.7）——玻璃壳计时闭环 + 存储统计 + 提醒设置 + 托盘常驻（关闭隐藏到托盘、托盘菜单、Alt+Shift+P/S 全局热键、单实例、退出落库）；41 项测试全绿。下一个大件：打包分发，未立项。方案与实测结论见 `z.plan.md` 附录，任务档案见 `x.progress.md`。

## 技术栈

| 层   | 选型                                       | 说明                                                                  |
| ---- | ------------------------------------------ | --------------------------------------------------------------------- |
| 框架 | Tauri 2（Rust + 系统 WebView）             | 三端玻璃插件成熟、包体小（~10MB）                                     |
| 语言 | Rust（后端全部业务逻辑）                   | 学习目标：状态机/SQLite/调度表达自然、可单测                          |
| 前端 | Vue 3 + TypeScript + Vite                  | 与系列一致，只做展示                                                  |
| 存储 | rusqlite（SQLite）                         | 零配置本地库，`data/pulse.db`（配置 `configs/config.json`，便携布局） |
| 玻璃 | window-vibrancy                            | macOS vibrancy / Windows acrylic / Linux blur                         |
| 通知 | tauri-plugin-notification + 前端 `<audio>` | 通知与声音降级互不依赖                                                |

## 项目目标（除产品外）

- **学习 Rust**：全部业务逻辑放 Rust 侧（状态机、统计、持久化、提醒调度），前端只做展示——Rust 占比高、可单测
- 系列定位：CapsuleRetro（包装游戏）→ CapsulePlan（落定计划）→ **CapsulePulse（记录时间）**——系列中第一个以 Rust 为主的项目

## 快速开始

### 环境要求

- Rust toolchain + Node.js（26+）；Tauri CLI 经 npm devDep `@tauri-apps/cli` 随 `npm install` 就位
- Windows 10/11（当前开发与验证实机；macOS/Linux 适配延后至 Windows 版成熟后 [problems#1]）

### 构建与测试

```bash
cargo test        # Rust 层全量测试（状态机/存储/提醒，不依赖 UI）
npm run tauri dev # 开发运行（透明玻璃窗口；前端产物内嵌自包含，改前端后先 npm run build）
npm run build     # 前端构建校验（含 vue-tsc）
```

## 架构一句话

计时状态机（Idle/Running/Paused）、SQLite 统计聚合、提醒调度全部在 Rust 侧实现并可用 `cargo test` 直测；Vue 前端只做展示与命令转发——业务逻辑零含量。

## 项目结构

```
CapsulePulse/
├── core/                 # Tauri 2 后端（版本单一来源 Cargo.toml；原框架默认名 src-tauri）
│   └── src/
│       ├── lib.rs        # 应用装配：玻璃挂载 + 模块注册
│       ├── main.rs       # 薄入口
│       ├── session.rs    # 计时状态机（纯 Rust 可单测）
│       ├── storage.rs    # SQLite Repository（今日/本周/累计聚合）
│       ├── period.rs     # 统计周期边界纯函数（本地零点/周一零点）
│       ├── reminder.rs   # 提醒评估器（阈值触发 + 5 分钟重发）
│       ├── settings.rs   # 提醒设置持久化（JSON 原子写）
│       └── commands/     # Tauri 命令层（mod / session / stats / reminder 按职责分文件）
├── ui/                   # Vue 前端（App + TimerCard / StatsCard / SettingsPanel，types.ts 镜像 DTO）
├── configs/              # 用户参数 config.json（运行时写入，gitignore）+ 固定参数占位
├── data/                 # 运行时数据 pulse.db（gitignore，运行时自建）
├── assets/               # 提示音、图标
├── AGENTS.md             # 项目规范（AI 协作必读）
├── CapsulePulse_plan.md  # 总体规划（Phase 0-5）
├── z.plan.md             # 方案与审计归档
├── x.progress.md         # 任务清单
├── y.problems.md / w.study.md
└── .agents/skills/       # 项目自建 skill（audit-project / audit-report / progress-task）
```

## 文档地图

- `CapsulePulse_plan.md`：总体规划与 Phase 划分、时间预估、风险对策
- `AGENTS.md`：工程原则、代码规范、提交规范（AI 协作必读）
- `z.plan.md`：专题方案与审计归档
- `x.progress.md`：任务清单与进度
- `y.problems.md`：问题与远期改进备忘录
- `w.study.md`：项目分析报告
