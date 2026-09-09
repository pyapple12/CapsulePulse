//! 提醒设置：阈值与开关的用户持久化（~/.capsule-pulse/config.json，路径由调用方注入）。
//! 语义：文件不存在 = 首次启动 → 返回默认值（容错白名单登记项 PL003.3）；
//! JSON 损坏 / 字段非法 → 严格报错（AGENTS 错误策略主线）。

use std::path::{Path, PathBuf};

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
}

fn default_threshold() -> u32 {
    50
}

fn default_true() -> bool {
    true
}

impl Default for ReminderSettings {
    fn default() -> Self {
        Self {
            threshold_min: default_threshold(),
            sound_enabled: default_true(),
            notify_enabled: default_true(),
        }
    }
}

impl ReminderSettings {
    /// 校验：阈值须在 1–240 分钟内。
    pub fn validate(&self) -> Result<(), SettingsError> {
        if !(1..=240).contains(&self.threshold_min) {
            return Err(SettingsError::InvalidThreshold(self.threshold_min));
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

/// 默认设置文件路径：`~/.capsule-pulse/config.json`（目录不存在则自建）。
pub fn default_path() -> Result<PathBuf, SettingsError> {
    let Some(home) = dirs::home_dir() else {
        return Err(SettingsError::HomeDirUnavailable);
    };
    let dir = home.join(".capsule-pulse");
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join("config.json"))
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
    /// 无法定位用户主目录。
    #[error("无法定位用户主目录")]
    HomeDirUnavailable,
    /// 提醒阈值越界（合法范围 1–240 分钟）。
    #[error("提醒阈值非法：{0} 分钟（应为 1–240）")]
    InvalidThreshold(u32),
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
    }

    /// save → load 往返一致；非法阈值保存被拒。
    #[test]
    fn save_load_roundtrip_and_validation() {
        let p = temp_path("roundtrip");
        let s = ReminderSettings {
            threshold_min: 30,
            sound_enabled: false,
            notify_enabled: true,
        };
        s.save(&p).unwrap();
        assert_eq!(ReminderSettings::load(&p).unwrap(), s);

        let bad = ReminderSettings {
            threshold_min: 0,
            sound_enabled: true,
            notify_enabled: true,
        };
        assert!(matches!(
            bad.save(&p),
            Err(SettingsError::InvalidThreshold(0))
        ));
        let _ = std::fs::remove_file(&p);
    }

    /// 原子落盘：save 后无残留 .tmp 且往返一致（中断安全由"临时文件 + rename"语义保证）。
    #[test]
    fn atomic_save_roundtrip_without_temp_leftover() {
        let p = temp_path("atomic");
        let s = ReminderSettings {
            threshold_min: 42,
            sound_enabled: true,
            notify_enabled: false,
        };
        s.save(&p).unwrap();
        assert_eq!(ReminderSettings::load(&p).unwrap(), s);
        let leftovers: Vec<String> = std::fs::read_dir(p.parent().unwrap())
            .unwrap()
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "残留临时文件：{leftovers:?}");
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
