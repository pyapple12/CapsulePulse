# CapsulePulse — 项目计划书

> 玻璃质感的工作计时看板：开始/暂停记录工作时间，超时声音+通知提醒休息。三端（W/M/L），Tauri 2 + Rust。

## 1. 项目概述

CapsulePulse 是一款极简的工作计时看板应用：

- **核心交互**：一个大的开始/暂停按钮，记录工作时间
- **统计展示**：今日总时长、本周累计、历史累计（纯时长聚合，v1 不做标签/项目）
- **休息提醒**：连续工作达到阈值（默认 50 分钟，可配置）→ 声音 + 系统通知
- **UI**：macOS 透明玻璃艺术风格（vibrancy），三端适配
- **常驻**：托盘/菜单栏常驻，快捷键唤起，后台持续计时

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

### 2.2 数据模型（SQLite，rusqlite）

```sql
CREATE TABLE sessions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    started_at INTEGER NOT NULL,   -- Unix 秒
    seconds INTEGER NOT NULL       -- 会话时长
);
```

- 存储位置：`~/.capsule-pulse/pulse.db`（三端用户目录）
- v1 查询：今日 SUM、本周 SUM、全部 SUM
- 会话落库时机：暂停时写入一条记录（简单可靠，崩溃最多丢当前段）

### 2.3 提醒调度

- 阈值可配置（默认 50 分钟，设置界面修改，持久化）
- 触发：`tauri_plugin_notification` 系统通知 + 声音播放
- 声音：前端 `<audio>` 播放打包的提示音（跨平台最简）；Rust 侧 rodio 为备选
- 降级：通知不可用（部分 Linux）时仅声音；反之亦然

### 2.4 玻璃效果（window-vibrancy crate）

| 平台    | 实现                                                | 效果                      |
| ------- | --------------------------------------------------- | ------------------------- |
| macOS   | `apply_vibrancy(NSVisualEffectMaterial::HudWindow)` | 真玻璃（主开发/验证平台） |
| Windows | `apply_acrylic`                                     | 系统 Acrylic              |
| Linux   | `apply_blur` + 半透明                               | 依赖 compositor，妥协方案 |

- 编译期分支：`#[cfg(target_os = "...")]`，三端互不影响
- 前端玻璃卡片：CSS `backdrop-filter: blur()` 叠加系统级模糊

## 3. 技术栈

| 层   | 选型                                    | 理由                              |
| ---- | --------------------------------------- | --------------------------------- |
| 框架 | Tauri 2（Rust + 系统 WebView）          | 三端玻璃插件成熟、包体小（~10MB） |
| 语言 | Rust（后端全部业务）                    | 学习目标 + 状态机/SQLite 表达自然 |
| 前端 | Vue 3 + TypeScript + Vite               | 与系列一致，组件化玻璃卡片        |
| 存储 | rusqlite（SQLite）                      | 零配置本地库                      |
| 玻璃 | window-vibrancy                         | 三端统一 API                      |
| 通知 | tauri-plugin-notification               | 系统通知                          |
| 托盘 | Tauri 内置 tray API                     | 常驻 + 菜单                       |
| 测试 | cargo test（Rust 单测）+ vitest（前端） | 核心逻辑全测                      |

## 4. 界面设计（玻璃艺术）

```
┌─────────────────────────────────────┐
│  CapsulePulse                    - □ × │  ← 无边框透明窗口（tauri transparent）
│                                     │
│        今日 2h 35m ｜ 本周 12h 20m     │  ← 统计行（玻璃卡片）
│                                     │
│            ┌─────────────┐          │
│            │   02:35:42  │          │  ← 大计时器数字（等宽字体）
│            └─────────────┘          │
│            ┌─────────────┐          │
│            │   ⏸ 暂停    │          │  ← 大圆角按钮（状态切换）
│            └─────────────┘          │
│                                     │
│      已连续工作 50 分钟，休息一下吧     │  ← 提醒状态提示
└─────────────────────────────────────┘
```

- 无边框 + `transparent: true` + vibrancy，卡片用半透明白 + backdrop-filter
- 深色/浅色跟随系统（macOS 主题感知）
- 窗口小尺寸（约 360×480），可随意摆放

## 5. 工程结构

```
capsule-pulse/
├── src-tauri/
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs          # 入口：透明窗口、玻璃效果、托盘、快捷键
│       ├── commands.rs      # Tauri 命令层（session_start/pause/...）
│       ├── core/
│       │   ├── mod.rs
│       │   ├── session.rs   # 状态机（纯逻辑，可单测）
│       │   └── reminder.rs  # 提醒调度（阈值判定 + 通知/声音触发）
│       └── storage.rs       # SQLite Repository（add_session/today_total/...）
├── src/                     # Vue 前端
│   ├── App.vue
│   └── components/
│       ├── TimerCard.vue    # 大计时器 + 开始/暂停按钮
│       └── StatsCard.vue    # 今日/本周统计
├── assets/                  # 提示音、图标
└── tests/                   # 前端测试（可选）
```

## 6. 实现路径

### Phase 0 — 环境与 Rust 热身（约 1 周）

- [ ] macOS 装 Rust 工具链 + Tauri CLI + Node
- [ ] `cargo test` 跑通核心状态机单测（从第 1 段参考代码起步）
- [ ] Tauri 空壳：透明窗口 + vibrancy 玻璃效果跑通（macOS 主验证）
- [ ] 验收：玻璃窗口能显示，`cargo test` 通过

### Phase 1 — Rust 核心逻辑（约 1.5 周）

- [ ] WorkSession 状态机 + 单测（start/pause/resume/total 边界）
- [ ] Storage：建表、会话落库、今日/本周/累计聚合查询 + 单测
- [ ] Reminder：阈值判定逻辑 + 单测
- [ ] 验收：纯 Rust 层测试全绿（不依赖 UI）

### Phase 2 — Tauri 集成与前端（约 1.5 周）

- [ ] 命令层：session_start/pause/today_total/week_total 等
- [ ] 前端：玻璃卡片 UI、大计时器（每秒 tick）、按钮状态切换
- [ ] 托盘常驻 + 关闭最小化到托盘
- [ ] 验收：macOS 上完整计时流程（开始→暂停→统计更新→退出恢复）

### Phase 3 — 提醒系统（约 1 周）

- [ ] 通知 + 声音触发（达到阈值时）
- [ ] 设置界面（阈值分钟数、声音开关），持久化配置
- [ ] 验收：50 分钟阈值可配置，触发时通知+声音正常，可关闭

### Phase 4 — 三端适配与打磨（约 1.5 周）

- [ ] Windows：Acrylic 验证、打包；Linux：blur 妥协验证
- [ ] 快捷键（全局唤起/开始暂停）、深色浅色跟随系统
- [ ] 数据迁移/备份（拷 db 即备份）
- [ ] 验收：三端可运行（Windows/Linux 用 CI 或借机验证）

### Phase 5 — 打包与分发（约 1 周）

- [ ] 三端安装包：macOS .app/dmg、Windows NSIS/绿色版、Linux AppImage/deb
- [ ] 图标、版本号、自动更新（tauri-updater 可选）
- [ ] 验收：三端产物从零安装可运行

## 7. 时间预估

**全职投入：约 6-8 周**（含 Rust 学习曲线）

| Phase   | 内容             | 时间   |
| ------- | ---------------- | ------ |
| Phase 0 | 环境与热身       | 1 周   |
| Phase 1 | Rust 核心逻辑    | 1.5 周 |
| Phase 2 | Tauri 集成与前端 | 1.5 周 |
| Phase 3 | 提醒系统         | 1 周   |
| Phase 4 | 三端适配         | 1.5 周 |
| Phase 5 | 打包分发         | 1 周   |

**兼职投入：约 2.5-3.5 个月**（按每天 2-3 小时，含 Rust 学习）

关键里程碑：Phase 1 = Rust 学习达标（核心逻辑全绿）；Phase 2 = macOS 可用闭环；Phase 4 = 三端可跑；Phase 5 = 可分发。

## 8. 风险与对策

| 风险                          | 等级 | 对策                                                             |
| ----------------------------- | ---- | ---------------------------------------------------------------- |
| Rust 借用检查学习曲线         | 高   | 状态机/存储先写纯逻辑+单测，小步迭代；Mutex/生命周期报错及时求助 |
| Linux 玻璃效果依赖 compositor | 中   | 文档声明局限；半透明+模糊退化为可接受方案                        |
| 提醒声音三端差异              | 低   | 前端 audio 统一播放；通知走插件，降级互不依赖                    |
| 崩溃丢失当前段数据            | 低   | 暂停即落库；运行时启动时若检测到未落库的 Running 段可提示恢复    |

## 9. 与系列项目的关系

| 项目             | 职责           | 技术                      |
| ---------------- | -------------- | ------------------------- |
| CapsuleRetro     | DOS 游戏包装   | Tauri + Rust + DOSBox     |
| CapsulePlan      | 计划落定 agent | Pi 扩展 + TypeScript      |
| **CapsulePulse** | 工作时间看板   | **Tauri + Rust + SQLite** |

CapsulePulse 是系列中第一个**以 Rust 为主**的项目，其核心逻辑（状态机/存储/调度）可复用模式到 CapsuleRetro 的 Rust 侧。
