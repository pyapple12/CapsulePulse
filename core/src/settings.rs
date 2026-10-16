//! 提醒设置：阈值与开关的用户持久化（configs/config.json，双落址见 crate::paths，路径由调用方注入）。
//! 语义：文件不存在 = 首次启动 → 返回默认值（容错白名单登记项 PL003.3）；
//! JSON 损坏 / 字段非法 → 严格报错（AGENTS 错误策略主线）。

use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// 窗口位置（物理像素；`outer_position` 的原样记录，PATCH001.4）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowPos {
    /// 窗口左上角 x（物理像素）
    pub x: i32,
    /// 窗口左上角 y（物理像素）
    pub y: i32,
}

/// 应用设置（serde 结构单一来源，前端经 get/set 命令读写）。
/// 命名沿历史（原为纯提醒设置）：config.json 是单一设置文件，本结构还承载窗口位置等
/// **Rust 自管字段**——前端不参与，`set_settings` 整结构覆盖时由命令层继承（见 commands/reminder）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReminderSettings {
    /// 连续工作提醒阈值（分钟，1–99；上限与设计设置板输入框 max=99 同源，PL019.4）。
    #[serde(default = "default_threshold")]
    pub threshold_min: u32,
    /// 提示音开关。
    #[serde(default = "default_true")]
    pub sound_enabled: bool,
    /// 系统通知开关。
    #[serde(default = "default_true")]
    pub notify_enabled: bool,
    /// 自动下班时长（小时，4–12）：在岗满 N 小时未手动下班则按"上班 + N"回填下班
    /// （PL005.4；serde default 使旧 config.json 无此字段时自动补 8，首启即开箱可用；
    /// PL025.5 收窄到 4–12，与设置板 stepper 口径统一）。
    #[serde(default = "default_auto_out_hours")]
    pub workday_auto_out_hours: u32,
    /// 窗口置顶（PL028.3，用户定案**缺省关**）：窗口恒在最前；关闭后允许被其他窗口遮挡。
    #[serde(default)]
    pub always_on_top: bool,
    /// 贴边吸附（PL028.3，用户定案**缺省开**）：拖动松手时窗口边缘距屏幕边缘 ≤
    /// [`SNAP_THRESHOLD_PX`] 即自动贴边停驻（落位距边 [`SNAP_GAP_PX`]）。
    #[serde(default = "default_true")]
    pub snap_to_edge: bool,
    /// 窗口显隐切换全局热键（PL028.2；缺省 `Alt+Shift+S` = 原硬编码值）。
    #[serde(default = "default_hotkey_visible")]
    pub hotkey_toggle_visible: String,
    /// 工作/休息切换全局热键（PL028.2；缺省 `Alt+Shift+P` = 原硬编码值）。
    #[serde(default = "default_hotkey_timer")]
    pub hotkey_toggle_timer: String,
    /// 窗口位置记忆（`None` = 从未保存 → 启动走默认右上落位，PATCH001.4）。
    #[serde(default)]
    pub window_pos: Option<WindowPos>,
}

/// 贴边吸附阈值（px）：窗口边缘距屏幕边缘 ≤ 此值即吸附（单一来源；PL028.3 抄 CT PL017）。
pub const SNAP_THRESHOLD_PX: i32 = 50;
/// 贴边落位间距（px）：吸附后窗口边缘距屏幕边缘留出的缝隙。
pub const SNAP_GAP_PX: i32 = 5;

fn default_threshold() -> u32 {
    50
}

fn default_true() -> bool {
    true
}

fn default_auto_out_hours() -> u32 {
    8
}

fn default_hotkey_visible() -> String {
    "Alt+Shift+S".to_string()
}

fn default_hotkey_timer() -> String {
    "Alt+Shift+P".to_string()
}

impl Default for ReminderSettings {
    fn default() -> Self {
        Self {
            threshold_min: default_threshold(),
            sound_enabled: default_true(),
            notify_enabled: default_true(),
            workday_auto_out_hours: default_auto_out_hours(),
            always_on_top: false,
            snap_to_edge: default_true(),
            hotkey_toggle_visible: default_hotkey_visible(),
            hotkey_toggle_timer: default_hotkey_timer(),
            window_pos: None,
        }
    }
}

impl ReminderSettings {
    /// 校验：提醒阈值 1–99 分钟；自动下班小时数取
    /// [`AUTO_OUT_MIN_HOURS`](crate::workday::AUTO_OUT_MIN_HOURS)–[`OVERTIME_CAP_HOURS`](crate::workday::OVERTIME_CAP_HOURS)
    /// （FIX006.16：区间**两端都由 `workday` 常量单一来源给出**，与设置板步进器同源——原为两处裸字面量）。
    pub fn validate(&self) -> Result<(), SettingsError> {
        if !(1..=99).contains(&self.threshold_min) {
            return Err(SettingsError::InvalidThreshold(self.threshold_min));
        }
        if !(crate::workday::AUTO_OUT_MIN_HOURS..=crate::workday::OVERTIME_CAP_HOURS)
            .contains(&self.workday_auto_out_hours)
        {
            return Err(SettingsError::InvalidAutoOutHours(
                self.workday_auto_out_hours,
            ));
        }
        Ok(())
    }

    /// 从 JSON 文件载入；文件不存在 = 首次启动，返回默认值（容错白名单登记项）。
    pub fn load(path: &Path) -> Result<Self, SettingsError> {
        match std::fs::read_to_string(path) {
            Ok(json) => {
                let settings: Self = serde_json::from_str(&json)?;
                settings.validate()?;
                Ok(settings)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(SettingsError::Io(e)),
        }
    }

    /// 保存到 JSON 文件（保存前校验；先写同目录临时文件再 rename 原子落盘——中途失败不损坏既有配置）。
    pub fn save(&self, path: &Path) -> Result<(), SettingsError> {
        self.validate()?;
        let tmp = path.with_extension("tmp");
        let result = std::fs::write(&tmp, serde_json::to_string_pretty(self)?)
            .and_then(|()| std::fs::rename(&tmp, path));
        if result.is_err() {
            // 清理临时文件；清理失败仅残留 .tmp（下次保存覆盖），主错误照常上抛
            if let Err(cleanup) = std::fs::remove_file(&tmp) {
                eprintln!("设置临时文件清理失败：{cleanup}");
                crate::diag::log(&format!("设置临时文件清理失败：{cleanup}"));
            }
        }
        result?;
        Ok(())
    }
}

/// 设置层错误：底层错误透传 + 阈值校验，不静默兜底。
#[derive(Debug, Error)]
pub enum SettingsError {
    /// JSON 序列化/解析错误。
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    /// 文件读写错误。
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// 提醒阈值越界（合法范围 1–99 分钟）。
    #[error("提醒阈值非法：{0} 分钟（应为 1–99）")]
    InvalidThreshold(u32),
    /// 自动下班小时数越界（合法范围 4–12 小时）。
    #[error("自动下班小时数非法：{0}（应为 4–12）")]
    InvalidAutoOutHours(u32),
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    /// 临时目录内的独立设置文件（禁触真实用户数据）。
    fn temp_path(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("capsule-pulse-settings-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(format!("{tag}.json"))
    }

    /// 文件不存在 → 默认值（首启语义，容错登记项）。
    #[test]
    fn load_missing_returns_default() {
        let p = temp_path("missing");
        let _ = std::fs::remove_file(&p);
        let s = ReminderSettings::load(&p).unwrap();
        assert_eq!(s, ReminderSettings::default());
        assert_eq!(s.threshold_min, 50);
        assert!(s.sound_enabled && s.notify_enabled);
        assert_eq!(s.workday_auto_out_hours, 8);
    }

    /// PL005.4 旧配置兼容：config.json 缺 workday_auto_out_hours 字段 → serde default 补 8。
    #[test]
    fn missing_auto_hours_field_defaults_to_eight() {
        let p = temp_path("legacy");
        std::fs::write(
            &p,
            r#"{"threshold_min":30,"sound_enabled":true,"notify_enabled":false}"#,
        )
        .unwrap();
        let s = ReminderSettings::load(&p).unwrap();
        assert_eq!(s.threshold_min, 30, "既有字段照常读入");
        assert_eq!(s.workday_auto_out_hours, 8);
        let _ = std::fs::remove_file(&p);
    }

    /// FIX006.16：区间**两端由 `workday` 常量给出**——边界内放行、跨界拒绝。
    /// 用例随常量走（改 12/4 时本用例自动覆盖新边界，不再依赖裸字面量）。
    #[test]
    fn auto_hours_boundaries_follow_workday_constants() {
        let min = crate::workday::AUTO_OUT_MIN_HOURS;
        let max = crate::workday::OVERTIME_CAP_HOURS;
        for ok in [min, max] {
            let s = ReminderSettings {
                workday_auto_out_hours: ok,
                ..ReminderSettings::default()
            };
            assert!(s.validate().is_ok(), "{ok} 小时应放行（区间端点）");
        }
        for bad in [min - 1, max + 1] {
            let s = ReminderSettings {
                workday_auto_out_hours: bad,
                ..ReminderSettings::default()
            };
            assert!(s.validate().is_err(), "{bad} 小时应拒绝（越界）");
        }
    }

    /// 自动下班小时数越界（0 / 73）保存与载入均拒绝（合法 **4–12**）。
    #[test]
    fn auto_hours_out_of_range_rejected() {
        let p = temp_path("auto-range");
        let bad = ReminderSettings {
            threshold_min: 50,
            sound_enabled: true,
            notify_enabled: true,
            workday_auto_out_hours: 0,
            always_on_top: false,
            snap_to_edge: true,
            hotkey_toggle_visible: "Alt+Shift+S".to_string(),
            hotkey_toggle_timer: "Alt+Shift+P".to_string(),
            window_pos: None,
        };
        assert!(matches!(
            bad.save(&p),
            Err(SettingsError::InvalidAutoOutHours(0))
        ));
        let bad = ReminderSettings {
            threshold_min: 50,
            sound_enabled: true,
            notify_enabled: true,
            workday_auto_out_hours: 73,
            always_on_top: false,
            snap_to_edge: true,
            hotkey_toggle_visible: "Alt+Shift+S".to_string(),
            hotkey_toggle_timer: "Alt+Shift+P".to_string(),
            window_pos: None,
        };
        assert!(matches!(
            bad.save(&p),
            Err(SettingsError::InvalidAutoOutHours(73))
        ));
        std::fs::write(
            &p,
            r#"{"threshold_min":50,"sound_enabled":true,"notify_enabled":true,"workday_auto_out_hours":0}"#,
        )
        .unwrap();
        assert!(ReminderSettings::load(&p).is_err());
        let _ = std::fs::remove_file(&p);
    }

    /// save → load 往返一致；非法阈值保存被拒。
    #[test]
    fn save_load_roundtrip_and_validation() {
        let p = temp_path("roundtrip");
        let s = ReminderSettings {
            threshold_min: 30,
            sound_enabled: false,
            notify_enabled: true,
            workday_auto_out_hours: 9,
            always_on_top: false,
            snap_to_edge: true,
            hotkey_toggle_visible: "Alt+Shift+S".to_string(),
            hotkey_toggle_timer: "Alt+Shift+P".to_string(),
            window_pos: None,
        };
        s.save(&p).unwrap();
        assert_eq!(ReminderSettings::load(&p).unwrap(), s);

        let bad = ReminderSettings {
            threshold_min: 0,
            sound_enabled: true,
            notify_enabled: true,
            workday_auto_out_hours: 8,
            always_on_top: false,
            snap_to_edge: true,
            hotkey_toggle_visible: "Alt+Shift+S".to_string(),
            hotkey_toggle_timer: "Alt+Shift+P".to_string(),
            window_pos: None,
        };
        assert!(matches!(
            bad.save(&p),
            Err(SettingsError::InvalidThreshold(0))
        ));
        let _ = std::fs::remove_file(&p);
    }

    /// 原子落盘：save 后自身无残留 .tmp 且往返一致（中断安全由"临时文件 + rename"语义保证）。
    /// 只断言本用例自己的 tmp——测试并行共享同一临时目录，扫描全目录会撞见邻用例的瞬时 tmp。
    #[test]
    fn atomic_save_roundtrip_without_temp_leftover() {
        let p = temp_path("atomic");
        let s = ReminderSettings {
            threshold_min: 42,
            sound_enabled: true,
            notify_enabled: false,
            workday_auto_out_hours: 6,
            always_on_top: false,
            snap_to_edge: true,
            hotkey_toggle_visible: "Alt+Shift+S".to_string(),
            hotkey_toggle_timer: "Alt+Shift+P".to_string(),
            window_pos: None,
        };
        s.save(&p).unwrap();
        assert_eq!(ReminderSettings::load(&p).unwrap(), s);
        let leftover = p.with_extension("tmp");
        assert!(!leftover.exists(), "残留临时文件：{}", leftover.display());
        let _ = std::fs::remove_file(&p);
    }

    /// PATCH001.4：窗口位置 save→load 往返一致（从无到有）。
    #[test]
    fn window_pos_roundtrip() {
        let p = temp_path("window-pos");
        let s = ReminderSettings {
            window_pos: Some(WindowPos { x: 320, y: 80 }),
            ..ReminderSettings::default()
        };
        s.save(&p).unwrap();
        let loaded = ReminderSettings::load(&p).unwrap();
        assert_eq!(loaded.window_pos, Some(WindowPos { x: 320, y: 80 }));
        let _ = std::fs::remove_file(&p);
    }

    /// PATCH001.4：旧 config.json 缺 window_pos 字段 → None（启动走默认落位），其余字段照常读入。
    #[test]
    fn missing_window_pos_field_defaults_to_none() {
        let p = temp_path("window-pos-legacy");
        std::fs::write(
            &p,
            r#"{"threshold_min":30,"sound_enabled":true,"notify_enabled":true,"workday_auto_out_hours":8}"#,
        )
        .unwrap();
        let s = ReminderSettings::load(&p).unwrap();
        assert_eq!(s.threshold_min, 30);
        assert_eq!(s.window_pos, None);
        let _ = std::fs::remove_file(&p);
    }

    /// PL025.5：自动下班小时口径收窄到 4–12（与设置板 stepper 一致）——3/13 拒，4/12 收。
    #[test]
    fn auto_hours_range_is_four_to_twelve() {
        let p = temp_path("auto-range-narrow");
        for bad in [3, 13] {
            let s = ReminderSettings {
                workday_auto_out_hours: bad,
                ..ReminderSettings::default()
            };
            assert!(
                matches!(s.save(&p), Err(SettingsError::InvalidAutoOutHours(_))),
                "{bad} 应被拒（合法 4–12）"
            );
        }
        for ok in [4, 12] {
            let s = ReminderSettings {
                workday_auto_out_hours: ok,
                ..ReminderSettings::default()
            };
            s.save(&p).unwrap();
            assert_eq!(
                ReminderSettings::load(&p).unwrap().workday_auto_out_hours,
                ok
            );
        }
        let _ = std::fs::remove_file(&p);
    }

    /// 损坏 JSON 严格报错（不静默回退默认——与 NotFound 语义相区分）。
    #[test]
    fn corrupted_json_is_strict_error() {
        let p = temp_path("corrupted");
        std::fs::write(&p, "{ not valid json").unwrap();
        assert!(ReminderSettings::load(&p).is_err());
        let _ = std::fs::remove_file(&p);
    }

    /// 文件内阈值越界（0 / 100）载入即拒绝（合法 1–99，PL019.4 与设计设置板 1~99 口径对齐）。
    #[test]
    fn out_of_range_threshold_rejected_on_load() {
        let p = temp_path("range");
        std::fs::write(
            &p,
            r#"{"threshold_min":0,"sound_enabled":true,"notify_enabled":true}"#,
        )
        .unwrap();
        assert!(ReminderSettings::load(&p).is_err());
        std::fs::write(
            &p,
            r#"{"threshold_min":100,"sound_enabled":true,"notify_enabled":true}"#,
        )
        .unwrap();
        assert!(ReminderSettings::load(&p).is_err());
        let _ = std::fs::remove_file(&p);
    }
}
