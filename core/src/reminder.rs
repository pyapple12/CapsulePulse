//! 提醒评估器：连续工作时长的阈值触发与重发判定（纯逻辑，无 tauri/时钟依赖，注入输入直测）。
//! 触发条件用无减法比较（segment >= last + repeat）规避"新段时长小于旧触发点"的下溢；
//! clear() 于暂停/开始/重开时调用（暂停即重置，用户定案 2026-09-09）。

use std::time::Duration;

/// 提醒配置：阈值与重发间隔（自用户设置换算）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReminderConfig {
    /// 触发阈值（连续工作时长）。
    pub threshold: Duration,
    /// 触发后的重发间隔。
    pub repeat: Duration,
}

impl ReminderConfig {
    /// 自用户设置换算（threshold_min 分钟；重发间隔固定 5 分钟——2026-09-09 用户定案）。
    pub fn from_minutes(threshold_min: u32) -> Self {
        Self {
            threshold: Duration::from_secs(u64::from(threshold_min) * 60),
            repeat: Duration::from_secs(300),
        }
    }
}

/// 段内触发状态：本段上次触发的段时长位置（暂停/开始/重开时经 clear 重置）。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ReminderFire {
    last_fired: Option<Duration>,
}

impl ReminderFire {
    /// 评估：达阈值且（未触发过 或 距上次触发 ≥ 重发间隔）时触发并记录；返回是否应提醒。
    pub fn evaluate(&mut self, segment: Duration, config: &ReminderConfig) -> bool {
        if segment < config.threshold {
            return false;
        }
        let due = match self.last_fired {
            None => true,
            Some(last) => segment >= last + config.repeat,
        };
        if due {
            self.last_fired = Some(segment);
        }
        due
    }

    /// 段内上次触发点（只读视图）。
    pub fn last_fired(&self) -> Option<Duration> {
        self.last_fired
    }

    /// 重置触发状态（暂停/开始/重开 = 新段）。
    pub fn clear(&mut self) {
        self.last_fired = None;
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn config() -> ReminderConfig {
        ReminderConfig::from_minutes(50)
    }

    fn mins(n: u64) -> Duration {
        Duration::from_secs(n * 60)
    }

    /// 未达阈值不触发；恰达阈值触发并记录触发点。
    #[test]
    fn fires_exactly_at_threshold() {
        let mut fire = ReminderFire::default();
        assert!(!fire.evaluate(mins(49), &config()));
        assert!(fire.evaluate(mins(50), &config()));
        assert_eq!(fire.last_fired(), Some(mins(50)));
    }

    /// 重发间隔：触发后 4 分钟不发、满 5 分钟再发。
    #[test]
    fn repeat_respects_five_minute_interval() {
        let mut fire = ReminderFire::default();
        assert!(fire.evaluate(mins(50), &config()));
        assert!(!fire.evaluate(mins(54), &config()));
        assert!(fire.evaluate(mins(55), &config()));
        assert!(!fire.evaluate(mins(59), &config()));
        assert!(fire.evaluate(mins(60), &config()));
    }

    /// 暂停即重置：clear 后新段从零计，再次达阈值再次触发。
    #[test]
    fn clear_resets_for_new_segment() {
        let mut fire = ReminderFire::default();
        assert!(fire.evaluate(mins(50), &config()));
        fire.clear();
        assert_eq!(fire.last_fired(), None);
        assert!(!fire.evaluate(mins(49), &config()));
        assert!(fire.evaluate(mins(50), &config()));
    }

    /// 旧触发点跨段残留不误触发也不下溢：新段时长小于旧触发点时安静，长到阈值+间隔才发。
    #[test]
    fn stale_fire_point_never_underflows() {
        let mut fire = ReminderFire::default();
        assert!(fire.evaluate(mins(50), &config()));
        // 新段开始（未 clear 的最坏情形）：段时长回退，必须安静且无 panic
        assert!(!fire.evaluate(mins(10), &config()));
        assert!(!fire.evaluate(mins(50), &config()));
        assert!(fire.evaluate(mins(55), &config()));
    }
}
