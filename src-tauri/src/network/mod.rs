// 网络模块：局域网同步服务器发现（扫描 TCP 5201 的 /api/health）。
use std::collections::HashSet;
use std::net::Ipv4Addr;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

const SYNC_PORT: u16 = 5201;

/// 扫描局域网内可用的 CryPtBox 同步服务器，返回形如 http://ip:5201 的地址列表。
pub fn scan_lan() -> Result<Vec<String>, String> {
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

    let results = Arc::new(Mutex::new(Vec::new()));
    let mut handles = Vec::new();
    for host in host_set {
        let results = Arc::clone(&results);
        handles.push(thread::spawn(move || {
            let url = format!("http://{}:{}/api/health", host, SYNC_PORT);
            let client = reqwest::blocking::Client::builder()
                .timeout(Duration::from_millis(400))
                .build()
                .unwrap_or_default();
            if let Ok(resp) = client.get(&url).send() {
                if resp.status().is_success() {
                    if let Ok(body) = resp.text() {
                        if body.contains("ok") {
                            results
                                .lock()
                                .unwrap()
                                .push(format!("http://{}:{}", host, SYNC_PORT));
                        }
                    }
                }
            }
        }));
    }
    for h in handles {
        let _ = h.join();
    }

    let mut list = results.lock().unwrap().clone();
    list.sort();
    Ok(list)
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
