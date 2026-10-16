//! 全局热键组合串的解析与规范化（PL028.2）：纯逻辑零依赖（禁 import tauri，`cargo test` 直测）。
//! 注册运行时由 `tauri-plugin-global-shortcut` 承担（`commands/reminder::register_action`）——
//! 本模块只管"什么算合法组合 + 统一成什么字符串"，是设置面板录键器之外的第二道钳制
//! （前端已挡一层，落库前再挡一层，单一事实源 = 本函数返回值）。
//! 输出格式对齐插件解析器（`global-hotkey` 的 `HotKey::from_str`）：**修饰键在前、主键恰一个**，
//! 且 Win 键写作 `Super`（插件认 SUPER/CMD，不认 WIN）。

use thiserror::Error;

/// 热键串错误（跨 IPC 经 `CommandError::Hotkey` 承载可读文案，本类型不直接序列化）。
#[derive(Debug, Error, PartialEq, Eq)]
pub enum HotkeyError {
    /// 组合为空（空串或含空段，如 `Ctrl++C`）
    #[error("热键组合为空")]
    Empty,
    /// 缺少修饰键
    #[error("缺少修饰键（Ctrl / Alt / Shift / Win 至少其一）")]
    NoModifier,
    /// 缺少主键
    #[error("缺少主键（字母 / 数字 / F1–F12）")]
    MissingMainKey,
    /// 主键不认识
    #[error("不支持的按键：{0}（仅字母 / 数字 / F1–F12）")]
    UnknownKey(String),
    /// 重复段（同一修饰键两次，或两个主键）
    #[error("重复的按键段：{0}")]
    Duplicate(String),
    /// 段数超上限（PATCH010.4 用户定案：最高 3 键 = 最多 2 修饰键 + 1 主键）
    #[error("按键过多（最多 3 键）")]
    TooManyKeys,
}

/// 解析并规范化组合串：修饰键顺序无关、大小写无关（`ctrl+alt+c` / `Alt+Shift+P` / `Win+Q` 皆可）；
/// 主键 = 单个 A–Z / 0–9 或 F1–F12，且必须恰有一个；**段数 2–3**（PATCH010.4 用户定案：
/// 至少 1 修饰键 + 1 主键，最多 2 修饰键 + 1 主键）。
///
/// # 返回
/// 规范化显示串（修饰键固定序 `Ctrl+Alt+Shift+Super` + 主键）——可直接交插件注册，
/// 也是落库与界面回显的同一份值（单一事实源）。
///
/// # Errors
/// 空串 / 空段 → [`HotkeyError::Empty`]；无修饰键 → [`HotkeyError::NoModifier`]；
/// 无主键 → [`HotkeyError::MissingMainKey`]；主键非法 → [`HotkeyError::UnknownKey`]；
/// 重复段 → [`HotkeyError::Duplicate`]；段数 > 3 → [`HotkeyError::TooManyKeys`]。
pub fn parse(combo: &str) -> Result<String, HotkeyError> {
    let trimmed = combo.trim();
    if trimmed.is_empty() {
        return Err(HotkeyError::Empty);
    }
    // 固定显示序的四路修饰键（Ctrl / Alt / Shift / Super）
    let mut flags = [false; 4];
    let mut main: Option<String> = None;
    for part in trimmed.split('+') {
        let token = part.trim();
        if token.is_empty() {
            return Err(HotkeyError::Empty);
        }
        let slot = match token.to_ascii_uppercase().as_str() {
            "CTRL" | "CONTROL" => Some(0),
            "ALT" | "OPTION" => Some(1),
            "SHIFT" => Some(2),
            "WIN" | "SUPER" | "CMD" | "COMMAND" | "META" => Some(3),
            _ => None,
        };
        match slot {
            Some(index) => {
                if flags[index] {
                    return Err(HotkeyError::Duplicate(token.to_string()));
                }
                flags[index] = true;
            }
            None => {
                if main.is_some() {
                    return Err(HotkeyError::Duplicate(token.to_string()));
                }
                main = Some(normalize_main_key(token)?);
            }
        }
    }
    if !flags.iter().any(|&flag| flag) {
        return Err(HotkeyError::NoModifier);
    }
    let main = main.ok_or(HotkeyError::MissingMainKey)?;
    // PATCH010.4：段数上限 3（= 最多 2 修饰键 + 1 主键）——修饰键已按位去重，此处按位计数
    if flags.iter().filter(|&&flag| flag).count() + 1 > 3 {
        return Err(HotkeyError::TooManyKeys);
    }
    let mut parts: Vec<&str> = ["Ctrl", "Alt", "Shift", "Super"]
        .iter()
        .enumerate()
        .filter(|(index, _)| flags[*index])
        .map(|(_, name)| *name)
        .collect();
    parts.push(&main);
    Ok(parts.join("+"))
}

/// 主键规范化：单个 A–Z / 0–9（统一大写）或 F1–F12（统一大写），其余报未知键。
fn normalize_main_key(token: &str) -> Result<String, HotkeyError> {
    let upper = token.to_ascii_uppercase();
    let bytes = upper.as_bytes();
    if bytes.len() == 1 && (bytes[0].is_ascii_uppercase() || bytes[0].is_ascii_digit()) {
        return Ok(upper);
    }
    if let Some(rest) = upper.strip_prefix('F') {
        if let Ok(n) = rest.parse::<u32>() {
            if (1..=12).contains(&n) {
                // FIX006.12：**用解析值重组**（`F01` / `F001` → `F1`）——下游插件按**精确串**匹配
                // （`global-hotkey` 的 `parse_hotkey` 只有 "F1"…"F12" 分支），原样透传会注册失败
                return Ok(format!("F{n}"));
            }
        }
    }
    Err(HotkeyError::UnknownKey(token.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 标准组合：解析 → 规范化 → 修饰位与主键正确。
    #[test]
    fn parse_standard_combo() {
        assert_eq!(parse("Ctrl+Alt+C").unwrap(), "Ctrl+Alt+C");
    }

    /// 段序与大小写无关：乱序 / 小写 / 带空格皆归一到固定序。
    #[test]
    fn parse_order_and_case_irrelevant() {
        assert_eq!(parse("alt+ctrl+c").unwrap(), "Ctrl+Alt+C");
        assert_eq!(parse(" Shift + ctrl + f12 ").unwrap(), "Ctrl+Shift+F12");
    }

    /// Win 键别名（Win/Super/Cmd/Command/Meta）统一归一到插件认得的 `Super`。
    #[test]
    fn parse_win_aliases_normalize_to_super() {
        for alias in ["Win", "Super", "Cmd", "Command", "Meta"] {
            assert_eq!(parse(&format!("{alias}+Q")).unwrap(), "Super+Q", "{alias}");
        }
    }

    /// 数字与功能键主键合法。
    #[test]
    fn parse_digit_and_function_keys() {
        assert_eq!(parse("Ctrl+7").unwrap(), "Ctrl+7");
        assert_eq!(parse("Alt+F1").unwrap(), "Alt+F1");
    }

    /// FIX006.12：F 键归一——**前导零折叠为规范串**（下游插件按精确串匹配：`global-hotkey`
    /// 只有 "F1"…"F12" 分支，原样透传 `F01` 会注册失败 ⇒ 静默不可用）。
    #[test]
    fn parse_function_key_normalizes_leading_zero() {
        assert_eq!(parse("Alt+F01").unwrap(), "Alt+F1");
        assert_eq!(parse("ctrl+f001").unwrap(), "Ctrl+F1");
        assert_eq!(parse("Alt+F12").unwrap(), "Alt+F12", "两位数不受影响");
    }

    /// 空串与空段 → Empty。
    #[test]
    fn parse_rejects_empty() {
        assert_eq!(parse(""), Err(HotkeyError::Empty));
        assert_eq!(parse("Ctrl++C"), Err(HotkeyError::Empty));
    }

    /// 无修饰键 → NoModifier（全局热键必须带修饰键，防误吞裸键）。
    #[test]
    fn parse_rejects_no_modifier() {
        assert_eq!(parse("C"), Err(HotkeyError::NoModifier));
    }

    /// 缺主键 → MissingMainKey。
    #[test]
    fn parse_rejects_missing_main_key() {
        assert_eq!(parse("Ctrl+Alt"), Err(HotkeyError::MissingMainKey));
    }

    /// 不支持的主键 → UnknownKey（F13 与空格均不在白名单）。
    #[test]
    fn parse_rejects_unknown_key() {
        assert_eq!(
            parse("Ctrl+Space"),
            Err(HotkeyError::UnknownKey("Space".to_string()))
        );
        assert_eq!(
            parse("Ctrl+F13"),
            Err(HotkeyError::UnknownKey("F13".to_string()))
        );
    }

    /// 重复段 → Duplicate（同一修饰键两次 / 两个主键）。
    #[test]
    fn parse_rejects_duplicate_segment() {
        assert_eq!(
            parse("Ctrl+Ctrl+C"),
            Err(HotkeyError::Duplicate("Ctrl".to_string()))
        );
        assert_eq!(
            parse("Ctrl+C+A"),
            Err(HotkeyError::Duplicate("A".to_string()))
        );
    }

    /// 段数上限（PATCH010.4 用户定案）：最高 3 键 = 最多 2 修饰键 + 1 主键；
    /// 2 键（1 修饰 + 1 主键）与 3 键合法，4 键及以上被拒。
    #[test]
    fn parse_rejects_more_than_three_keys() {
        assert_eq!(parse("Ctrl+A").unwrap(), "Ctrl+A");
        assert_eq!(parse("Ctrl+Alt+A").unwrap(), "Ctrl+Alt+A");
        assert_eq!(parse("Ctrl+Alt+Shift+A"), Err(HotkeyError::TooManyKeys));
        assert_eq!(parse("Ctrl+Alt+Shift+Win+A"), Err(HotkeyError::TooManyKeys));
    }

    /// 往返恒等：规范化输出可再解析且不变（落库值稳定，重注册幂等的前提）。
    #[test]
    fn roundtrip_is_identity() {
        for text in ["Ctrl+Alt+C", "Shift+Super+F1", "Ctrl+5", "Alt+Shift+P"] {
            let normalized = parse(text).unwrap();
            assert_eq!(parse(&normalized).unwrap(), normalized, "{text}");
        }
    }
}
