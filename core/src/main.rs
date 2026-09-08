//! CapsulePulse 入口：薄壳调用应用库（lib.rs）的装配函数。
//! 所有模块逻辑与 Tauri 装配都在 lib 侧，本文件仅保持二进制入口存在。

fn main() {
    capsule_pulse::run()
}
