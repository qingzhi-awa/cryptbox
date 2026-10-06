// 客户端本地日志（client.log）：记录本机的关键操作（解锁/锁定/导入导出/同步等），
// 供「日志」页查看。日志只落本地数据目录，绝不上传，也不记录任何明文口令或条目内容。
//
// 设计要点：
//   · 写入 best-effort：日志失败绝不影响业务功能（忽略所有 IO 错误）；
//   · 单行三段式 `时间\t动作\t详情`，动作/详情内的制表符与换行一律替换为空格，
//     防止伪造多行日志误导排查；
//   · 读取时从尾部截取，避免长期使用后日志过大拖慢界面；
//   · 清空即整文件截断（日志不含密钥类敏感数据，无需安全擦除）。

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

/// 串行化追加写，避免多命令并发交错输出。
static LOG_LOCK: Mutex<()> = Mutex::new(());

/// 日志文件路径：与数据库同目录（便携模式在 exe 旁，安装模式在配置目录）。
pub fn log_file_path() -> PathBuf {
    crate::store::data_dir().join("client.log")
}

/// 清洗单行字段：制表符/换行替换为空格，保证一行一条日志。
fn sanitize(s: &str) -> String {
    s.replace(['\t', '\n', '\r'], " ")
}

/// 追加一条日志（best-effort，任何失败都静默忽略）。
pub fn log(action: &str, detail: &str) {
    let line = format!(
        "{}\t{}\t{}\n",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
        sanitize(action),
        sanitize(detail)
    );
    let _guard = LOG_LOCK.lock();
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(log_file_path()) {
        let _ = f.write_all(line.as_bytes());
    }
}

/// 读取最近 N 条日志（新在前），供前端「日志」页展示。
#[tauri::command]
pub fn read_client_logs(limit: Option<usize>) -> Result<Vec<String>, String> {
    let limit = limit.unwrap_or(500).clamp(1, 5000);
    let content = std::fs::read_to_string(log_file_path()).unwrap_or_default();
    let lines: Vec<String> = content
        .lines()
        .rev()
        .take(limit)
        .map(|s| s.to_string())
        .collect();
    Ok(lines)
}

/// 清空本地日志。
#[tauri::command]
pub fn clear_client_logs() -> Result<(), String> {
    std::fs::write(log_file_path(), "").map_err(|e| e.to_string())
}
