# 项目分析报告（w.study.md）

> 文件职责：对项目代码与外部依赖的系统性研究存档。结构：1 项目概述 → 2 目录结构与模块职责 → 3 核心设计模式（真实代码片段 + 要点列表）→ 4 代码风格观察 → 5 与其他项目对比。代码未落地的章节留占位，落地后按实际回改。
> §1/§2 已按实际落地回改（2026-09-08，含目录风格改造同步）；§3.1/§3.2 为实测沉淀；§4/§5 留占位。

## 1. 项目概述

CapsulePulse：玻璃质感的工作计时看板。极简交互（一个大的开始/暂停按钮）+ 纯时长统计 + 超时休息提醒，三端（Windows / macOS / Linux）通用。组件流水线：

```
[计时] core/src/session.rs           Idle → Running { start } → Paused { elapsed } 纯 Rust 状态机，可单测
[存储] core/src/storage.rs           rusqlite：会话落库（暂停时写一条）+ 今日/本周/累计聚合查询
[提醒] core/src/reminder.rs          阈值判定 → 系统通知（tauri-plugin-notification）+ 前端 <audio> 提示音
[展示] ui/（Vue）                     大计时器 + 玻璃卡片统计 + 按钮状态切换，业务逻辑零含量
```

系列定位：CapsuleRetro（包装游戏）→ CapsulePlan（落定计划）→ **CapsulePulse（记录时间）**——系列中第一个以 Rust 为主的项目。

## 2. 目录结构与模块职责

（2026-09-08 按实际回改——目录风格改造后形态；命令/存储/提醒随阶段落地后继续回改）

```
core/               # Tauri 2 后端（版本单一来源 Cargo.toml；原框架默认名 src-tauri）
  Cargo.toml
  tauri.conf.json   # version 字段省略（回落 Cargo.toml）
  src/
    lib.rs          # 应用装配：Acrylic 挂载 + 模块注册 + 存储/设置初始化
    main.rs         # 薄入口（调 capsule_pulse::run()）
    session.rs      # 计时状态机（已落地）
    commands/       # Tauri 命令层（已落地：按职责分文件——mod 上下文与共享 / session 会话与提醒评估 / stats 统计 / reminder 副作用与设置，测试随职责分布）
    storage.rs      # SQLite 会话存储（已落地：PL002）
    period.rs       # 统计周期边界纯函数（已落地：PL002）
    reminder.rs     # 提醒评估器（已落地：PL003）
    settings.rs     # 提醒设置持久化（已落地：PL003）
ui/                 # Vue 3 + TS + Vite 前端（App / TimerCard / StatsCard / SettingsPanel）
configs/            # 程序读的参数（预建占位）
assets/             # 提示音（house_alarm-clock_loud.mp3）
```

## 3. 核心设计模式

（逐条实测沉淀：外部依赖陷阱与内部模式分开记；写法 = 来源与实测日期 + 真实片段 + 要点与边界）

### 3.1 Tauri 2 ACL 静默拒——能力白名单外的调用零报错失效（实测 2026-09-08，tauri 2.11.5）

PL001 阶段 B 实测：`data-tauri-drag-region` 声明了拖动区但拖动完全不生效，控制台/WebView 零报错。根因：拖动由前端调 `start_dragging` IPC 命令实现，走 Tauri 2 ACL 白名单；`core:default` 集只含 `core:window:default`，而该集（构建产物 `core/gen/schemas/acl-manifests.json` 可查）全部是只读查询权限（position/size/is-* 系），**不含 `allow-start-dragging`**——IPC 被静默拒绝，无任何诊断输出。

```
capabilities/default.json
  "permissions": ["core:default", "core:window:allow-start-dragging"]  # 修复：显式补授
```

要点与边界：

- **症状学**：ACL 拒绝的统一症状 = 功能静默失效 + 控制台干净；命令（invoke）可能成功而能力（listen/drag）全无，或反之——按"哪个 IPC 命令被拒"逐个对号，不能凭"构建绿"推断功能可用
- **系列案例累计两例**：CapsuleRetro `listen()` 被 ACL 静默拒（缺 `core:event`，2026-09-06）；本项目拖动静默失效（缺 `allow-start-dragging`，2026-09-08）——凡新增前端能力调用，必须 live 验证功能生效
- **核查手法**：构建产物 `core/gen/schemas/acl-manifests.json` 是权限事实源——`default_permission.permissions` 数组逐项核对，比查文档快且与本机构建版本一致
- **边界**：只影响前端→Rust 的 IPC 能力面；Rust 侧代码（setup 挂玻璃等）不经 ACL，不受影响

### 3.2 vite publicDir 默认在项目根——子目录 public/ 不生效（实测 2026-09-09，vite 8.2.2）

PL003 提示音 404 无声实测：音频放 `ui/public/`，但 vite 的 `publicDir` 默认 = `<project root>/public`（本项目根为 E:\CodeMission\CapsulePulse）——`ui/public/` 从不被拷贝进 dist，`/chime.wav` 404，`audio.play()` reject 走降级。

```ts
// 修法：import 打包（哈希文件名进 dist），比配置 publicDir 更稳——显式依赖 + 类型安全（vite/client 内建 mp3 声明）
import chimeUrl from "../assets/house_alarm-clock_loud.mp3";
```

要点与边界：

- 实证手法：构建后 `ls dist/assets` 直接看资源是否落地
- 副产品：404 → play() reject → 降级 catch——容错白名单路径被真实触发验证（PL003 提示音失败降级仅通知）
- 关联：未打包 exe 的系统通知署名回退 PowerShell 属另一独立问题（y.problems #2，随打包解决）

## 4. 代码风格观察

（暂无——代码未落地；首次审计后按实际记录）

## 5. 与其他项目对比

（暂无——候选对照点：CapsuleRetro 的 Tauri 2 工程结构、严格抛错 + thiserror 错误模式、容错白名单实践；本项目为单 crate 非 workspace，版本管理方式与 CapsuleRetro 的 workspace 单一来源不同，落地后可在此建立对照表）
