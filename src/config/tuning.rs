//! 系统自适应参数调优
//!
//! 根据实际硬件资源（CPU 核心数、物理内存大小）动态计算最优性能参数，
//! 避免因默认值过大导致内存不足或过小浪费性能。

use std::process::Command;

use num_cpus;

/// 性能调优参数
pub struct TuningParams {
    /// 单次发送行数（INSERT 语句中包含的 VALUES 数量）
    pub insert_rows: usize,
    /// 并发执行的 INSERT 语句数量（同时向数据库发送的批次数）
    pub concurrency: usize,
    /// 每个被引用表缓存在内存中的主键值数量（用于外键随机选择）
    pub fk_pool_cap: usize,
}

/// 获取系统总物理内存（单位：MB）
///
/// 跨平台实现：Linux 读取 `/proc/meminfo`，macOS 使用 `sysctl hw.memsize`，
/// Windows 使用 `wmic`（可选）。若所有方法失败，返回 `None`。
fn get_total_memory_mb() -> Option<usize> {
    #[cfg(target_os = "linux")]
    {
        let content = std::fs::read_to_string("/proc/meminfo").ok()?;
        for line in content.lines() {
            if line.starts_with("MemTotal:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    let kib: u64 = parts[1].parse().ok()?;
                    return Some((kib / 1024) as usize);
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        let output = Command::new("sysctl")
            .args(&["-n", "hw.memsize"])
            .output()
            .ok()?;
        if output.status.success() {
            let bytes = String::from_utf8_lossy(&output.stdout);
            if let Ok(bytes) = bytes.trim().parse::<u64>() {
                return Some((bytes / (1024 * 1024)) as usize);
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        use std::ffi::OsString;
        use std::os::windows::ffi::OsStringExt;
        // 尝试使用 wmic 获取内存（不保证所有 Windows 都可用）
        let output = Command::new("wmic")
            .args(&["ComputerSystem", "get", "TotalPhysicalMemory"])
            .output()
            .ok()?;
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if let Ok(bytes) = line.trim().parse::<u64>() {
                    return Some((bytes / (1024 * 1024)) as usize);
                }
            }
        }
    }

    None
}

/// 自动调优性能参数
///
/// - `pool_max_size`: 数据库连接池最大连接数（可选，用于限制并发度上限）
pub fn auto_tune(pool_max_size: Option<usize>) -> TuningParams {
    let cpu_cores = num_cpus::get();
    let mem_mb = get_total_memory_mb().unwrap_or(2048); // 默认 2GB

    eprintln!(
        "[auto_tune] detected CPU cores: {}, memory: {} MB",
        cpu_cores, mem_mb
    );

    // 根据内存大小分级（单位 MB）
    let (base_insert, base_concurrency, base_fk_cap) = match mem_mb {
        m if m < 512 => (500, 2, 1000), // 超低配置
        m if m < 1024 => (1000, 3, 2000),
        m if m < 2048 => (3000, 4, 6000),
        m if m < 4096 => (6000, 6, 10000),
        m if m < 8192 => (10000, 8, 20000),
        m if m < 16384 => (30000, 10, 60000),
        _ => (50000, 12, 100000), // 16GB 以上
    };

    // 并发度受连接池上限约束
    let pool_max = pool_max_size.unwrap_or(20);
    let concurrency = base_concurrency.min(pool_max).max(2);

    TuningParams {
        insert_rows: base_insert,
        concurrency,
        fk_pool_cap: base_fk_cap,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_detection() {
        let mem = get_total_memory_mb();
        println!("Detected memory: {:?} MB", mem);
        assert!(mem.is_some());
    }

    #[test]
    fn test_auto_tune() {
        let params = auto_tune(Some(20));
        println!(
            "params: insert_rows={}, concurrency={}, fk_pool_cap={}",
            params.insert_rows, params.concurrency, params.fk_pool_cap
        );
        assert!(params.insert_rows > 0);
        assert!(params.concurrency >= 2);
        assert!(params.fk_pool_cap > 0);
    }
}
