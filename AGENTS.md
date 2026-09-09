# CapsulePulse — Agent Guide

玻璃质感的工作计时看板：一个大的开始/暂停按钮记录工作时间，连续工作超阈值时声音 + 系统通知提醒休息，三端（Windows / macOS / Linux）通用。总体规划见 `CapsulePulse_plan.md`。

**当前状态**：PL001（玻璃壳与计时闭环，V0.1.0.1）、PL002（存储与统计聚合，V0.1.0.2）、PL003（提醒调度与设置持久化）均已完结——玻璃 UI + 状态机 + SQLite 落库 + 统计行 + 提醒三通道（通知/提示音/文案，降级互不依赖）+ ⚙ 设置持久化，35 项测试全绿；macOS/Linux 延后 [problems#1]，通知署名随打包解决 [problems#2]。下一个大件：托盘常驻/打包分发（PL004 候选），未立项。方案见 `z.plan.md` 附录，任务档案见 `x.progress.md`。`.agents/skills/` 存放项目自建 skill（audit-project / audit-report / progress-task）。遗留与远期项登记 `y.problems.md`。

## 技术栈

| 组件     | 选型                                                             |
| -------- | ---------------------------------------------------------------- |
| 框架     | Tauri 2（Rust 后端 + 系统 WebView）                              |
| 前端 UI  | Vue 3 + TypeScript + Vite（只做展示，业务零含量）                |
| 核心逻辑 | 纯 Rust（状态机/统计/持久化/提醒调度，cargo test 直测）          |
| 存储     | rusqlite（SQLite，`~/.capsule-pulse/pulse.db`）                  |
| 玻璃效果 | window-vibrancy（macOS vibrancy / Windows acrylic / Linux blur） |
| 通知提醒 | tauri-plugin-notification + 前端 `<audio>` 提示音                |
| 常驻     | Tauri 内置 tray API + 全局快捷键                                 |

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
- **会话落库时机 = 暂停时**（计划书 §2.2 定案）：崩溃最多丢当前段；单表 sessions(started_at, seconds)，v1 只做今日/本周/累计 SUM 聚合
- **提醒调度**（计划书 §2.3）：阈值可配置（默认 50 分钟，设置界面修改、持久化）；tauri_plugin_notification 系统通知 + 前端 `<audio>` 提示音；通知与声音降级互不依赖
- **玻璃效果三端**（计划书 §2.4）：macOS `apply_vibrancy(HudWindow)` / Windows `apply_acrylic`（当前实机验证平台）/ Linux `apply_blur` + 半透明妥协；编译期 `#[cfg(target_os)]` 分支互不影响；前端玻璃卡片用 CSS `backdrop-filter` 叠加系统级模糊；macOS/Linux 适配延后至 Windows 版成熟后 [problems#1]
- **常驻形态**：托盘/菜单栏常驻、关闭最小化到托盘、全局快捷键唤起、后台持续计时
- **运行时数据**：`~/.capsule-pulse/pulse.db`（用户目录，不入仓库）；备份 = 直接拷 db（计划书 Phase 4）

## 目录规划

（2026-09-08 目录风格改造后实态；根目录分类学 = ui / core / configs / modules，modules 待规模需要再建）

```
core/             # Tauri 2 后端（原框架默认名 src-tauri，整体改名；框架文件须与 Cargo.toml 同住）
  Cargo.toml      # 版本单一来源（version 三段式 X.Y.Z）
  tauri.conf.json # version 字段省略（回落 Cargo.toml）
  src/
    lib.rs        # 应用装配：玻璃挂载 + 模块注册
    main.rs       # 薄入口（调 capsule_pulse::run()）
    session.rs    # 计时状态机（纯逻辑，可单测；业务纯逻辑平铺于此，禁 import tauri）
    commands.rs   # Tauri 命令层（PL001 阶段 D）
    storage.rs    # SQLite Repository（PL002）
    reminder.rs   # 提醒调度（PL003）
ui/               # Vue 前端（展示层）
  App.vue
  components/
    TimerCard.vue # 大计时器 + 开始/暂停按钮
    StatsCard.vue # 今日/本周统计
configs/          # 程序读的固定参数与用户参数（预建占位，首个实体文件出现于设置持久化落地时）
assets/           # 提示音、图标
.agents/skills/   # 项目自建 skill（audit-project / audit-report / progress-task）
.temp/            # 临时脚本与文件（gitignore）；探针、验证记录放这里
y.problems.md     # 问题与远期改进备忘录（只增不删、编号递增；任务清单以 [problems#N] 引用）
```

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

## Commit 提交规范

- **标题行**：`<type>: V<版本>，<摘要>`——版本为四段式 `VX.Y.Z.R`（如 `V0.1.0.1`）：前三段与 `core/Cargo.toml` 的 `version` 一致，第四段 R 为修订号（不入配置文件，见下条）；摘要一句话概括核心
- **版本递增**：`feat`/`fix`/`refactor`/`perf` 提交前先 bump 第四段 R（R+1）；前三段在 `Cargo.toml` 中推进时 R 回到 1；`docs`/`test`/`style`/`chore` 不强制
- **type 全集**（conventional 风格）：`feat` 新功能 / `fix` 修复 / `refactor` 重构（行为不变）/ `perf` 性能 / `docs` 文档 / `test` 测试 / `style` 格式 / `build` 构建依赖 / `ci` CI / `chore` 杂项 / `revert` 回滚
- **正文**（默认必写，仅 docs/style 微调可省）：`- ` 列表逐条"具体做了什么"，每条一个功能块、一行自然中文、≤ 100 字符；体例＝`主题：内容（细节；细节）`，同主题多细节用 `；` 分隔。样例：`- 计时状态机：Idle/Running/Paused 三态 + start/pause/resume/total 四操作（暂停继承累计、时间源可注入）`
- **禁止项**：内部编号（PL001.1/FIX001.2 等任务编号）、验证/回归数字（如"全量回归 506 项通过"）、英文混排描述
- **提交范围**：一个版本的所有连带改动一次提交（源码 + 配置 + 文档同步）
- **GitHub 身份与远程**：远程 `git@takechance:pyapple12/CapsulePulse.git`（SSH 别名 `takechance` = pyapple12 账号，密钥 id_b_takechance，经 socks5 127.0.0.1:10808）；仓库级身份 `git config user.name pyapple12` + `user.email takechance_bao@188.com`（覆盖全局 YiYi 身份，与 CapsuleRetro 同款做法）
- **流程**：每次提交前 AI 必须 `git status` + `git diff`（含 `--stat`）核对改动范围无误，再草拟完整 commit 内容（add 清单 + 标题 + 正文）交用户确认；用户审阅后自行执行 `git add`/`git commit`/`git push`（AI 不执行任何 git 写操作）

## 错误策略

主线：**严格抛错**——配置缺失/非法输入直接返回错误（Rust 返回 `Result` 并定义明确错误类型；TS 抛出具体 Error），不静默兜底、不降级假成功。

容错白名单（初始为空；仅产品明确允许的边界可容错，**新增容错须先在此登记**，写明场景、降级行为与理由三要素）：

- **提醒设置文件不存在回退默认**（PL003.3 登记，2026-09-09）：场景——首次启动或用户删除 config.json，载入时 NotFound；降级行为——返回默认设置（阈值 50 分钟/声音开/通知开）不报错；理由——首启无设置文件是正常态而非错误，回退默认即"开箱即用"；JSON 损坏等其余错误仍严格报错不在此列。
- **系统通知发送失败降级仅声音**（PL003.8 登记，2026-09-09）：场景——tauri-plugin-notification 发送失败（部分 Linux DE 无通知服务/权限拒绝等）；降级行为——错误落日志，reminder-due 事件照发（前端提示音与文案不受影响）；理由——计划书 §2.3 定案通知与声音降级互不依赖，通知失败不应吞掉提醒功能本体。
- **提示音播放失败降级仅通知**（PL003.8 登记，2026-09-09）：场景——前端 `<audio>` play 被浏览器策略拒绝或音频资源缺失；降级行为——console.warn 记录，系统通知与提醒文案不受影响；理由——同上，双通道互不依赖，任一失败不吞提醒。

## 素材与环境陷阱

- **运行时数据不入仓库**：`~/.capsule-pulse/`（pulse.db 等）是用户数据；测试用临时 db 一律落 `.temp/` 或系统临时目录，禁止对真实用户数据做测试写入
- **玻璃效果平台差异**：macOS 真 vibrancy / Windows Acrylic / Linux 依赖 compositor（计划书 §2.4/§8 风险项）；当前仅 Windows 实机可验——改玻璃相关代码须注明"Windows 实机验证 + macOS/Linux 按分支逻辑推演"，三端实测按计划 Phase 4 排期，不提前宣布跨平台可用
- **提醒通知三端差异**（计划书 §2.3/§8）：通知与声音可能分别不可用，降级互不依赖；通知/声音相关改动要单独验证降级路径，不能只测正常路径
- **深浅色主题**：跟随系统（计划书 §4）；硬编码纯色会破坏主题感知，玻璃卡片用半透明 + 系统模糊适配两种主题
- **Rust 学习曲线**（计划书 §8）：状态机/存储先写纯逻辑 + 单测、小步迭代；时间相关逻辑（跨零点、跨周）用注入时间源测试，禁止 sleep 等真实等待；Mutex/生命周期报错及时求助不硬扛
- **Tauri 2 ACL 静默拒**：前端能力调用（事件 listen、`data-tauri-drag-region` 拖动、invoke 等）走 ACL 白名单，未授权时**静默失效、控制台零报错**（系列两例：listen 需 `core:event`、拖动需 `core:window:allow-start-dragging`，`core:default` 均不含）；权限事实源 = 构建产物 `core/gen/schemas/acl-manifests.json`；新增前端能力调用后必须 live 验证功能生效，构建绿不代表功能可用
- **运行中 exe 锁文件**：调试运行 capsule-pulse.exe 期间 cargo build 无法覆盖产物（os error 5）——重建前先停运行实例（2026-09-08 实测）
- **Windows 杀软误报**：开发期单文件 exe 可能被误报，调试用绿色版形态
- **PowerShell 中文编码**：见"代码约定"同名额
