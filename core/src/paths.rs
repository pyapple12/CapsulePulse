//! 运行时数据落址：configs/（用户参数）与 data/（会话数据库）的目录解析。
//! 双落址定案（2026-09-10 用户拍板）：dev 构建（debug_assertions）编译期定位项目根——
//! CARGO_MANIFEST_DIR（core/）的父目录，源码树内直观可见且 cargo clean 不伤；
//! release/绿色版 = exe 同级目录（便携形态，整个文件夹即应用全量）。
//! 杜绝使用机器用户目录（翻计划书 §2.2/§2.3 的 ~/.capsule-pulse/ 旧址）；
//! 运行时文件不入仓库（.gitignore 覆盖 configs/config.json 与 data/）。

use std::io;
use std::path::PathBuf;

/// 运行时根目录：dev = 项目根；release/绿色版 = exe 所在目录。
fn runtime_root() -> io::Result<PathBuf> {
    #[cfg(debug_assertions)]
    {
        // env! 为编译期常量，仅存在于 dev 二进制（release 分支不含构建机路径）
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    "无法从 CARGO_MANIFEST_DIR 定位项目根",
                )
            })?;
        Ok(root.to_path_buf())
    }
    #[cfg(not(debug_assertions))]
    {
        // 先绑定再取父目录：current_exe() 的临时值不能活过借用（E0716）
        let exe = std::env::current_exe()?;
        let root = exe
            .parent()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "无法定位程序所在目录"))?;
        Ok(root.to_path_buf())
    }
}

/// 子目录文件路径：<root>/<dir>/<file>，目录不存在则自建。
fn ensure_under(dir_name: &str, file: &str) -> io::Result<PathBuf> {
    let dir = runtime_root()?.join(dir_name);
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join(file))
}

/// 用户配置文件路径：configs/config.json（目录不存在则自建）。
pub(crate) fn default_config_path() -> io::Result<PathBuf> {
    ensure_under("configs", "config.json")
}

/// 会话数据库路径：data/pulse.db（目录不存在则自建）。
pub(crate) fn default_db_path() -> io::Result<PathBuf> {
    ensure_under("data", "pulse.db")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// dev 根 = 项目根：含 core/（Cargo 工程目录），cargo clean 波及不到。
    #[test]
    fn dev_runtime_root_is_project_root() {
        let root = runtime_root().unwrap();
        assert!(
            root.join("core").is_dir(),
            "dev 运行时根应为项目根（实际 {root:?}）"
        );
    }

    /// 配置落 configs/config.json、数据库落 data/pulse.db——均在运行时根下，不在用户主目录。
    #[test]
    fn default_paths_use_configs_and_data() {
        let config = default_config_path().unwrap();
        assert_eq!(config.file_name().unwrap(), "config.json");
        assert_eq!(config.parent().unwrap().file_name().unwrap(), "configs");

        let db = default_db_path().unwrap();
        assert_eq!(db.file_name().unwrap(), "pulse.db");
        assert_eq!(db.parent().unwrap().file_name().unwrap(), "data");
    }
}
