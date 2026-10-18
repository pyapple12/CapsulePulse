//! CapsulePulse 入口：薄壳调用应用库（lib.rs）的装配函数。
//! 所有模块逻辑与 Tauri 装配都在 lib 侧，本文件仅保持二进制入口存在。
//!
//! `windows_subsystem` **仅 release 生效**（照 CT FIX008.4 写法）：release 走 GUI 子系统 ⇒
//! 直启不再弹控制台窗（那串以 exe 路径命名的任务栏项就是它）；**debug / test 保留 console
//! 子系统**——dev 期的 stderr 日志通道（`console()` / `warn_diag` 的控制台半）依赖它可见。
//! release 下诊断主通道 = `data/pulse.log`（`warn_diag` 的文件半）。

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    capsule_pulse::run()
}
