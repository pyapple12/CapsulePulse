//! 提醒设置：阈值与开关的用户持久化（configs/config.json，双落址见 crate::paths，路径由调用方注入）。
//! 语义：文件不存在 = 首次启动 → 返回默认值（容错白名单登记项 PL003.3）；
//! JSON 损坏 / 字段非法 → 严格报错（AGENTS 错误策略主线）。

use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// 提醒设置（serde 结构单一来源，前端经 get/set 命令读写）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReminderSettings {
    /// 连续工作提醒阈值（分钟，1–240）。
    #[serde(default = "default_threshold")]
    pub threshold_min: u32,
    /// 提示音开关。
    #[serde(default = "default_true")]
    pub sound_enabled: bool,
    /// 系统通知开关。
    #[serde(default = "default_true")]
    pub notify_enabled: bool,
    /// 自动下班时长（小时，1–72）：在岗满 N 小时未手动下班则按"上班 + N"回填下班
    /// （PL005.4；serde default 使旧 config.json 无此字段时自动补 8，首启即开箱可用）。
    #[serde(default = "default_auto_out_hours")]
    pub workday_auto_out_hours: u32,
}

fn default_threshold() -> u32 {
    50
}

fn default_true() -> bool {
    true
}

fn default_auto_out_hours() -> u32 {
    8
}

impl Default for ReminderSettings {
    fn default() -> Self {
        Self {
            threshold_min: default_threshold(),
            sound_enabled: default_true(),
            notify_enabled: default_true(),
            workday_auto_out_hours: default_auto_out_hours(),
        }
    }
}

impl ReminderSettings {
    /// 校验：提醒阈值 1–240 分钟；自动下班 1–72 小时。
    pub fn validate(&self) -> Result<(), SettingsError> {
        if !(1..=240).contains(&self.threshold_min) {
            return Err(SettingsError::InvalidThreshold(self.threshold_min));
        }
        if !(1..=72).contains(&self.workday_auto_out_hours) {
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
    /// 提醒阈值越界（合法范围 1–240 分钟）。
    #[error("提醒阈值非法：{0} 分钟（应为 1–240）")]
    InvalidThreshold(u32),
    /// 自动下班小时数越界（合法范围 1–72 小时）。
    #[error("自动下班小时数非法：{0}（应为 1–72）")]
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

    /// 自动下班小时数越界（0 / 73）保存与载入均拒绝（合法 1–72）。
    #[test]
    fn auto_hours_out_of_range_rejected() {
        let p = temp_path("auto-range");
        let bad = ReminderSettings {
            threshold_min: 50,
            sound_enabled: true,
            notify_enabled: true,
            workday_auto_out_hours: 0,
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
        };
        s.save(&p).unwrap();
        assert_eq!(ReminderSettings::load(&p).unwrap(), s);

        let bad = ReminderSettings {
            threshold_min: 0,
            sound_enabled: true,
            notify_enabled: true,
            workday_auto_out_hours: 8,
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
        };
        s.save(&p).unwrap();
        assert_eq!(ReminderSettings::load(&p).unwrap(), s);
        let leftover = p.with_extension("tmp");
        assert!(!leftover.exists(), "残留临时文件：{}", leftover.display());
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

    /// 文件内阈值越界（0 / 241）载入即拒绝。
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
            r#"{"threshold_min":241,"sound_enabled":true,"notify_enabled":true}"#,
        )
        .unwrap();
        assert!(ReminderSettings::load(&p).is_err());
        let _ = std::fs::remove_file(&p);
    }
}
