//! 极简诊断日志（FIX002.7）：关键失败路径（启动失败/退出落库失败/落库回滚）追加一行到
//! data/pulse.log——release GUI 子系统下 eprintln 无处可落，错误"必须明确记录"的策略主线
//! 由此兜底。设计取舍：写失败静默忽略——日志自身不得引发二次故障（容错白名单第 ⑥ 项，
//! AGENTS 错误策略）；不引日志框架（YAGNI，打包期若加 windows_subsystem 再评估升级）。

use std::io::Write;

use chrono::Local;

/// 追加一行诊断（本地时间前缀）；路径经 crate::paths 解析，任何失败静默忽略（白名单 ⑥）。
pub(crate) fn log(line: &str) {
    let Ok(path) = crate::paths::default_log_path() else {
        return;
    };
    append(
        &path,
        &format!("{} {line}", Local::now().format("%Y-%m-%d %H:%M:%S")),
    );
}

/// 向指定文件追加一行（路径注入可测）；打开/写入失败静默忽略。
fn append(path: &std::path::Path, line: &str) {
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(file, "{line}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 临时文件追加：多行按序落盘（诊断日志的核心语义）。
    #[test]
    fn append_writes_lines_in_order() {
        let dir = std::env::temp_dir().join(format!("capsule-pulse-diag-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("probe.log");
        let _ = std::fs::remove_file(&path);

        append(&path, "第一行");
        append(&path, "第二行");

        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content, "第一行\n第二行\n");

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    /// 不可写路径静默忽略（白名单 ⑥：日志失败不引发二次故障）。
    #[test]
    fn append_failure_is_silent() {
        // 目录不存在且不创建 → open 失败 → 静默
        append(
            std::path::Path::new("Z:/capsule-pulse-不可能存在的目录/probe.log"),
            "不应panic",
        );
    }
}
