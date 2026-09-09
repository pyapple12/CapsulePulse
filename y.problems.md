# 问题备忘录（y.problems.md）

> 文件职责：问题与远期改进的备忘录，**只增不删、编号递增**（解决后不删条目，在任务清单里闭环）。
> 条目格式：

```markdown
{N}. **{标题}**（{YYYY-MM-DD} {状态}）：

当前问题：{现状描述、为什么是问题}

未来方案：

- {方案要点}（可含设计草稿/代码块/工作量估计）
```

- **状态取值**：记录 / 讨论确认 / 定案 / 待实现
- 条目被任务清单引用（`[problems#N]`），任务完成时回改状态
- 适合记录：暂不做但要留案的设计讨论、远期方向、"遇到特定场景才能推进"的验证项

---

## 一、问题记录（自有条目，2026-09-08 起）

1. **macOS/Linux 平台适配延后**（2026-09-08 定案，待触发）：

   当前问题：计划书 §2.4/§4 以 macOS 为主开发/验证平台，但当前开发实机只有 Windows；macOS vibrancy / Linux blur / 三端打包在 Windows 版成熟前无法实测，提前做只会积累未经真机验证的代码与虚假的跨平台结论。

   未来方案：

   - Windows 版功能全量优先（玻璃 Acrylic / 计时 / 存储 / 提醒 / 托盘 / 打包）；代码中平台分支按 `#[cfg(target_os)]` 预留，但只在 Windows 实测
   - 触发条件：Windows 版到达"成熟"（功能闭环 + 打包分发跑通，约对应计划书 Phase 4/5）后立项跨平台适配，按计划书 §2.4 分支逻辑逐端实测
   - 玻璃判定书（PL001.7）只对 Windows Acrylic 出结论，macOS/Linux 不在判定范围；PL001 期间所有玻璃相关改动须注明"仅 Windows 实机验证"（AGENTS.md 素材与环境陷阱同款约束）

2. **dev exe 系统通知署名为 PowerShell**（2026-09-09 记录，随打包自然解决）：

   当前问题：PL003 实测系统通知 toast 的来源显示为"Windows PowerShell"而非 CapsulePulse。根因（源码级实锤）：tauri-winrt-notification 0.7.3 经 `CreateToastNotifierWithId(AUMID)` 发送，未安装的裸 exe 无 AUMID 注册时按 crate 官方设计回退 `POWERSHELL_APP_ID`（注释原文"the toast will erroneously report its origin as powershell"）——Tauri 未打包应用的已知限制，非本项目 bug；不影响提醒功能本体（通知内容/时间正确）。

   未来方案：

   - 正解 = Phase 5 打包安装器（NSIS/MSI）：bundler 自动注册开始菜单快捷方式 + AUMID + 应用图标，toast 即署名 CapsulePulse 并带正式图标
   - 不提前 hack：手工注册快捷方式 + AppUserModelID 属性可让 dev exe 署名正确，但属机器级一次性配置不入库，打包后仍要安装器重新注册（重复劳动，2026-09-09 用户拍板接受 dev 期偏差）
