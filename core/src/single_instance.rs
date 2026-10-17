//! 单实例闸门（FIX006.10 定案）：在**进程第一行**判重——第二个 exe「开不了」，且
//! **不碰数据库、不写日志、不建窗、不出托盘**（零副作用）。
//!
//! 为什么不用 `tauri-plugin-single-instance`（实证 2.4.4 `platform_impl/windows.rs:69-96`）：
//! 它的判定跑在**插件 setup**（晚于 `run()` 前段的开库与留痕），且靠 `FindWindowW` 去找首实例
//! **稍后才创建的隐藏消息窗** ⇒ ① 第二实例被拦下前已经 `Storage::open_default()`（建表 DDL）
//! 并往共享 `data/pulse.log` 写过"启动：缓存中存在未闭合班（待对账）"（日志误导；`busy_timeout=0`
//! 下撞车还可能 `exit(1)` 而首实例不被唤前）；② `FindWindowW` 取不到窗时**没有 else 分支**
//! （不退出）⇒ 启动竞态下两个实例**并存**、共享同一 `data/pulse.db`。本模块把闸门前移到第一行，
//! 两个缺陷一并消除；**零新依赖**（手写 kernel32 FFI，沿 `aumid.rs` / `glass_backdrop.rs` 先例）。
//!
//! 命名空间取 **`Global\`**（全机器级判重、跨用户会话）——MSDN「Kernel object namespaces」：
//! `SeCreateGlobalPrivilege` **仅对 file-mapping / symbolic-link 的创建**有要求，命名
//! mutex / event **不需要**该特权 ⇒ 普通用户进程也能建 ✓。默认安全描述符只授创建者 ⇒ 跨用户
//! 会话会因 DACL 打不开；这里显式给 **NULL DACL**（Everyone 全权）——对象只当"启动闸门"、
//! 不承载任何数据，暴露面最小。
//!
//! 唤起：`Global\...-show` 命名事件（**自动复位** = 脉冲语义，早到不丢：保持有信号直到被取走）。
//! 第二实例 `SetEvent` 后即退；首实例在 `setup` 起守候线程 `WaitForSingleObject`，被唤醒即
//! 把主窗唤到前台（等价旧插件回调）。
//!
//! 失败降级：建对象失败（极罕见）⇒ **落日志并放行**——宁可可能双开，不可把用户挡在应用外
//! （与"启动迁移失败仅落档不阻断"同一条纪律）。
//!
//! 平台：本模块当前只实现 Windows（唯一实机平台）。非 Windows 返回 [`Gate::First`]（不判重），
//! 待 Phase 7 三端适配时补等价实现（flock / 命名 socket）。

/// 判重结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Gate {
    /// 本进程是首个实例：已在 `Global\` 命名空间占住闸门（句柄由内核持有至进程结束）。
    First,
    /// 已有实例在跑：已向它发出"把窗口唤到前台"的信号，调用方应**立即退出**。
    Second,
}

/// 第二实例的退出码（0 = 非错误；与旧插件 `exit(0)` 同义）。
pub(crate) const SECOND_INSTANCE_EXIT: i32 = 0;

/// 进程启动第一行调用：占住闸门，或给已有实例发"唤前"信号。
///
/// # 参数
/// - `identifier`：`tauri.conf.json` 的 `identifier`（同一 identifier 的副本视为同一应用）。
///
/// # 返回
/// [`Gate::First`] = 继续启动；[`Gate::Second`] = 调用方应立即以 [`SECOND_INSTANCE_EXIT`] 退出。
/// 建对象失败时返回 [`Gate::First`] 并落一条诊断日志（放行优先）。
pub(crate) fn enforce(identifier: &str) -> Gate {
    #[cfg(target_os = "windows")]
    {
        let name = format!("Global\\CapsulePulse-SingleInstance-{identifier}");
        match acquire_mutex(&name) {
            MutexOutcome::First(handle) => {
                // 句柄存起来**一直持有**（进程退出由内核回收 ⇒ 永不残留脏状态）
                MUTEX_HANDLE.store(handle, std::sync::atomic::Ordering::SeqCst);
                Gate::First
            }
            MutexOutcome::Second => {
                signal_show_window(identifier);
                Gate::Second
            }
            MutexOutcome::Failed(err) => {
                crate::warn_diag(&format!("单实例闸门：{err}；本次放行（可能双开）"));
                Gate::First
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = identifier;
        Gate::First
    }
}

/// 首实例在 `setup` 调用：建"唤前"命名事件 + 起守候线程；每次被第二实例唤醒即调用 `on_signal`。
///
/// # 参数
/// - `identifier`：同 [`enforce`]。
/// - `on_signal`：唤醒动作（生产传"把主窗唤到前台"）——留成回调以保持本模块不依赖 tauri 装配层。
pub(crate) fn spawn_show_listener<F>(identifier: &str, on_signal: F)
where
    F: Fn() + Send + 'static,
{
    #[cfg(target_os = "windows")]
    {
        let name = format!("Global\\CapsulePulse-ShowWindow-{identifier}");
        let wide_name = wide(&name);
        let attrs = SecurityAttributes::permissive();
        // 自动复位（manual_reset = 0）+ 初始未置位（initial_state = 0）
        let handle = unsafe { CreateEventW(attrs.ptr(), 0, 0, wide_name.as_ptr()) };
        if handle == 0 {
            let err = unsafe { GetLastError() };
            crate::warn_diag(&format!(
                "单实例唤起：CreateEventW 失败（GetLastError={err}）；第二实例将无法唤起主窗"
            ));
            return;
        }
        // "只记一次"闩（本线程常驻、长期循环；FIX007.7）
        static WAIT_FAIL_LOGGED: std::sync::atomic::AtomicBool =
            std::sync::atomic::AtomicBool::new(false);
        std::thread::spawn(move || loop {
            let rc = unsafe { WaitForSingleObject(handle, INFINITE) };
            if rc == WAIT_OBJECT_0 {
                on_signal();
            } else {
                // FIX007.7：`INFINITE` 不会超时 ⇒ 非 `WAIT_OBJECT_0` 即**故障**（`WAIT_FAILED`：
                // 句柄失效等）⇒ 只记一次（长期循环里每拍记会刷爆日志），退避后重试
                crate::warn_diag_once(
                    &WAIT_FAIL_LOGGED,
                    &format!("单实例唤起：等待唤前事件异常（rc={rc}）；本次会话内唤起可能不可用"),
                );
                std::thread::sleep(std::time::Duration::from_millis(500));
            }
        });
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (identifier, on_signal);
    }
}

// —— 私有 FFI 核（kernel32；零新依赖）——

/// 首个实例的互斥体句柄（`isize` 即 `HANDLE`）——**保活锚点，只写不读是刻意的**（FIX007.8）：
/// 进程存活期**必须一直持有、禁止 `CloseHandle`**；一旦关闭，闸门即失效（可再开第二个实例），
/// 且症状要等真双击两次才会暴露 ⇒ **不得当作死代码清理**。
#[cfg(target_os = "windows")]
static MUTEX_HANDLE: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);

/// `ERROR_ALREADY_EXISTS`：`CreateMutexW` 撞名时 `GetLastError` 的返回值。
#[cfg(target_os = "windows")]
const ERROR_ALREADY_EXISTS: u32 = 183;

/// `WaitForSingleObject` 成功返回。
#[cfg(target_os = "windows")]
const WAIT_OBJECT_0: u32 = 0;

/// `WaitForSingleObject` 无限等待。
#[cfg(target_os = "windows")]
const INFINITE: u32 = 0xFFFF_FFFF;

/// `EVENT_MODIFY_STATE`：`SetEvent` 所需的最小访问权（跨用户只需此项，不请求全权）。
#[cfg(target_os = "windows")]
const EVENT_MODIFY_STATE: u32 = 0x0002;

/// `SECURITY_DESCRIPTOR_REVISION`。
#[cfg(target_os = "windows")]
const SECURITY_DESCRIPTOR_REVISION: u32 = 1;

/// `SECURITY_ATTRIBUTES`（`#[repr(C)]` 与 Win32 同布局；x64 下 `nLength` = 24）。
#[cfg(target_os = "windows")]
#[repr(C)]
struct SecurityAttributesRaw {
    n_length: u32,
    lp_security_descriptor: *mut core::ffi::c_void,
    inherit_handle: i32,
}

/// `SECURITY_DESCRIPTOR` 的定长缓冲（x64 下 40 字节、需 8 字节对齐 ⇒ 用 5 个 `u64` 承载）。
#[cfg(target_os = "windows")]
#[repr(C)]
struct SecurityDescriptorBuf([u64; 5]);

#[cfg(target_os = "windows")]
impl SecurityDescriptorBuf {
    /// 构造"**NULL DACL**"安全描述符（Everyone 全权）——跨用户会话也能建/开（仅启动闸门用）。
    /// 任一步失败返回 `None`（调用方退化为系统默认 SD）。
    fn permissive() -> Option<Box<Self>> {
        let mut buf = Box::new(Self([0; 5]));
        let ptr = buf.0.as_mut_ptr() as *mut core::ffi::c_void;
        unsafe {
            if InitializeSecurityDescriptor(ptr, SECURITY_DESCRIPTOR_REVISION) == 0 {
                return None;
            }
            // present = TRUE、dacl = NULL（= 全权）、defaulted = FALSE
            if SetSecurityDescriptorDacl(ptr, 1, std::ptr::null(), 0) == 0 {
                return None;
            }
        }
        Some(buf)
    }
}

/// `SECURITY_ATTRIBUTES` 的持有者：`Box` 住安全描述符 ⇒ 堆地址稳定（`raw` 里的指针不会因搬迁失效）。
#[cfg(target_os = "windows")]
struct SecurityAttributes {
    raw: SecurityAttributesRaw,
    _sd: Option<Box<SecurityDescriptorBuf>>,
}

#[cfg(target_os = "windows")]
impl SecurityAttributes {
    /// 构造（NULL DACL；SD 构造失败则退回系统默认 SD）。
    fn permissive() -> Self {
        let sd = SecurityDescriptorBuf::permissive();
        let ptr = match sd.as_deref() {
            Some(buf) => buf.0.as_ptr() as *mut core::ffi::c_void,
            None => std::ptr::null_mut(),
        };
        Self {
            raw: SecurityAttributesRaw {
                n_length: std::mem::size_of::<SecurityAttributesRaw>() as u32,
                lp_security_descriptor: ptr,
                inherit_handle: 0,
            },
            _sd: sd,
        }
    }

    /// 传给 `Create*W` 的指针。
    fn ptr(&self) -> *const SecurityAttributesRaw {
        &self.raw
    }
}

/// 建/开互斥体的三分支结果。
#[cfg(target_os = "windows")]
enum MutexOutcome {
    /// 本进程创建成功（首个实例）——携带句柄。
    First(isize),
    /// 已有实例（撞名）。
    Second,
    /// 调用失败（放行）。
    Failed(String),
}

/// 在 `Global\` 下建互斥体：成功=首个；`ERROR_ALREADY_EXISTS`=已有实例。
#[cfg(target_os = "windows")]
fn acquire_mutex(name: &str) -> MutexOutcome {
    let wide_name = wide(name);
    let attrs = SecurityAttributes::permissive();
    // 注意：`GetLastError` 必须紧跟调用读取（其后不得插入其它 API）
    let handle = unsafe { CreateMutexW(attrs.ptr(), 0, wide_name.as_ptr()) };
    if handle == 0 {
        let err = unsafe { GetLastError() };
        return MutexOutcome::Failed(format!("CreateMutexW 失败（GetLastError={err}）"));
    }
    let err = unsafe { GetLastError() };
    if err == ERROR_ALREADY_EXISTS {
        unsafe {
            CloseHandle(handle);
        }
        return MutexOutcome::Second;
    }
    MutexOutcome::First(handle)
}

/// 给已有实例发"唤前"脉冲。**两种结果都只落档、不阻断退出**（唤起是体验层；白名单登记项）：
/// - `OpenEventW` 返 0 = **预期竞态**（首实例的唤前事件尚未建好，或首实例已退出）⇒ **不记**
///   （记了则每次快速双击都刷一条，纯噪音）；
/// - `SetEvent` 返 0 = 真失败（句柄失效 / 权限）⇒ `warn_diag_once`（FIX007.7：原实现**丢弃了它的
///   返回值** ⇒ 出现"双击图标没反应"时日志零痕迹、无法诊断）。
#[cfg(target_os = "windows")]
fn signal_show_window(identifier: &str) {
    // "只记一次"闩（本函数在每次双击时都会跑；FIX007.7）
    static SET_EVENT_FAIL_LOGGED: std::sync::atomic::AtomicBool =
        std::sync::atomic::AtomicBool::new(false);
    let name = format!("Global\\CapsulePulse-ShowWindow-{identifier}");
    let wide_name = wide(&name);
    let handle = unsafe { OpenEventW(EVENT_MODIFY_STATE, 0, wide_name.as_ptr()) };
    if handle == 0 {
        return; // 预期竞态（见上说明）：静默
    }
    let ok = unsafe { SetEvent(handle) };
    // `GetLastError` 必须紧跟目标调用读取（其后不得插入其它 API）
    let last_error = unsafe { GetLastError() };
    unsafe {
        CloseHandle(handle);
    }
    if ok == 0 {
        crate::warn_diag_once(
            &SET_EVENT_FAIL_LOGGED,
            &format!(
                "单实例唤起：SetEvent 失败（GetLastError={last_error}）；本次未能唤起已有窗口"
            ),
        );
    }
}

/// `&str` → 以 NUL 结尾的 UTF-16 缓冲。
#[cfg(target_os = "windows")]
fn wide(s: &str) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;

    std::ffi::OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

#[cfg(target_os = "windows")]
#[link(name = "kernel32")]
extern "system" {
    /// 建/开命名互斥体（碰撞时返回已有对象句柄并把 `GetLastError` 置 `ERROR_ALREADY_EXISTS`）。
    fn CreateMutexW(
        attrs: *const SecurityAttributesRaw,
        initial_owner: i32,
        name: *const u16,
    ) -> isize;
    /// 建/开命名事件（`manual_reset` = 0 ⇒ 自动复位）。
    fn CreateEventW(
        attrs: *const SecurityAttributesRaw,
        manual_reset: i32,
        initial_state: i32,
        name: *const u16,
    ) -> isize;
    /// 打开已存在的事件（拿最小访问权）。
    fn OpenEventW(desired_access: u32, inherit: i32, name: *const u16) -> isize;
    /// 置事件为有信号。
    fn SetEvent(handle: isize) -> i32;
    /// 等待对象有信号。
    fn WaitForSingleObject(handle: isize, millis: u32) -> u32;
    /// 关句柄。
    fn CloseHandle(handle: isize) -> i32;
    /// 取最近一次错误码（必须紧跟目标调用读取）。
    fn GetLastError() -> u32;
    /// 初始化安全描述符（`revision` = 1）。
    fn InitializeSecurityDescriptor(sd: *mut core::ffi::c_void, revision: u32) -> i32;
    /// 设置 DACL（`dacl` 传 NULL 且 `present` 非 0 ⇒ 全权）。
    fn SetSecurityDescriptorDacl(
        sd: *mut core::ffi::c_void,
        present: i32,
        dacl: *const core::ffi::c_void,
        defaulted: i32,
    ) -> i32;
}
