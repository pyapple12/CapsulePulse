# CapsulePulse — Agent Guide

玻璃质感的工作计时看板：上班/下班打卡开启一天，班内计时 = 工作、空隙 = 休息，下班出在岗/工作/休息三值与时间图谱；连续工作超阈值时声音 + 系统通知提醒休息，三端（Windows / macOS / Linux）通用。总体规划见 `CapsulePulse_plan.md`。

> **状态行已废弃**（2026-10-16 用户定案：AGENTS 不再维护项目状态行）。进度看 `x.progress.md`、方案看 `z.plan.md`、遗留看 `y.problems.md`、分期看 `CapsulePulse_plan.md`。

## 技术栈

| 组件     | 选型                                                                                                                                                                                                                          |
| -------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 框架     | Tauri 2（Rust 后端 + 系统 WebView）                                                                                                                                                                                           |
| 前端 UI  | Vue 3 + TypeScript + Vite（只做展示，业务零含量）                                                                                                                                                                             |
| 核心逻辑 | 纯 Rust（状态机/统计/持久化/提醒调度，cargo test 直测）                                                                                                                                                                       |
| 存储     | rusqlite（SQLite，`data/pulse.db`，双落址见目录规划）                                                                                                                                                                         |
| 玻璃效果 | CT 分态纱（PL021.4 照抄 CapsuleTODO：恒纯 alpha 透明窗 + 前端 ::before 纱层失焦 30%/聚焦 0%，双主题三态令牌锁暗；DWM 背板退役，仅留 Win11 窗口框架整定（激活描边隐藏 + 系统圆角，投影已关）；macOS/Linux 天然同构，仅需验证） |
| 通知提醒 | tauri-plugin-notification + 前端 `<audio>` 提示音                                                                                                                                                                             |
| 常驻     | Tauri 内置 tray API + 全局快捷键                                                                                                                                                                                              |

## 启动命令（规划）

```bash
npm run tauri dev      # 开发运行（透明玻璃窗口；Tauri CLI 走 npm devDep，见 .temp/pl001a-toolchain.md 决策；tauri 目录已改名 core/，CLI 按 tauri.conf.json 探测）
cargo test             # Rust 层全量测试（状态机/存储/提醒，不依赖 UI）
npm run dev            # 前端热更开发
npm run build          # 前端构建校验（含 vue-tsc；产物 dist/ 内嵌进 exe，改前端后先 build 再 cargo build）
```

依赖：Rust toolchain、Node.js（26+）、Tauri CLI；平台依赖——系统 WebView、通知服务、玻璃效果（Linux 需 compositor，见计划书 §2.4）。

## 架构要点

- **业务逻辑全在 Rust 侧**（项目学习目标，也是架构基线）：计时状态机、统计聚合、持久化、提醒调度全部放 core/src，前端只做展示与命令转发——禁止把业务逻辑写进 Vue 组件
- **计时状态机纯 Rust 可单测**（计划书 §2.1）：`Idle / Running { start } / Paused { elapsed }` 三态 + start/pause/resume/total() 四操作，无外部依赖；时间源可注入，跨零点/跨周边界用注入时间测试，禁止 sleep 真实等待
- **存储与落库时机**（PL027 重做，2026-10-13）：**`events` 单表为唯一事实源**（`clock_in`/`clock_out`/`auto_clock_out`/`segment_start`/`segment_end`/`overtime`/`session_reset`/`crash_clock_out`，只追加）+ **`runtime` 运行态缓存**（当前态 + 最后心跳；除心跳外均可由流水重建）+ **`days` 按天汇总**（已封口数据日，加速层、可随时重建）；会话落库时机 = **暂停时 + 在岗每分钟心跳 + 启动对账（崩溃 / 正常退出皆走：判据只看停机时长）**（停机 ≤10 分钟无缝续、>10 分钟按最后心跳补下班）——旧"崩溃最多丢当前段"的表述随 `workdays`/`sessions` 两表一并退役
- **工作日/打卡单一口径**（PL005 立、PL027 收敛）：**今日/本周/累计与三值/图谱全部走同一套单日归约**（`day_summary_inner`：流水 → 在岗窗口 → 工作/休息块），不再有"sessions 自然日口径 vs workdays 打卡口径"两套并存；数据日 = 上班打卡日（跨零点不切分）；"当前班"由最后一个 gap 之后的第一块起点推得
- **提醒调度**（计划书 §2.3）：阈值可配置（默认 50 分钟，设置界面修改、持久化）；tauri_plugin_notification 系统通知 + 前端 `<audio>` 提示音；通知与声音降级互不依赖
- **玻璃效果三端**（计划书 §2.4；PL021.4 CT 化定案）：恒纯 alpha 透明窗（tauri.conf transparent）+ 前端 CT 分态纱（::before 纱层失焦 30%/聚焦 0% 桌面直透，双主题三态令牌锁暗）——纯 CSS 实现**三端天然同构**，无 OS 材质依赖（原 DWM/vibrancy/blur 三分支随换血退役）；Rust 侧仅两件：转发 window-focus 焦点态 + setup 一次性隐藏 Win11 激活描边（`glass_backdrop.rs`，`DWMWA_BORDER_COLOR = NONE`——聚焦态 DWM 给活动窗口画一圈系统强调色描边，透明窗上表现为"窗口外面一圈蓝框"，CSS 不可达）；浮板毛玻璃 = 板上 0.25 黑纱 + backdrop blur40（窗内采样有效）+ 被盖内容自雾化 blur4；macOS/Linux 适配项收敛为常规回归验证 [problems#1]
- **常驻形态**：托盘/菜单栏常驻、关闭最小化到托盘、全局快捷键唤起、后台持续计时
- **运行时数据**：config.json 落 `configs/`、pulse.db 落 `data/`——dev=项目根、release=exe 同级（双落址，2026-09-10 热更新定案杜绝机器用户目录）；备份 = 直接拷 configs/ + data/（计划书 Phase 4）

## 目录规划

（2026-09-08 目录风格改造后实态；根目录分类学 = ui / core / configs / data / modules，modules 待规模需要再建）

```
core/             # Tauri 2 后端（原框架默认名 src-tauri，整体改名；框架文件须与 Cargo.toml 同住）
  Cargo.toml      # 版本单一来源（version 三段式 X.Y.Z）
  tauri.conf.json # version 字段省略（回落 Cargo.toml）
  src/
    lib.rs            # 应用装配：玻璃挂载 + 模块注册
    main.rs           # 薄入口（调 capsule_pulse::run()）
    session.rs        # 计时状态机（纯逻辑，可单测；业务纯逻辑平铺于此，禁 import tauri）
    workday.rs        # 工作日状态机 + 事件归约 reduce_day（PL005；打卡/图谱纯逻辑，注入时间可单测）
    commands/         # Tauri 命令层（PL001 阶段 D；按职责分文件：mod 上下文与共享 / session 会话 / stats 统计 / reminder 副作用与设置 / workday 打卡与单日明细 / hover 托盘悬浮概览 / tray_menu 自绘菜单窗 / window 窗口级命令）
    storage.rs        # SQLite Repository（PL002；**PL027 重做**：events 单表为唯一事实源 + runtime 运行态缓存 + days 按天汇总——旧 sessions / workdays 两表已退役）
    period.rs         # 统计周期边界纯函数（PL002）
    reminder.rs       # 提醒评估器（PL003）
    settings.rs       # 提醒设置持久化（PL003；PL005 增自动下班小时数字段；PL028.3 增置顶 / 贴边开关；PL028.2 增两条全局热键）
    hotkey.rs         # 全局热键组合串解析与规范化纯函数（PL028.2：修饰键必需 + 单主键 + 固定序归一；PATCH010.4 增段数上限 3；注册运行时由 tauri-plugin-global-shortcut 承担）
    hover.rs          # 托盘 hover 今日概览归约纯函数（PL028.1：四状态 + 剩余/倒计时算术，命令层只取数）
    menu.rs           # 托盘右键自绘菜单窗（PATCH008：锚定计算纯逻辑 + 点外收起守候线程 + 锚点状态；建窗在 lib.rs）
    snap.rs           # 贴边吸附纯函数（PL028.3：四边 ±50px 判定 / 5px 落位 / 幂等，被 lib.rs 的 Moved 状态机消费）
    diag.rs           # 诊断日志（FIX002.7：data/pulse.log；PL024.2 起 cfg(test) 不落盘）
    paths.rs          # 运行时数据双落址（config.json→configs/、pulse.db→data/；dev=项目根 / release=exe 同级）
    glass_backdrop.rs # 窗口框架与材质（PATCH001.3/.5：隐藏 Win11 激活描边 + 补系统圆角；PL029：按"板开合 + 焦点"挂摘系统背板；DWM 属性级 FFI，零新依赖）
    aumid.rs          # 通知署名注册（PATCH012：首启把内嵌图标解出到 data/ + 写 HKCU 的 AppUserModelId 三项——让绿色单 exe 的通知署名 = 应用名 + 图标；手写 advapi32 FFI，零新依赖）
ui/               # Vue 前端（展示层）
  App.vue             # 骨架：topbar + 计时页 + 浮层件装配（板开合 / 文案条 / 确认框 / 背板时序）
  components/         # 计时页与浮板
    TimerCard.vue     # 大计时器 + 表盘 + 沙漏 + 电池 + 打卡 pill（未上班置灰）
    StatsView.vue     # 统计板：时间图谱 + 三值 + 段明细 + 周卡 + 前后日翻看（PL005）
    SettingsPanel.vue # 设置板七行（PL019 重装；PATCH009/PATCH010 整定）
    ConfirmModal.vue  # 玻璃确认框（打卡双向确认，PL005）
  src/
    components/       # 独立窗口件：TrayHover.vue（托盘悬浮卡）/ TrayMenu.vue（自绘右键菜单）
    styles/           # 样式层（glass / topbar / timer / hourglass / stats / graph / boards / controls / glass-bar）
    dev/              # mock-invoke（无 Tauri 外壳时的 DEV 假 IPC 基座）
    title-particles.ts / use-glass-bar.ts / use-threshold-drag.ts # 视觉与交互件
  types.ts            # IPC DTO 类型镜像（与 Rust serde 同源）；format.ts 展示格式化助手；main.ts 入口
configs/          # 程序读的固定参数与用户参数（预建占位，首个实体文件出现于设置持久化落地时）
release/          # 发布目录（PATCH012）：release/README.md 入库（随包使用说明，中英双语 + 更新日志）；exe 与 zip 二进制 gitignore（由各版本构建产出、zip 走 GitHub Release 挂载）
assets/           # 提示音、图标
.agents/skills/   # 项目自建 skill（audit-project / audit-report / progress-task）
.temp/            # 临时脚本与文件（gitignore）；探针、验证记录放这里
y.problems.md     # 问题与远期改进备忘录（只增不删、编号递增；任务清单以 [problems#N] 引用）
```

## 任务清单纪律（x.progress.md，2026-09-10 定案）

- **条目做法必须写到文件/函数级**：`—— 做法` 部分要指明改哪个文件、哪个函数/模块、新增什么结构/命令/表、怎么改（细到可照做）；禁止一句话概括后只留验证方式（2026-09-10 PL005 初版条目因做法缩略被退回重写，此为先例）
- **验证方式必须可执行、可断言**：写明用例名/命令/live 步骤；TDD 任务先写红灯用例再实现
- **编号轨道三线**（2026-10-11 定案，自系列项目 CapsuleTODO 引入）：`PL{NNN}` 功能里程碑 / `FIX{N}` 审计修复（由 audit-report 生成）/ `PATCH{NNN}` 小修小改——用户日常提出的小功能与小调整（不源自审计、不走审计轮），编号自 PATCH001 递增；方案入 z.plan.md 附录 `PATCH{NNN}`、执行条目入 x.progress.md `### PATCH{NNN}` 组，做法/验证标准与 PL 同规
- **A/FIX 编号绑定**（2026-10-11 定案，自系列项目 CapsuleTODO 引入）：审计报告附录 `A{NNN}` 与其修复任务组 `FIX{NNN}` **恒同号**（A005↔FIX005 既有实践即此规）；取号 = 下一可用 FIX 号（`max+1`，A 侧不独立取号）；轮次叙述（"第 N 轮"）与编号解耦——FIX 号被非审计批（PATCH / 观察项清理批等）占用时，审计顺延取号防撞号
- **已完成组全量保留、只增不删**；两个区内部从上到下 = 从旧到新，新组追加在区末尾（详见 x.progress.md 头部规则）
- 立项方案（z.plan 附录）的实现措施与任务条目同标准：一律拆到文件/函数级

## 自动格式化与静态检查（硬性工作流，替代 LSP）

> ZCode 无自动 LSP / Formatter 挂载机制，本节即强制工作流：**改完必须跑，不通过不得宣称完成**；环境缺工具时先报告用户，不得静默跳过。

- **编辑任何 `.rs` 文件后**：依次运行 `cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo check`，三条全过才算完成
- **编辑前端文件（`.ts` `.tsx` `.vue` `.css`）后**：运行 `npx prettier --write <file>`；若改动含逻辑（非纯样式），还需 `npx vue-tsc --noEmit` 通过
- **编辑 `.md` 后**：运行 `npx prettier --write <file>`（本文件与计划书等均适用）
- **Rust 文档注释与签名变更后**：`cargo doc --no-deps` 确认无 broken intra-doc link（可与其他检查合并跑）
- **提交前全量门禁**：`cargo fmt --check` + `cargo clippy -D warnings` + `cargo test` + `npm run build`

## 工程原则（设计哲学，所有项目通用）

> 设计哲学总纲（18 条 / 5 大类，与用户级 instructions.md 同源）；下文"代码规范"为项目细则，两者冲突时以本项目边界为准。

### 核心思想

- 以第一性原理思考问题：理解需求背后的真实目标，而非直接套用已有模式或技术方案。
- 优先解决本质问题，避免为假设中的未来需求提前设计复杂系统。
- 在保证长期可维护性的前提下，选择当前最简单、可靠、清晰的实现方案。

### 简洁与设计

- 遵循 KISS：优先选择简单直接的实现，避免不必要的复杂度。
- 遵循 DRY：避免重复逻辑，但不要为了消除少量重复而创建过度抽象。
- 遵循 SOLID 思想：职责清晰、降低模块耦合，提高可维护性和扩展能力。

### 架构

- 不长期保留废弃方案：优先删除过时代码，而不是增加兼容层、fallback 或临时迁移逻辑。
- 不进行未经验证的架构设计：避免提前引入抽象、配置和间接层。
- 从最小可工作的版本开始逐步演进，每次修改建立在已有可运行系统之上。
- 永远不要用未来可能需要的复杂性，牺牲当前产品的可用性。

### 代码质量

- 保持模块职责明确，避免一个模块承担过多职责。
- 优先使用成熟、稳定、维护良好的第三方库，而不是重复造轮子。
- 使用项目已有依赖解决问题之前，不要随意新增依赖。
- 在引入新方案前，先检查已有代码、依赖、文档和能力。
- 避免为了"看起来更优雅"而增加实际复杂度。

### 工程决策

- 优先选择长期可维护的方案，而不是只能临时运行的解决方案。
- 代码应该服务于业务目标，而不是为了展示技术复杂度。
- 如果简单方案已经满足需求，不要主动升级为复杂方案。

## 函数注释规则（Rust / TypeScript）

- **Rust**：每个函数定义上方紧跟 `///` 文档注释（中文），说明用途和核心逻辑（1-3 行）；公开 API 的参数、返回值、panic 条件用 `/// # 参数` / `/// # 返回` / `/// # Panics` 小节补充；**禁止无文档注释的 `pub` 函数**
- **Rust 模块**：每个 `.rs` 文件顶部用 `//!` 模块注释说明文件职责、设计理由（为什么这样做）、关联的配置或外部依赖
- **TypeScript / Vue**：导出函数与组合式函数用 `/** */` 中文注释说明用途和核心逻辑（1-3 行）；复杂内部函数同样注释
- **禁止以注释替代类型**：能用类型签名表达的约束（`Option` / `Result` / 联合类型）不写进注释
- 单行 `//` 注释用于解释"为什么"，不复述代码本身做什么

## 代码约定（Rust + Vue/TS）

- **注释语言**：所有注释与文档注释使用中文；注释符号：Rust 用 `//` `///` `//!`，TS/Vue 用 `//` `/** */`，禁止其他语言注释符号
- **命名风格**：Rust 由编译器强制——函数/变量 `snake_case`、类型 `CamelCase`、常量/静态 `UPPER_CASE`；TS/Vue 变量函数 `camelCase`、类型/组件 `PascalCase`、常量 `UPPER_CASE`
- **私有性**：Rust 模块内部使用收敛可见性（默认私有，跨模块用 `pub(crate)`，慎用 `pub`）；TS 模块内部用 `_` 前缀表示内部私用，外部模块不应直接调用
- **入口**：可执行 crate 有 `fn main()`；Tauri 命令参数与返回值走 serde 类型定义
- **类型**：Rust 强类型天然覆盖；TS 禁止 `any`，优先显式标注，接口用 `interface`/`type`，跨进程数据结构用 serde + TS 类型定义单一来源
- **数据结构**：配置聚合优先用 Rust struct + `serde::Deserialize`（对应设置持久化 / 通知参数 / 窗口配置），不做散装 HashMap 取值
- **use / import 顺序**：Rust——标准库 → 第三方 crate → 本地 crate/模块，组间空行（rustfmt 默认分组）；TS——外部包 → 内部模块，组间空行
- **字符串格式化**：Rust 优先 `format!("{var}")` 内联变量写法；TS 优先模板字符串，避免 `+` 拼接
- **集合构建**：Rust 优先 iterator 适配器链（`map`/`filter`/`collect`）而非手写 for 循环构建集合
- **布尔判断**：用 `if x` / `if !x`（TS 用 `if (x)` / `if (!x)`），而非与字面量显式比较
- **空值判断**：Rust 用 `Option::is_none()` / `is_some()` 及 `match`/`if let`；TS 用 `x == null` / `x != null` 同时覆盖 null 与 undefined，禁止与 `undefined` 单独比较
- **错误处理**：Rust 禁止在业务代码散落 `unwrap()`/`expect()`——核心逻辑用 `thiserror` 定义错误类型，应用顶层用 `anyhow`；错误必须向上传播或明确记录，禁止空 `catch`/`let _ =` 吞错；TS 避免 `catch {}`，至少记录日志
- **行长度**：每行尽量不超过 100 字符（rustfmt `max_width = 100` 默认值；超过时在运算符或逗号后换行）
- **格式化自动化**：空格、缩进、引号、尾逗号等排版细节**以 rustfmt 与 prettier 输出为准**，不手工对齐；普通字符串双引号（TS 由 prettier 配置保证）
- **路径处理**：Rust 强制使用 `std::path::Path` / `PathBuf`，禁止手拼字符串路径；运行时数据目录经 dirs crate / 配置解析，禁止硬编码机器路径；TS 使用 `path.join` / `URL`，禁止字符串拼接路径
- **临时文件**：所有临时生成的脚本/文件必须写入项目根目录下的 `.temp/` 文件夹（gitignore），禁止散落在源码目录
- **文件修改必须用 edit 工具**：修改既有文件（.rs/.ts/.vue/.md/.json/.toml）一律用 edit 的精确 oldString/newString 替换，禁止用 python -c 或 PowerShell 脚本做内容替换（易踩引号/缩进坑）；新建 .temp 探针脚本不受限
- **版本双轨**：机器版本只存 `core/Cargo.toml` 的 `version`（三段式 X.Y.Z，Cargo 强制 semver，本项目单 crate 非 workspace）；对外正式文本（commit 标题、README 徽章、发布说明）一律写四段式 `VX.Y.Z.R`——前三段与 Cargo.toml 一致，第四段 R 为修订号，只随提交历史记录、不入任何配置文件（递增规则见"Commit 提交规范"）；`tauri.conf.json` 省略 version 字段（Tauri 回落读 Cargo.toml），`package.json` 的 version 为 npm 生态必填字段、不参与发布——豁免于单一来源，不派生不注入（对齐系列项目 FIX001.25 豁免先例）
- **Shell 命令**：执行命令前先检测当前 shell（`echo $SHELL` 或检查系统），本机 Windows 环境；避免使用 Linux-only 或 PowerShell-only 工具，优先跨 shell 兼容命令
- **PowerShell 中文编码**：bash 工具的 pwsh 会话带 `-NoProfile`，不加载 `$PROFILE`，输出中文前必须先设置编码：`[Console]::OutputEncoding = [System.Text.Encoding]::UTF8;`
- **Git 操作**：未经用户明确要求，不得擅自执行 `git add`、`git commit` 或任何其他 Git 写操作
- **UI 提示时长（全应用统一）**（PATCH010.5 用户定案，2026-10-16）：**错误提示（红字）10s 自动消失、非红字提示 5s 自动消失**——文案条与板内红字（板底保存失败 / 行内录键失败）同一口径，常量 = `ui/App.vue` 的 `ERROR_HIDE_MS` / `HINT_HIDE_MS`。新增任何提示必须按此口径在前端设自隐计时，**禁止常驻提示**；错误详情一律进 console，UI 只显示短文案（设置板行宽有限，长文案必换行）

## Commit 提交规范

- **标题行**：`<type>: V<版本>，<摘要>`——版本为四段式 `VX.Y.Z.R`（如 `V0.1.0.1`）：前三段与 `core/Cargo.toml` 的 `version` 一致，第四段 R 为修订号（不入配置文件，见下条）；摘要一句话概括核心
- **版本递增**：`feat`/`fix`/`refactor`/`perf` 提交前先 bump 第四段 R（R+1）；前三段在 `Cargo.toml` 中推进时 R 回到 1；`docs`/`test`/`style`/`chore` 不强制
- **type 全集**（conventional 风格）：`feat` 新功能 / `fix` 修复 / `refactor` 重构（行为不变）/ `perf` 性能 / `docs` 文档 / `test` 测试 / `style` 格式 / `build` 构建依赖 / `ci` CI / `chore` 杂项 / `revert` 回滚
- **正文**（默认必写，仅 docs/style 微调可省）：`- ` 列表逐条"具体做了什么"，每条一个功能块、一行自然中文、≤ 100 字符；体例＝`主题：内容（细节；细节）`，同主题多细节用 `；` 分隔。样例：`- 计时状态机：Idle/Running/Paused 三态 + start/pause/resume/total 四操作（暂停继承累计、时间源可注入）`
- **禁止项**：内部编号（PL001.1/FIX001.2 等任务编号）、验证/回归数字（如"全量回归 506 项通过"）、英文混排描述
- **提交范围**：一个版本的所有连带改动一次提交（源码 + 配置 + 文档同步）
- **GitHub 身份与远程**：远程 `git@takechance:pyapple12/CapsulePulse.git`（SSH 别名 `takechance` = pyapple12 账号，密钥 id_b_takechance，经 socks5 127.0.0.1:10808）；仓库级身份 `git config user.name pyapple12` + `user.email takechance_bao@188.com`（覆盖全局 YiYi 身份，与 CapsuleRetro 同款做法）
- **流程**：每次提交前 AI 必须 `git status` + `git diff`（含 `--stat`）核对改动范围无误，再草拟完整 commit 内容（add 清单 + 标题 + 正文）交用户确认；用户审阅后自行执行 `git add`/`git commit`/`git push`（AI 不执行任何 git 写操作）

## 发布注意事项（Release 流程，2026-10-16 定案）

> 目标：绿色单 exe 发版一条龙（改说明 → 建 exe → 打包 → GitHub Release），下次照此执行。**发布动作（建 tag / Release）由用户执行**。

### 命名与口径

- **tag 与 Release 标题 = 三段式** `CapsulePulse V<X.Y.Z> —— <主功能摘要>`（如 `CapsulePulse V0.2.0 —— 首个公开版本`）；**不用四段 R**（R 只进 commit 标题，见 Commit 提交规范）
- **资产名** `CapsulePulse_v<X.Y.Z>_win_x64.zip`（zip 内 `capsule-pulse.exe` + `README.md`，根目录平铺，不带 `configs/`、`data/`）
- **Release 正文与 `release/README.md` 均中英双语**：先**整段中文**、再**整段英文**（不逐段对照）
- **Release 正文结构**：🚀 更新内容（✨ 新特性 / 🐛 修复 / 📝 说明）+ 对应英文段；**「修复」从 `git diff <上个 tag>..HEAD` 全量提取**、每条一句话
- **README 双口径**（沿版本双轨）：徽章 = 代码版本（随 `Cargo.toml` 即时推进）；下载链接 = 实际发布产物（**发版后才改**，提前改 = 指向不存在文件 404）
- `release/README.md` = 随包使用说明（入库、版本化）；脚注版本号随发版更新

### 发布流程（顺序）

1. **改说明文件**：`release/README.md`（补新功能 + 更新日志 + 脚注版本）、`README.md`（徽章 + 双语下载链接 + 功能亮点）
2. **全量门禁**：`cargo fmt --check` / `clippy -D warnings` / `cargo test` / `cargo doc --no-deps` / `vue-tsc` + `vite build` / `prettier`
3. **提交并推送**（一条提交带齐源码 + 配置 + 文档同步）
4. **构建发布 exe**：`npm run tauri build -- --no-bundle`（`--no-bundle` 免安装器，只要绿色单 exe；`beforeBuildCommand` 自动重编前端）
5. **拷 exe 入 `release/`**：`core/target/release/capsule-pulse.exe` → `release/capsule-pulse.exe`
6. **打 zip**：`Compress-Archive -Path release/capsule-pulse.exe,release/README.md -DestinationPath release/CapsulePulse_v<X.Y.Z>_win_x64.zip -Force`（**直接用 `release/` 打包，不另建 staging**）
7. **打 tag**：`git tag -a v<X.Y.Z> -m "CapsulePulse V<X.Y.Z>——<摘要>"` + `git push origin v<X.Y.Z>`（annotated）
8. **建 GitHub Release**：挂 zip 资产，标题/正文按上面口径
9. **发布自查**：Release 页标题 / 资产名 / 下载链接 404 核对；`README.md` 徽章与链接口径

### 注意点（坑）

- **`release/*.exe`、`release/*.zip` 已 gitignore**（二进制不入库）；仅 `release/README.md` 入库
- **`--no-bundle` 必加**：默认会跑 Windows 安装器打包（NSIS/MSI），绿色单 exe 分发不需要
- **构建前停运行实例**：exe 被占用时 cargo 无法覆盖产物（os error 5）
- **tag 必须指向含最终文档的提交**：先提交文档再打 tag（tag 钉住发布点）
- **gh 发布权限**：本机 `gh` 登录账号对 `pyapple12/CapsulePulse` 可能仅 READ ⇒ `gh release create` 会 403；Release 由用户手动发布，或先切到有写权限的账号 / `GH_TOKEN`
- **修复项全量**：Release 的「修复」从 `git diff` 逐条核对，不漏
- **图标**：`core/icons/` 需五件套（`32x32.png` / `64x64.png` / `128x128.png` / `128x128@2x.png` / `icon.ico` / `icon.png`）并与 `tauri.conf.json` 的 `bundle.icon` 数组同步；`icon.png` 同时供通知署名 `IconUri`（`aumid.rs` 的 `include_bytes!` 编译期嵌入）⇒ **换图标后必须重新构建 exe**
- **通知署名**：绿色版靠 `aumid.rs` 首启写 HKCU 项署名（`SOFTWARE\Classes\AppUserModelId\<AUMID>`）；**AUMID 必须与 `tauri.conf.json` 的 `identifier` 一致**（通知插件同源取值，不一致则署名仍回退 PowerShell）

## 错误策略

主线：**严格抛错**——配置缺失/非法输入直接返回错误（Rust 返回 `Result` 并定义明确错误类型；TS 抛出具体 Error），不静默兜底、不降级假成功。

容错白名单（初始为空；仅产品明确允许的边界可容错，**新增容错须先在此登记**，写明场景、降级行为与理由三要素）：

- **提醒设置文件不存在回退默认**（PL003.3 登记，2026-09-09）：场景——首次启动或用户删除 config.json，载入时 NotFound；降级行为——返回默认设置（阈值 50 分钟/声音开/通知开）不报错；理由——首启无设置文件是正常态而非错误，回退默认即"开箱即用"；JSON 损坏等其余错误仍严格报错不在此列。
- **系统通知发送失败降级仅声音**（PL003.8 登记，2026-09-09）：场景——tauri-plugin-notification 发送失败（部分 Linux DE 无通知服务/权限拒绝等）；降级行为——错误落日志，reminder-due 事件照发（前端提示音与文案不受影响）；理由——计划书 §2.3 定案通知与声音降级互不依赖，通知失败不应吞掉提醒功能本体。
- **提示音播放失败降级仅通知**（PL003.8 登记，2026-09-09）：场景——前端 `<audio>` play 被浏览器策略拒绝或音频资源缺失；降级行为——console.warn 记录，系统通知与提醒文案不受影响；理由——同上，双通道互不依赖，任一失败不吞提醒。
- **退出前落库失败仍退出**（A002-P3-7 登记，2026-09-12）：场景——托盘退出时 persist_before_quit 失败（存储故障等）；降级行为——错误落诊断日志（data/pulse.log）后照常 `app.exit(0)`；理由——退出意图优先，落库失败多为持久性故障（db 损坏/被锁）重试无门，不能因数据保存把用户困在应用里。
- **设置既有文件字段缺失回退默认**（A002-P3-16 登记，2026-09-12）：场景——config.json 存在但缺个别字段（旧版本残留/手改丢失，如缺 threshold_min）；降级行为——该字段经 serde default 补默认值（auto_out_hours 有 PL005.4 注释依据），其余字段照常读入；理由——与"文件不存在回退默认"同义外延，属开箱即用而非错误；字段类型非法仍严格报错不在此列。
- **诊断日志写失败静默忽略**（FIX002.7 登记，2026-09-12）：场景——diag::log 写 data/pulse.log 失败（磁盘满/目录不可写）；降级行为——静默忽略不报错；理由——日志自身不得引发二次故障，否则"记录错误"反成新错误源。
- **焦点联动分态纱事件失败维持前态**（FIX003.7 登记，2026-09-13；PL021.4 修订——DWM 背板随 CT 化退役，剩余适用面收窄为事件转发）：场景——窗口焦点切换时 window-focus 事件发送失败；降级行为——错误落诊断日志（data/pulse.log），纱态维持切换前状态；理由——分态纱为纯装饰层，运行时焦点事件不可中断主流程，下次焦点切换自动重试自愈。
- **窗口框架整定失败维持前态**（PATCH001.3/PATCH001.5 登记，2026-10-11）：场景——setup 整定时 `DwmSetWindowAttribute` 返回非零 HRESULT 或取不到窗口句柄（① 激活描边隐藏 `DWMWA_BORDER_COLOR`；② 系统圆角 `DWMWA_WINDOW_CORNER_PREFERENCE`——Win10 无此属性属预期）；降级行为——错误落 dev 控制台（描边隐藏另落诊断日志 data/pulse.log；系统圆角**不落 diag**，免 Win10 每次启动写常驻噪音），窗口保持系统默认（聚焦态仍有一圈系统强调色描边 / 上角为方角），**不中断启动**；理由——窗口框架整定为纯装饰层，失败唯一后果是观感回退（计时/打卡/统计/提醒/托盘全不受影响），与上一条同一条纪律（装饰层不阻断主流程）。
- **按天汇总与运行态心跳失败仅落档不阻断主流程**（PL027.3/PL027.7 登记，2026-10-13）：场景——`seal_days_inner` 滚动"已封口数据日"时失败（存储故障/锁中毒等）、`beat_inner` 写运行态缓存失败、或启动/下班后触发的那一轮失败；降级行为——错误落诊断日志（`data/pulse.log`），主流程（打卡/下班/统计读取）照常；理由——按天汇总与运行态缓存都是**加速/可重建层**（心跳由流水 + 动作即刻双写补足），可由流水随时重建（与"诊断日志写失败静默忽略"同一条纪律：加速层不得反噬业务）。
- **启动迁移失败仅落档不阻断**（FIX006.9 登记，2026-10-17）：场景——`migrate_on_start` 一次性迁移旧模型（`workdays`/`sessions` → `events` 单表）失败（旧库形态异常 / 校验不过 / 磁盘或锁故障）；降级行为——错误落诊断日志（`data/pulse.log`），**旧两表原样保留**、`model_version` 不置位，下次启动重试（迁移幂等，已迁过即跳过）；理由——新代码只读 `events` 流水、旧表留存无害，迁移是一次性后台动作，不得把用户挡在应用外（与"按天汇总与运行态心跳失败仅落档不阻断"同一条纪律：加速/迁移层不得反噬业务）。
- **窗口控制与位置记忆失败仅日志不阻断**（PATCH001.2/PATCH001.4 登记，2026-10-11）：场景——① 隐藏失败（`window.hide()`；事件/托盘路径 = Alt+F4 / 托盘左键 / Alt+Shift+S）；② 位置记忆读失败（`outer_position` / 设置锁中毒）与写失败（config.json 落盘）；③ 落位失败（`set_position` / 显示器枚举失败）。降级行为——① 事件/托盘路径错误落诊断日志（data/pulse.log）+ dev 控制台后照常继续；② **双击标题路径不降级**：命令层把错误上抛给前端错误条（严格抛错主线）；③ 位置相关失败退化为"不记忆"或"回默认右上落位"。理由——窗口控制与位置记忆都是体验层，失败不应挡住"隐藏/退出"这类用户意图（与"退出前落库失败仍退出"同一条纪律）。

- **贴边吸附与窗口偏好失败仅日志不阻断**（PL028.3 登记，2026-10-15）：场景——① 吸附判定读设置锁中毒（`snap_if_needed`）；② 吸附 `set_position` 落位失败；③ 托盘右键菜单偏好切换的读取 / 落库 / 置顶应用失败；④ `prefs-changed` 广播或托盘菜单重建（勾选态刷新）失败；⑤ 启动 setup 与设置保存时的置顶应用失败。降级行为——① 跳过本轮吸附；②③⑤ 错误落诊断日志（`data/pulse.log`），本次改动不生效（勾选态由下次事实源读取纠正、置顶由下次启动 setup 按 config 重应用自愈）；④ 跳过本轮同步（前端开关回显与托盘勾选态停在旧值，下次任一路径改动即自愈）。理由——吸附 / 置顶 / 勾选态同步都是**体验装饰层**，失败唯一后果是观感回退（计时 / 打卡 / 统计 / 提醒 / 托盘功能全不受影响），与"窗口控制与位置记忆失败仅日志不阻断"同一条纪律。
- **全局热键注册失败仅日志不阻断**（PL028.2 登记，2026-10-15）：场景——① 启动 `register_global_shortcuts` 注册失败（热键被其它程序占用、或 config.json 被手改成非法组合串）；② 撤旧键（`unregister`）失败。降级行为——① 错误落诊断日志（`data/pulse.log`）+ dev 控制台，**该条热键本次不可用但应用照常启动**（面板里录一条新组合即恢复；退出重开按 config 重试）；② 仅记日志继续换新键（旧键可能本就没注册成功）。理由——全局热键是**便利层**（主窗显隐另有托盘三入口、计时切换另有界面按钮，功能不缺），启动期不得因便利层失败把用户挡在应用外；设置面板录键路径**不降级**（冲突/非法由 `set_hotkey` 严格上抛 + 回滚旧键 + 行内红字，属用户主动操作的即时反馈）。

- **托盘悬浮概览失败仅日志不阻断**（PL028.1 登记，2026-10-15）：场景——① 悬停时聚合失败（存储故障 / 锁中毒）；② 悬浮窗尺寸设置、概览推送（`hover-update`）、锚定或显示 / 收起失败；③ 托盘图标矩形取不到（`Enter` 事件非物理坐标）。降级行为——① 本次不显示（错误落诊断日志 `data/pulse.log`）；② 错误落诊断日志，该步跳过（尺寸失败仍显示、推送失败前端保持上一次内容、锚定失败停在原位、显示/收起失败下次悬停自愈）；③ 跳过本次（记录不到锚点即不显示）。理由——悬浮概览是**只读装饰层**（今日三值/剩余/倒计时在主窗与统计板都可看，功能不缺），失败唯一后果是"少一个便利视图"，与"窗口控制与位置记忆失败仅日志不阻断"同一条纪律。

- **系统背板切换失败仅日志不阻断**（PL029 登记，2026-10-16）：场景——① `DwmSetWindowAttribute(DWMWA_SYSTEMBACKDROP_TYPE)` 返回非零 HRESULT；② 取不到主窗口句柄；③ Win10 无该属性（Win11 22H2+ 才有）。降级行为——错误落诊断日志（`data/pulse.log`）+ dev 控制台，**维持前态**（观感回现状：板开时仍是纯透明），下次切换自愈。理由——系统背板是**纯装饰层**（只在板开合时切换的材质），失败唯一后果是观感回退（计时 / 打卡 / 统计 / 提醒 / 托盘全不受影响），与"窗口框架整定失败维持前态"同一条纪律。

- **通知署名注册失败仅日志不阻断**（PATCH012 登记，2026-10-16）：场景——① 图标解出到 `data/aumid-icon.png` 失败（目录不可写 / 磁盘满）；② `RegCreateKeyExW` / `RegSetValueExW` 返回非零（组策略限制 / 注册表被锁）。降级行为——错误落诊断日志（`data/pulse.log`），**通知照常发送**但来源署名回退 "Windows PowerShell"（crate 内置回退），下次启动重试自愈。理由——署名是**观感层**（通知内容/时间/触发时机全不受影响），不得因装饰性注册把用户挡在应用外；与"窗口框架整定失败维持前态"同一条纪律。

## 素材与环境陷阱

- **运行时数据不入仓库**：config.json（configs/）与 pulse.db（data/）是用户运行时数据——dev 落项目根、release 落 exe 同级，均已 gitignore（双落址，2026-09-10 定案）；测试用临时 db 一律落 `.temp/` 或系统临时目录，禁止对真实用户数据做测试写入
- **测试不得污染运行时日志**（PL024.2 登记，2026-10-11）：单测在 debug 构建下运行时根 = 项目根，`diag::log` 会写真实 `data/pulse.log`——曾把"故意污染锁"用例的回滚噪音写进真实日志，被误判为"实机锁中毒"（problems#5）。`diag::log` 已按 `cfg(test)` 隔离（测试不落盘）；新增任何会写日志的测试路径同样不得落真实运行时文件（临时文件一律 `.temp/`）
- **玻璃效果平台差异**：macOS 真 vibrancy / Windows Acrylic / Linux 依赖 compositor（计划书 §2.4/§8 风险项）；当前仅 Windows 实机可验——改玻璃相关代码须注明"Windows 实机验证 + macOS/Linux 按分支逻辑推演"，三端实测按计划 Phase 4 排期，不提前宣布跨平台可用
- **提醒通知三端差异**（计划书 §2.3/§8）：通知与声音可能分别不可用，降级互不依赖；通知/声音相关改动要单独验证降级路径，不能只测正常路径
- **深浅色主题**：跟随系统（计划书 §4）；硬编码纯色会破坏主题感知，玻璃卡片用半透明 + 系统模糊适配两种主题
- **Rust 学习曲线**（计划书 §8）：状态机/存储先写纯逻辑 + 单测、小步迭代；时间相关逻辑（跨零点、跨周）用注入时间源测试，禁止 sleep 等真实等待；Mutex/生命周期报错及时求助不硬扛
- **Tauri 2 ACL 静默拒**：前端能力调用（事件 listen、`data-tauri-drag-region` 拖动、invoke 等）走 ACL 白名单，未授权时**静默失效、控制台零报错**（系列两例：listen 需 `core:event`、拖动需 `core:window:allow-start-dragging`，`core:default` 均不含）；权限事实源 = 构建产物 `core/gen/schemas/acl-manifests.json`；新增前端能力调用后必须 live 验证功能生效，构建绿不代表功能可用。**新增第二个窗口必须同步 `core/capabilities/default.json` 的 `windows`**（PATCH007.1 实例：悬浮窗 `tray-hover` 未入列 ⇒ 其 `listen` 被静默拒 ⇒ 卡片数据冻结在首次取数；**自定义命令不经 ACL 故 `invoke` 照常通**，症状是"能显示、不更新"——这类"半通"最易误判为业务 bug）
- **DEV 基座无窗口拖拽层**（PATCH002.2 登记，2026-10-11）：`ui/src/dev/mock-invoke.ts` 的浏览器基座只有命令 mock，**没有窗口级 mousedown 拖拽层**——凡由拖拽白名单（`ui/App.vue` 的 `DRAG_INTERACTIVE`）吞掉点击引发的问题，在 DEV 基座里**必然复现不出**，四步实测全绿也判不了通过（实例：三段控件「工作/休息」是 `<label>` 未入列 → 按下即被 `startDragging` 抢走鼠标捕获、click 永不派发，表现为"按了没反应"；"重置"是 `<button>` 在列，故只有它能用）；此类交互必须 live 实机点击验证，DEV 只用来验前端状态与渲染（与上条同属"构建绿 ≠ 功能可用"）
- **运行中 exe 锁文件**：调试运行 capsule-pulse.exe 期间 cargo build 无法覆盖产物（os error 5）——重建前先停运行实例（2026-09-08 实测）
- **停旧实例后勿立即重启**（PATCH012 实测一次，2026-10-16）：`Stop-Process` 后仅约 2 秒就 `Start-Process` ⇒ 新实例**日志停在"启动：缓存中存在未闭合班（待对账）"、`Responding=False`**（setup 阻塞——主窗已由 builder 建好故 UI 正常渲染、命令层照常响应，阻塞点疑似第二窗 `tray-hover` 的 webview 创建：旧实例的 WebView2 进程/环境尚未完全退出）；等 **4 秒以上**重启即一切正常（日志立刻续上"启动恢复：无缝续"）。⇒ **重启流程固定为：停实例 → 等 ≥4 秒 → 再启动**；若发现"日志停在待对账 + Responding=False"，直接杀掉重来即可
- **验证禁止更改电脑系统设置**：live 验证一律以系统当前态进行（当前 = Windows 深色主题 + 动画效果开）；禁止为验证去切换主题、动画效果（reduced-motion）等系统设置——替代手段 = 代码走查确认退避/令牌规则在位 + 用户日常使用复核（2026-09-13 用户定案，M2/M3 类验证照此执行）
- **dev 无 HMR**：`beforeDevCommand = npm run build`、`frontendDist = dist`（静态产物）——改前端后必须重新 build + 重启 dev 流程才生效，热更不存在（2026-09-13 PL008 实测）。**补充（PATCH001 实测 2026-10-11）**：产物是**编译期嵌进 exe** 的——改源文件或 dist 都不会影响已编译二进制，必须 `npm run build` + `cargo build` + 重启三步（实证：仅重建 dist 即触发 crate 重编、exe 哈希变化）；且**前端构建产物进二进制后 cargo 才重编**，所以 `npm run tauri dev` 的 beforeDevCommand 与 cargo 构建不能并行跑 `vite`（见下条）
- **vite dev 与 cargo 构建不可并行**（PATCH001 实测，2026-10-11）：`npm run dev` 以项目根为 root，会去 watch `core/target/**`；cargo 重建时覆盖 `target/debug/deps/*.exe` → vite 抛 `EBUSY: resource busy or locked` 直接崩（探针会话被中断一次）。跑浏览器探针前先停 cargo 构建，或探针跑完再构建
- **DPI-unaware 探针坐标会虚拟化**（PATCH001 实测，2026-10-11）：pwsh 默认 DPI-unaware，`MoveWindow`/`GetWindowRect` 在 125% 缩放下拿到的坐标被系统虚拟化，与 app 上报的物理坐标（Tauri `outer_position`）差 1.25 倍——跨坐标系断言必然假 FAIL。Win32 窗口/鼠标探针首行必须 `SetProcessDPIAware`（`Add-Type` P/Invoke），断言才在同一坐标系内
- **Windows 杀软误报**：开发期单文件 exe 可能被误报，调试用绿色版形态
- **PowerShell 中文编码**：见"代码约定"同名额
