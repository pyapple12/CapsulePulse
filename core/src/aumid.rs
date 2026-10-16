//! 通知署名（AUMID）注册（PATCH012）：让**绿色单 exe** 的系统通知署名 = 应用名 + 正式图标。
//!
//! 机理（2026-10-16 取证）：`tauri-plugin-notification` 的 Windows 实现把 `tauri.conf.json`
//! 的 `identifier` 当 AUMID 传给底层 `Toast::new(app_id)`；Windows 解析 toast 来源需要该 AUMID
//! 的**注册信息**（显示名 + 图标），未注册则回退显示 "Windows PowerShell"（crate 注释原文
//! "the toast will erroneously report its origin as powershell"）。未打包应用的正解（crate 自带
//! 示例 `unpackaged_app.rs` 同款）= 在 **HKCU**（当前用户，免管理员）下建
//! `SOFTWARE\Classes\AppUserModelId\<AUMID>` 并写 `DisplayName` / `IconBackgroundColor` / `IconUri`。
//! 本模块把它**产品化**为"首启自注册"（而非需要用户手工配置的机器级 hack）。
//!
//! 第一性原理：Windows 的 toast 来源解析**必须**有 AUMID 注册（开始菜单快捷方式或注册表项）
//! ⇒ "纯绿色 + 零注册表"与"正确署名"不可兼得；本项目选择写入**一条 HKCU 项**（免管理员、可清理）。
//!
//! 设计：手写 advapi32 FFI（零新依赖，沿 `glass_backdrop.rs` 的 dwmapi 先例）；图标经
//! `include_bytes!` 编译期嵌入、首启解出到 `data/aumid-icon.png`（自包含，不依赖 zip 内附文件）。
//! **幂等**：每次启动覆盖写（`IconUri` 随 exe 路径变；用户删项后亦自愈）。失败仅日志——署名回退
//! PowerShell，通知本体不受影响（容错白名单 PATCH012 登记）。

use std::path::Path;

/// 编译期嵌入的通知图标（正式图标落地后替换 `core/icons/icon.png` 即生效）。
#[cfg(target_os = "windows")]
const ICON_PNG: &[u8] = include_bytes!("../icons/icon.png");

/// 确保通知署名已注册：① 把内嵌图标解出到 `data/aumid-icon.png`（内容不同才写）；
/// ② 写 HKCU 的 `SOFTWARE\Classes\AppUserModelId\<aumid>` 三值（显示名 / 图标底色 / 图标路径）。
///
/// # 参数
/// - `aumid`：应用 AUMID——传 `tauri.conf.json` 的 `identifier`（与通知插件传给
///   `Toast::new(app_id)` 的值**必须一致**，否则署名仍不生效）。
/// - `display_name`：通知来源显示名（传产品名）。
///
/// # 返回
/// 成功 `Ok(())`；失败返回可读原因（调用方仅落日志，不阻断启动）。
pub fn ensure_notification_identity(aumid: &str, display_name: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let icon = crate::paths::default_aumid_icon_path().map_err(|e| e.to_string())?;
        write_icon_if_changed(&icon)?;
        let sub_key = format!("SOFTWARE\\Classes\\AppUserModelId\\{aumid}");
        let key = RegKey::create(&sub_key)?;
        key.set_string("DisplayName", display_name)?;
        key.set_string("IconBackgroundColor", "0")?;
        key.set_string("IconUri", &icon.to_string_lossy())?;
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (aumid, display_name);
        Ok(()) // 非 Windows 无 AUMID 概念，静默 no-op
    }
}

/// 把内嵌图标解出到 `path`（内容一致则跳过，避免每次启动无谓写盘）。
#[cfg(target_os = "windows")]
fn write_icon_if_changed(path: &Path) -> Result<(), String> {
    if let Ok(existing) = std::fs::read(path) {
        if existing == ICON_PNG {
            return Ok(());
        }
    }
    std::fs::write(path, ICON_PNG).map_err(|e| format!("通知图标解出失败：{e}"))
}

// —— 私有 FFI 核（advapi32；零新依赖）——

/// `HKEY_CURRENT_USER` 伪句柄：Win32 定义为 `(HKEY)(LONG)0x80000001` ⇒ 需**符号扩展**到 64 位。
#[cfg(target_os = "windows")]
const HKEY_CURRENT_USER: isize = 0x8000_0001u32 as i32 as isize;

/// `KEY_WRITE` = `KEY_SET_VALUE | KEY_CREATE_SUB_KEY`（写值 + 建子键，权限最小化）。
#[cfg(target_os = "windows")]
const KEY_WRITE: u32 = 0x0002 | 0x0004;

/// `REG_SZ`：以 NUL 结尾的 UTF-16 字符串。
#[cfg(target_os = "windows")]
const REG_SZ: u32 = 1;

#[cfg(target_os = "windows")]
#[link(name = "advapi32")]
extern "system" {
    /// 建/开注册表键（返回 0 = ERROR_SUCCESS）。
    fn RegCreateKeyExW(
        hkey: isize,
        sub_key: *const u16,
        reserved: u32,
        class: *const u16,
        options: u32,
        sam_desired: u32,
        security_attributes: *const core::ffi::c_void,
        result: *mut isize,
        disposition: *mut u32,
    ) -> i32;
    /// 写注册表值（返回 0 = ERROR_SUCCESS）。
    fn RegSetValueExW(
        hkey: isize,
        value_name: *const u16,
        reserved: u32,
        ty: u32,
        data: *const u8,
        cb_data: u32,
    ) -> i32;
    /// 关注册表键。
    fn RegCloseKey(hkey: isize) -> i32;
}

/// 注册表键的小包装：`create` 建/开，`set_string` 写 REG_SZ，`Drop` 自动关（RAII）。
#[cfg(target_os = "windows")]
struct RegKey {
    handle: isize,
}

#[cfg(target_os = "windows")]
impl RegKey {
    /// 建或打开 `sub_key`（HKCU 下），拿到可写句柄。
    fn create(sub_key: &str) -> Result<Self, String> {
        use std::os::windows::ffi::OsStrExt;

        let wide: Vec<u16> = std::ffi::OsStr::new(sub_key)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let mut handle: isize = 0;
        let mut disposition: u32 = 0;
        let rc = unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                wide.as_ptr(),
                0,
                std::ptr::null(),
                0,
                KEY_WRITE,
                std::ptr::null(),
                &mut handle,
                &mut disposition,
            )
        };
        if rc != 0 {
            return Err(format!("注册表键创建失败 rc={rc}（{sub_key}）"));
        }
        Ok(Self { handle })
    }

    /// 写一个 REG_SZ 值（UTF-16 + 结尾 NUL，字节长度 = (len + 1) × 2）。
    fn set_string(&self, name: &str, value: &str) -> Result<(), String> {
        use std::os::windows::ffi::OsStrExt;

        let name_wide: Vec<u16> = std::ffi::OsStr::new(name)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let value_wide: Vec<u16> = std::ffi::OsStr::new(value)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let bytes = unsafe {
            std::slice::from_raw_parts(value_wide.as_ptr() as *const u8, value_wide.len() * 2)
        };
        let rc = unsafe {
            RegSetValueExW(
                self.handle,
                name_wide.as_ptr(),
                0,
                REG_SZ,
                bytes.as_ptr(),
                bytes.len() as u32,
            )
        };
        if rc != 0 {
            return Err(format!("注册表值写入失败 rc={rc}（{name}）"));
        }
        Ok(())
    }
}

#[cfg(target_os = "windows")]
impl Drop for RegKey {
    /// 关句柄（失败无补救手段，忽略返回值）。
    fn drop(&mut self) {
        unsafe {
            RegCloseKey(self.handle);
        }
    }
}
