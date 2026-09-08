//! Tauri 命令层：前端 invoke 的唯一入口，只做转发——状态机逻辑全在 session.rs，本层零业务判断。
//! 命令核心逻辑抽为接收 `&SessionHandle` 的自由函数（脱离 tauri::State），U2 可在
//! cargo test 下无窗口直测；`#[tauri::command]` 包装仅做 State 解包。
//! 注意：自定义命令不经 ACL 白名单（系列实证），但事件/窗口等能力仍需 capabilities 授权。

use std::sync::{Mutex, MutexGuard};

use serde::Serialize;
use tauri::State;

use crate::session::{SessionError, SessionState, WorkSession};

/// 应用级共享会话：lib.rs 经 `.manage()` 注册一次，五命令经 State 访问同一实例。
pub struct SessionHandle(pub Mutex<WorkSession>);

/// 命令层错误：跨 IPC 序列化为字符串（前端在 promise reject 的 message 中读到文案）。
#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    /// 状态机拒绝（非法状态操作），文案沿用 session.rs。
    #[error(transparent)]
    Session(#[from] SessionError),
    /// 会话锁中毒（此前持锁线程 panic）——严格报错，不静默续行。
    #[error("会话锁已中毒（此前持锁线程异常终止）")]
    Poisoned,
}

impl Serialize for CommandError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

/// status 命令返回体：前端展示所需会话快照（serde 结构单一来源，TS 侧镜像）。
#[derive(Debug, Serialize)]
pub struct SessionStatus {
    /// 状态标识：idle / running / paused。
    pub state: &'static str,
    /// 累计工作毫秒数（Running 态为现算值；十分秒位显示的取数源，2026-09-08 用户定案）。
    pub total_ms: u64,
}

/// 取会话锁：中毒严格报错（Poisoned），不 unwrap 不吞错。
fn lock(handle: &SessionHandle) -> Result<MutexGuard<'_, WorkSession>, CommandError> {
    handle.0.lock().map_err(|_| CommandError::Poisoned)
}

/// 开始新会话：仅 Idle 合法。
fn start_session(handle: &SessionHandle) -> Result<(), CommandError> {
    lock(handle)?.start()?;
    Ok(())
}

/// 暂停：仅 Running 合法，本段并入累计。
fn pause_session(handle: &SessionHandle) -> Result<(), CommandError> {
    lock(handle)?.pause()?;
    Ok(())
}

/// 继续：仅 Paused 合法，累计被继承。
fn resume_session(handle: &SessionHandle) -> Result<(), CommandError> {
    lock(handle)?.resume()?;
    Ok(())
}

/// 重开：reset + start 同锁原子完成——U1"暂停态重开归零"的承载命令。
fn restart_session(handle: &SessionHandle) -> Result<(), CommandError> {
    let mut session = lock(handle)?;
    session.reset();
    session.start()?;
    Ok(())
}

/// 三态 → 前端状态标识（serde 泄露面最小化：内部计时字段不出 IPC）。
fn state_key(state: &SessionState) -> &'static str {
    match state {
        SessionState::Idle => "idle",
        SessionState::Running { .. } => "running",
        SessionState::Paused { .. } => "paused",
    }
}

/// 会话快照：当前状态 + 累计毫秒。
fn status_snapshot(handle: &SessionHandle) -> Result<SessionStatus, CommandError> {
    let session = lock(handle)?;
    Ok(SessionStatus {
        state: state_key(session.state()),
        // u128→u64：毫秒计时溢出 u64 需 5.8 亿年，实际不可达
        total_ms: session.total().as_millis() as u64,
    })
}

/// 开始新会话（仅 Idle 合法）。
#[tauri::command]
pub fn session_start(handle: State<'_, SessionHandle>) -> Result<(), CommandError> {
    start_session(&handle)
}

/// 暂停进行中的会话。
#[tauri::command]
pub fn session_pause(handle: State<'_, SessionHandle>) -> Result<(), CommandError> {
    pause_session(&handle)
}

/// 继续暂停的会话（累计继承）。
#[tauri::command]
pub fn session_resume(handle: State<'_, SessionHandle>) -> Result<(), CommandError> {
    resume_session(&handle)
}

/// 重开：归零并立即开始（任意态合法）。
#[tauri::command]
pub fn session_restart(handle: State<'_, SessionHandle>) -> Result<(), CommandError> {
    restart_session(&handle)
}

/// 查询会话快照（前端 1s tick 拉取）。
#[tauri::command]
pub fn session_status(handle: State<'_, SessionHandle>) -> Result<SessionStatus, CommandError> {
    status_snapshot(&handle)
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;
    use std::time::Duration;

    use super::*;

    /// 空闲句柄。
    fn idle_handle() -> SessionHandle {
        SessionHandle(Mutex::new(WorkSession::default()))
    }

    /// U2：三态 → 前端状态标识映射正确。
    #[test]
    fn state_key_maps_three_states() {
        assert_eq!(state_key(&SessionState::Idle), "idle");
        assert_eq!(
            state_key(&SessionState::Running {
                start: Duration::ZERO
            }),
            "running"
        );
        assert_eq!(
            state_key(&SessionState::Paused {
                elapsed: Duration::ZERO
            }),
            "paused"
        );
    }

    /// U2：start → status 报 running；pause → status 报 paused 且立即暂停秒数为 0。
    #[test]
    fn start_pause_status_forwarding() {
        let handle = idle_handle();
        start_session(&handle).unwrap();
        let s = status_snapshot(&handle).unwrap();
        assert_eq!(s.state, "running");

        pause_session(&handle).unwrap();
        let s = status_snapshot(&handle).unwrap();
        assert_eq!(s.state, "paused");
        assert_eq!(s.total_ms, 0);
    }

    /// U2：非法转发严格报错——Idle 下 pause/resume、Running 下重复 start。
    #[test]
    fn invalid_forwarding_errors() {
        let handle = idle_handle();
        assert!(matches!(
            pause_session(&handle),
            Err(CommandError::Session(SessionError::NotRunning))
        ));
        assert!(matches!(
            resume_session(&handle),
            Err(CommandError::Session(SessionError::NotPaused))
        ));
        start_session(&handle).unwrap();
        assert!(matches!(
            start_session(&handle),
            Err(CommandError::Session(SessionError::NotIdle))
        ));
    }

    /// U2：restart 单锁原子重开——Paused 下归零并立即 running；Idle 下同样可用。
    #[test]
    fn restart_resets_and_starts_atomically() {
        let handle = idle_handle();
        start_session(&handle).unwrap();
        pause_session(&handle).unwrap();
        restart_session(&handle).unwrap();
        let s = status_snapshot(&handle).unwrap();
        assert_eq!(s.state, "running");
        assert_eq!(s.total_ms, 0);

        let handle = idle_handle();
        restart_session(&handle).unwrap();
        assert_eq!(status_snapshot(&handle).unwrap().state, "running");
    }
}
