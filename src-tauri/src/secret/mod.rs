// 系统凭据库封装（PT-03）。
//
// 目标：本地不再出现「可直接使用的 vault_key 明文」。同步用的端到端加密密钥
// 交给操作系统凭据存储保管：
//   · Windows → 凭据管理器（DPAPI 保护的 Generic Credential）
//   · macOS   → 钥匙串（Keychain）
//   · Linux   → Secret Service（GNOME Keyring / KWallet 等桌面密钥环）
//
// 降级策略：Linux 无桌面密钥环（如无头服务器、容器）时，凭据库会返回错误。
// 此时退化为数据目录下 `vault_key.bin`（Unix 下 0600 权限），并由上层向用户
// 告警——安全性弱于凭据库，但仍远好于明文写在 SQLite 里（可被随手复制）。
//
// R11-05：**会话令牌不参与上述降级**。`vault_key` 必须在降级时持久化（否则离线
// 无法解密密码库，属不可避免的取舍），但会话令牌是可直接调用服务端 API 的凭据、
// 且不需要跨重启存活（重新登录即可）。因此降级模式下令牌只保留在内存：
// 既避免"裸 JWT 明文文件"随备份/副本外泄，也顺带清理历史遗留的明文令牌文件。
//
// 防护边界：凭据库能挡住「拿到数据文件/备份的人」，但挡不住「已控制当前
// 操作系统账户的恶意程序」——后者可以调用同样的凭据 API 读取密钥。
use std::path::{Path, PathBuf};

use base64::{engine::general_purpose, Engine};

/// 凭据库中的服务名（Windows 凭据管理器「网络地址」列）。
const SERVICE: &str = "CryPtBox";
/// 同步密钥的槽位名。应用同时只保留一个同步账号的密钥，故使用固定槽位。
const ACCOUNT_VAULT_KEY: &str = "vault_key";
/// 会话令牌（JWT）的槽位名（PT-06）：令牌交由 Rust 侧保管，不再落在 WebView 存储。
const ACCOUNT_SESSION: &str = "session_token";
/// 探测用的独立槽位（避免探针误删真实数据）。
const PROBE_ACCOUNT: &str = "vault_key_probe";
/// 强制走文件后端的开关（测试与「无密钥环」诊断用）。
const FORCE_FILE_ENV: &str = "CRYPTBOX_FORCE_FILE_SECRET";

/// 实际生效的存储后端。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    /// 操作系统凭据库。
    Keyring,
    /// 数据目录下的 0600 文件（降级）。
    File,
}

impl Backend {
    pub fn as_str(&self) -> &'static str {
        match self {
            Backend::Keyring => "keyring",
            Backend::File => "file",
        }
    }
}

pub struct SecretStore {
    dir: PathBuf,
    force_file: bool,
    /// 凭据库服务名。生产恒为 [`SERVICE`]；测试可传入独立服务名，
    /// 使测试永不触碰真实槽位（R12-02）。
    service: String,
}

impl SecretStore {
    /// 以数据目录构造凭据库封装（是否强制文件后端由环境变量决定）。
    pub fn new(data_dir: &Path) -> Self {
        let force_file = std::env::var(FORCE_FILE_ENV)
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        Self::with_backend(data_dir, force_file)
    }

    /// 显式指定是否强制使用文件后端。
    /// 独立于环境变量，便于并发测试（环境变量是进程级全局状态）。
    pub fn with_backend(data_dir: &Path, force_file: bool) -> Self {
        Self::with_service(data_dir, force_file, SERVICE)
    }

    /// 以指定服务名构造（R12-02）。
    ///
    /// 仅供测试隔离使用：测试传入 `CryPtBoxTest-<pid>` 之类的独立服务名，
    /// 即便用例误触删除路径也只会作用于测试条目，**绝不波及生产槽位**。
    /// 生产代码请使用 [`SecretStore::new`]。
    pub fn with_service(data_dir: &Path, force_file: bool, service: &str) -> Self {
        Self {
            dir: data_dir.to_path_buf(),
            force_file,
            service: service.to_string(),
        }
    }

    // ---- 同步密钥（vault_key，32 字节）----

    /// 写入同步密钥。返回实际使用的后端。
    pub fn set(&self, data: &[u8]) -> Result<Backend, String> {
        self.write_account(ACCOUNT_VAULT_KEY, &general_purpose::STANDARD.encode(data))
    }

    /// 读取同步密钥。返回 None 表示尚未写入过。
    pub fn get(&self) -> Result<Option<Vec<u8>>, String> {
        match self.read_account(ACCOUNT_VAULT_KEY)? {
            Some(s) => decode(&s).map(Some),
            None => Ok(None),
        }
    }

    /// 删除同步密钥。
    pub fn delete(&self) -> Result<(), String> {
        self.delete_account(ACCOUNT_VAULT_KEY)
    }

    // ---- 会话令牌（JWT，PT-06 / R11-05）----
    //
    // 会话令牌**只**交由系统凭据库保管，降级（无凭据库）时**不落盘**：
    // 它是可直接调用服务端 API 的凭据，且重启后重新登录即可恢复，
    // 不像 vault_key 那样有"必须持久化"的硬约束。

    /// 写入会话令牌。返回实际使用的后端（`File` 表示已降级、令牌仅存内存）。
    pub fn set_session(&self, token: &str) -> Result<Backend, String> {
        if !self.force_file {
            if let Ok(entry) = keyring::Entry::new(&self.service, ACCOUNT_SESSION) {
                match entry.set_password(token) {
                    Ok(()) => {
                        self.remove_legacy_session_file();
                        return Ok(Backend::Keyring);
                    }
                    Err(e) => eprintln!("[secret] 会话令牌写入凭据库失败：{e}"),
                }
            }
        }
        // 降级：不写文件（R11-05），并清理历史遗留的明文令牌文件。
        self.remove_legacy_session_file();
        eprintln!(
            "[secret] 无可用系统凭据库：会话令牌仅保留在内存中，重启后需重新登录（安全降级）"
        );
        Ok(Backend::File)
    }

    /// 读取会话令牌（None 表示未登录）。
    pub fn get_session(&self) -> Result<Option<String>, String> {
        if !self.force_file {
            if let Ok(entry) = keyring::Entry::new(&self.service, ACCOUNT_SESSION) {
                match entry.get_password() {
                    Ok(v) => return Ok(Some(v)),
                    Err(keyring::Error::NoEntry) => { /* 未写入 */ }
                    Err(e) => eprintln!("[secret] 会话令牌读取失败：{e}"),
                }
            }
        }
        // 降级模式不落盘。若发现历史版本遗留的明文令牌文件，读取一次供本次会话使用，
        // 随即删除以消除明文残留（下次启动必然要求重新登录）。
        if let Ok(s) = std::fs::read_to_string(self.fallback_path(ACCOUNT_SESSION)) {
            self.remove_legacy_session_file();
            let tok = s.trim().to_string();
            if !tok.is_empty() {
                return Ok(Some(tok));
            }
        }
        Ok(None)
    }

    /// 删除会话令牌。
    pub fn delete_session(&self) -> Result<(), String> {
        // R12-02：与 set/get 路径对称——force_file 模式**绝不触达系统凭据库**。
        // 否则测试（或 CRYPTBOX_FORCE_FILE_SECRET=1 的诊断运行）会删掉真实会话令牌。
        if !self.force_file {
            if let Ok(entry) = keyring::Entry::new(&self.service, ACCOUNT_SESSION) {
                match entry.delete_credential() {
                    Ok(()) | Err(keyring::Error::NoEntry) => {}
                    Err(e) => eprintln!("[secret] 删除会话令牌失败：{e}"),
                }
            }
        }
        self.remove_legacy_session_file();
        Ok(())
    }

    /// 删除历史遗留的降级会话令牌文件（R11-05）。
    fn remove_legacy_session_file(&self) {
        let p = self.fallback_path(ACCOUNT_SESSION);
        if p.exists() {
            if let Err(e) = std::fs::remove_file(&p) {
                eprintln!("[secret] 清理历史遗留的会话令牌文件失败：{e}");
            }
        }
    }

    // ---- 通用账号槽位实现 ----

    fn fallback_path(&self, account: &str) -> PathBuf {
        self.dir.join(format!("{account}.bin"))
    }

    fn write_account(&self, account: &str, value: &str) -> Result<Backend, String> {
        if !self.force_file {
            match keyring::Entry::new(&self.service, account) {
                Ok(entry) => match entry.set_password(value) {
                    Ok(()) => return Ok(Backend::Keyring),
                    Err(e) => eprintln!("[secret] 系统凭据库写入失败，降级为文件存储：{e}"),
                },
                Err(e) => eprintln!("[secret] 系统凭据库不可用，降级为文件存储：{e}"),
            }
        }
        self.write_file(&self.fallback_path(account), value)?;
        Ok(Backend::File)
    }

    fn read_account(&self, account: &str) -> Result<Option<String>, String> {
        if !self.force_file {
            if let Ok(entry) = keyring::Entry::new(&self.service, account) {
                match entry.get_password() {
                    Ok(v) => return Ok(Some(v)),
                    Err(keyring::Error::NoEntry) => { /* 未写入，继续看降级文件 */ }
                    Err(e) => eprintln!("[secret] 系统凭据库读取失败，尝试降级文件：{e}"),
                }
            }
        }
        match std::fs::read_to_string(self.fallback_path(account)) {
            Ok(s) => Ok(Some(s.trim().to_string())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    fn delete_account(&self, account: &str) -> Result<(), String> {
        // R12-02：与 write_account / read_account 对称，force_file 模式下跳过凭据库。
        // 这是 `cargo test` 曾抹掉真实 `vault_key` 的直接原因（测试用 with_backend(dir, true)，
        // 但旧 delete 路径无守卫，仍会调用 keyring 删除生产槽位）。
        if !self.force_file {
            if let Ok(entry) = keyring::Entry::new(&self.service, account) {
                match entry.delete_credential() {
                    Ok(()) | Err(keyring::Error::NoEntry) => {}
                    Err(e) => eprintln!("[secret] 删除凭据库条目失败：{e}"),
                }
            }
        }
        match std::fs::remove_file(self.fallback_path(account)) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }

    /// 探测当前环境实际可用的后端：写入—读回一个探针值后清理。
    /// 用于界面提示「凭据库不可用，已降级为文件存储」。
    pub fn probe(&self) -> Backend {
        if self.force_file {
            return Backend::File;
        }
        let probe_val = b"cryptbox-secret-probe";
        // 使用独立槽位，绝不触碰真实密钥。
        match keyring::Entry::new(&self.service, PROBE_ACCOUNT) {
            Ok(entry) => {
                let encoded = general_purpose::STANDARD.encode(probe_val);
                if entry.set_password(&encoded).is_ok() {
                    let ok = matches!(entry.get_password(), Ok(ref v) if v == &encoded);
                    let _ = entry.delete_credential();
                    if ok {
                        return Backend::Keyring;
                    }
                }
                Backend::File
            }
            Err(_) => Backend::File,
        }
    }

    fn write_file(&self, path: &Path, value: &str) -> Result<(), String> {
        let _ = std::fs::create_dir_all(&self.dir);
        std::fs::write(path, value.as_bytes()).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }
}

fn decode(s: &str) -> Result<Vec<u8>, String> {
    general_purpose::STANDARD
        .decode(s)
        .map_err(|e| format!("凭据数据损坏：{e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let mut d = std::env::temp_dir();
        d.push(format!(
            "cryptbox-secret-test-{}-{}",
            tag,
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn file_backend_roundtrip() {
        // 强制文件后端，验证写入/读回/删除与文件权限（Unix）。
        let dir = temp_dir("roundtrip");
        let store = SecretStore::with_backend(&dir, true);
        let data = [7u8; 32];

        assert_eq!(store.get().unwrap(), None, "初始应为空");
        assert_eq!(store.set(&data).unwrap(), Backend::File);
        assert_eq!(store.get().unwrap(), Some(data.to_vec()));
        assert_eq!(store.probe(), Backend::File);

        // 降级文件内容为 base64，不应包含原始字节明文。
        let raw = std::fs::read_to_string(dir.join("vault_key.bin")).unwrap();
        assert!(raw.len() >= 40, "base64 长度异常: {}", raw.len());

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(dir.join("vault_key.bin"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600, "降级文件权限应为 0600，实际 {:o}", mode);
        }

        // 会话令牌（PT-06 / R11-05）：降级模式下**不落盘**，避免裸 JWT 明文外泄。
        assert_eq!(store.get_session().unwrap(), None);
        assert_eq!(store.set_session("jwt-abc.def.ghi").unwrap(), Backend::File);
        assert!(
            !dir.join("session_token.bin").exists(),
            "R11-05：降级模式不得把裸 JWT 写入明文文件"
        );
        assert_eq!(
            store.get_session().unwrap(),
            None,
            "R11-05：降级模式下会话令牌不跨进程持久化"
        );
        assert_eq!(store.get().unwrap(), Some(data.to_vec()), "会话令牌不应影响同步密钥");

        // 历史遗留的明文令牌文件：读取一次供本次会话使用，随即清除（消除明文残留）。
        std::fs::write(dir.join("session_token.bin"), "legacy-jwt-token").unwrap();
        assert_eq!(
            store.get_session().unwrap().as_deref(),
            Some("legacy-jwt-token"),
            "遗留明文令牌应可读取一次（不打断升级用户）"
        );
        assert!(
            !dir.join("session_token.bin").exists(),
            "R11-05：遗留明文令牌文件应在读取后立即清除"
        );

        store.delete().unwrap();
        assert_eq!(store.get().unwrap(), None);
        // 重复删除幂等。
        store.delete().unwrap();
        store.delete_session().unwrap();
        assert_eq!(store.get_session().unwrap(), None);

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 本进程专用的测试服务名：与生产 `CryPtBox` 隔离，测试永不触碰真实槽位（R12-02）。
    fn test_service(tag: &str) -> String {
        format!("CryPtBoxTest-{}-{}", tag, std::process::id())
    }

    #[test]
    fn keyring_roundtrip_when_available() {
        // 若系统凭据库可用则做一次真实往返；不可用（无头 Linux）时跳过，避免误报失败。
        // R12-02：必须使用独立服务名，否则一次 `cargo test` 会覆盖并删除真实的 vault_key。
        let dir = temp_dir("keyring");
        let service = test_service("roundtrip");
        let store = SecretStore::with_service(&dir, false, &service);
        if store.probe() != Backend::Keyring {
            eprintln!("[secret] 当前环境无系统凭据库，跳过 keyring 往返用例");
            let _ = std::fs::remove_dir_all(&dir);
            return;
        }
        let data = [42u8; 32];
        assert_eq!(store.set(&data).unwrap(), Backend::Keyring);
        assert_eq!(store.get().unwrap(), Some(data.to_vec()));
        store.delete().unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn force_file_mode_never_touches_keyring() {
        // R12-02 回归：force_file 模式下**任何**路径（含 delete / delete_session）都不得
        // 触达系统凭据库。做法：在测试专用服务名下预置真实条目，调用 delete 系列后断言其仍在。
        let dir = temp_dir("force-file-guard");
        let service = test_service("guard");
        let store = SecretStore::with_service(&dir, true, &service);

        let seed = |account: &str, val: &str| -> bool {
            keyring::Entry::new(&service, account)
                .and_then(|e| e.set_password(val))
                .is_ok()
        };
        if !seed(ACCOUNT_VAULT_KEY, "REAL-KEY") || !seed(ACCOUNT_SESSION, "REAL-JWT") {
            eprintln!("[secret] 当前环境无系统凭据库，跳过 force_file 守卫用例");
            let _ = std::fs::remove_dir_all(&dir);
            return;
        }

        store.delete().unwrap();
        store.delete_session().unwrap();

        let still = |account: &str| -> bool {
            keyring::Entry::new(&service, account)
                .and_then(|e| e.get_password())
                .is_ok()
        };
        let key_still = still(ACCOUNT_VAULT_KEY);
        let sess_still = still(ACCOUNT_SESSION);

        for acc in [ACCOUNT_VAULT_KEY, ACCOUNT_SESSION] {
            let _ = keyring::Entry::new(&service, acc).and_then(|e| e.delete_credential());
        }
        let _ = std::fs::remove_dir_all(&dir);

        assert!(
            key_still,
            "R12-02：force_file 模式下 delete() 不得删除系统凭据库条目"
        );
        assert!(
            sess_still,
            "R12-02：force_file 模式下 delete_session() 不得删除系统凭据库条目"
        );
    }
}
