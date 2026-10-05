// 网络模块：局域网同步服务器发现（PT-10）。
//
// 旧实现只探测明文 `http://ip:5201/api/health`，且判定条件宽松（状态 200 +
// 响应体含 "ok"）——任意 HTTP 服务都很容易伪造成"同步服务器"（钓鱼）。
// 现改为：
//   · 同一主机同时探测 https 与 http，**优先展示 https**；
//   · 判定改为校验 `/api/status` 的 JSON 结构（必须含 `initialized` 字段）；
//   · https 结果附带对端证书指纹，供用户在界面上一并核对。
use std::collections::HashSet;
use std::net::Ipv4Addr;
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use serde::Serialize;

use crate::sync::pinning;

const SYNC_PORT: u16 = 5201;

/// 并发探测的固定工作线程数。
/// 早期实现「每主机一个线程」：双网卡 /24 会瞬时创建 500+ 线程，且每个线程在
/// https 优先探测里还要新建 ObservedClient（含 rustls ClientConfig 构建 + 证书栈
/// 初始化），资源开销与抖动都很大。改为固定大小线程池后 OS 线程数恒定。
const SCAN_WORKERS: usize = 64;

/// 单主机探测超时。
/// https 探测失败后还会回退 http，因此最坏耗时约为该值的两倍；收敛到 400ms 量级
/// 后，整段扫描（含超时主机）从「秒级 × N」降到可接受的几秒内。
const PROBE_TIMEOUT: Duration = Duration::from_millis(400);

/// 局域网发现到的一台同步服务器。
#[derive(Serialize, Clone)]
pub struct LanServer {
    /// 归一化后的访问地址（形如 https://192.168.1.5:5201）。
    pub url: String,
    /// "https" 或 "http"。
    pub scheme: String,
    /// 是否加密（https）。
    pub secure: bool,
    /// 对端证书指纹（仅 https 有效，冒号分隔大写十六进制）。
    pub fingerprint: String,
    /// 是否已由受信 CA 校验通过（自签为 false，需用户确认信任）。
    pub verified: bool,
}

/// 扫描局域网内可用的 CryPtBox 同步服务器。
/// 返回结果已按「加密优先」排序。
pub fn scan_lan() -> Result<Vec<LanServer>, String> {
    let local_ips = local_ipv4s();
    if local_ips.is_empty() {
        return Err("未检测到局域网地址".into());
    }

    // 收集所有待扫描主机（按 /24 网段，仅扫描私有网段）
    let mut host_set: HashSet<String> = HashSet::new();
    for ip in &local_ips {
        if !is_private_ipv4(ip) {
            continue;
        }
        let o = ip.octets();
        for i in 1..=254u8 {
            host_set.insert(format!("{}.{}.{}.{}", o[0], o[1], o[2], i));
        }
    }

    // 明文探测共享一个客户端（克隆开销极小）。
    // 注意：不能用 pinning::plain_client()——它面向用户显式配置的服务器、超时 15s，
    // 在局域网扫描里会让每个不存在的主机各拖满超时。这里用短超时的独立客户端。
    let plain_client = reqwest::blocking::Client::builder()
        .timeout(PROBE_TIMEOUT)
        .build()
        .unwrap_or_default();

    // 固定大小工作线程池：通过共享通道分发主机，OS 线程数恒定（不再随网卡/网段膨胀）。
    let results = Arc::new(Mutex::new(Vec::new()));
    let (tx, rx) = mpsc::channel::<String>();
    let rx = Arc::new(Mutex::new(rx));

    let worker_count = SCAN_WORKERS.min(host_set.len().max(1));
    let mut handles = Vec::with_capacity(worker_count);
    for _ in 0..worker_count {
        let results = Arc::clone(&results);
        let plain_client = plain_client.clone();
        let rx: Arc<Mutex<Receiver<String>>> = Arc::clone(&rx);
        handles.push(thread::spawn(move || loop {
            // 通道关闭或已取空即退出该 worker。
            let host = match rx.lock().unwrap().recv() {
                Ok(h) => h,
                Err(_) => break,
            };
            if let Some(s) = probe_host_secure(&host) {
                results.lock().unwrap().push(s);
                continue; // 已加密即最优结果，不再探测明文
            }
            if let Some(s) = probe_host_plain(&plain_client, &host) {
                results.lock().unwrap().push(s);
            }
        }));
    }
    // 投递全部主机后关闭发送端，worker 取空后自然退出。
    for host in host_set {
        let _ = tx.send(host);
    }
    drop(tx);
    for h in handles {
        let _ = h.join();
    }

    let mut list = results.lock().unwrap().clone();
    // 加密优先，其次按地址排序（结果稳定可复现）。
    list.sort_by(|a, b| {
        b.secure
            .cmp(&a.secure)
            .then_with(|| a.url.cmp(&b.url))
    });
    Ok(list)
}

/// HTTPS 探测：走 https 并取回对端证书指纹（自签可通过握手，指纹待用户确认）。
fn probe_host_secure(host: &str) -> Option<LanServer> {
    let url = format!("https://{}:{}/api/status", host, SYNC_PORT);
    let (body, fingerprint) = pinning::fetch_with_fingerprint(&url, PROBE_TIMEOUT)?;
    if !looks_like_cryptbox(&body) {
        return None;
    }
    Some(LanServer {
        url: format!("https://{}:{}", host, SYNC_PORT),
        scheme: "https".to_string(),
        secure: true,
        fingerprint,
        // 自签证书无法由系统 CA 校验通过；标记为待用户确认。
        verified: false,
    })
}

/// 明文 HTTP 探测（无加密、无身份校验）。
fn probe_host_plain(client: &reqwest::blocking::Client, host: &str) -> Option<LanServer> {
    let url = format!("http://{}:{}/api/status", host, SYNC_PORT);
    let resp = client.get(&url).send().ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let body = resp.text().ok()?;
    if !looks_like_cryptbox(&body) {
        return None;
    }
    Some(LanServer {
        url: format!("http://{}:{}", host, SYNC_PORT),
        scheme: "http".to_string(),
        secure: false,
        fingerprint: String::new(),
        verified: false,
    })
}

/// 判定响应是否为 CryPtBox 服务端：`/api/status` 的 JSON 必须包含 `initialized`（布尔）。
/// 比旧版的「状态码 200 且正文含 ok」严格得多，降低伪造服务器与误报概率。
fn looks_like_cryptbox(body: &str) -> bool {
    match serde_json::from_str::<serde_json::Value>(body) {
        Ok(v) => v.get("initialized").map(|x| x.is_boolean()).unwrap_or(false),
        Err(_) => false,
    }
}

/// 返回本机所有非回环的 IPv4 地址。
fn local_ipv4s() -> Vec<Ipv4Addr> {
    let mut out = Vec::new();
    if let Ok(ifaces) = get_if_addrs::get_if_addrs() {
        for iface in ifaces {
            if let get_if_addrs::IfAddr::V4(addr) = iface.addr {
                if !addr.ip.is_loopback() {
                    out.push(addr.ip);
                }
            }
        }
    }
    out
}

/// 判断是否为 RFC1918 私有网段（10.x / 172.16-31.x / 192.168.x）。
fn is_private_ipv4(ip: &Ipv4Addr) -> bool {
    let o = ip.octets();
    o[0] == 10 || (o[0] == 172 && o[1] >= 16 && o[1] <= 31) || (o[0] == 192 && o[1] == 168)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_structure_validation_is_strict() {
        assert!(looks_like_cryptbox(r#"{"initialized":true,"build":"abc"}"#));
        assert!(looks_like_cryptbox(r#"{"initialized":false,"build":"abc"}"#));
        // 旧判定（正文含 ok）会误报的样本，现在应被拒绝：
        assert!(!looks_like_cryptbox("ok"));
        assert!(!looks_like_cryptbox(r#"{"status":"ok"}"#), "缺少 initialized 字段应拒绝");
        assert!(!looks_like_cryptbox(r#"{"initialized":"yes"}"#), "initialize 非布尔应拒绝");
        assert!(!looks_like_cryptbox("<html>ok</html>"));
        assert!(!looks_like_cryptbox(""));
    }
}
