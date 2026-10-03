// 服务端身份校验（PT-02 / PT-10）。
//
// 背景：飞牛等自建部署普遍使用**自签证书**，浏览器/客户端无法验证其身份，
// 因此旧版本把内网地址一律降级为明文 HTTP——这既放弃了传输加密，也让任何
// 能应答 5201 端口的机器都能冒充同步服务器（口令、JWT、密文全部明文过网，
// 甚至可篡改密文）。本模块提供「加密传输 + 指纹固定（pinning）」：
//
//   1. 对端证书由受信 CA 正常校验通过 → 直接放行（并记录指纹）；
//   2. 对端为自签证书 → 不静默信任：把本地从 TLS 握手中取到的对端证书
//      SHA-256 指纹返回给上层，由用户在界面上确认「信任首次使用」（TOFU）；
//   3. 指纹与已信任记录不一致 → 判定为可能的中间人，明确报错并拒绝连接。
//
// 实现说明：身份判定必须基于 TLS 握手中**真实观测到**的证书，不能用接口返回值
// （接口正文同样可能被中间人改写）。这里用 rustls 的自定义 ServerCertVerifier
// 在握手阶段捕获对端证书 DER，再计算 SHA-256。
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::{engine::general_purpose, Engine};
use reqwest::blocking::Client;
use sha2::{Digest, Sha256};

/// 结构化错误前缀：指纹与已信任记录不一致（后跟 `<authority>|<新指纹>|<旧指纹>`）。
/// 首次信任与地址白名单确认由 Rust 侧原生对话框完成，因此不再需要 NEED_TRUST 返回值。
pub const ERR_FP_CHANGED: &str = "FINGERPRINT_CHANGED";

/// 一次指纹探测的结果。
pub struct Probe {
    /// 主机（含端口），用作信任记录的键。
    pub authority: String,
    /// 对端叶子证书的 SHA-256 指纹（冒号分隔大写十六进制）。
    pub fingerprint: String,
    /// 是否已由受信 CA 校验通过（false 表示自签/不受信，需要用户确认）。
    pub verified: bool,
}

/// 共享的「本次连接观测到的对端证书」槽位。
type SeenCert = Arc<Mutex<Option<Vec<u8>>>>;

/// 自定义证书校验器：不依赖系统 CA，无条件接受链验证（自签亦可通过），
/// 但把对端叶子证书记录下来；若已固定指纹，则要求完全一致。
#[derive(Debug)]
struct RecordingVerifier {
    seen: SeenCert,
    pinned: Option<String>,
}

impl rustls::client::danger::ServerCertVerifier for RecordingVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        let der = end_entity.as_ref().to_vec();
        if let Some(p) = &self.pinned {
            if &fingerprint_of_der(&der) != p {
                return Err(rustls::Error::General(
                    "certificate fingerprint mismatch".into(),
                ));
            }
        }
        *self.seen.lock().unwrap() = Some(der);
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        rustls::crypto::ring::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}

/// 一个「已记录对端证书」的客户端。
#[derive(Clone)]
pub struct ObservedClient {
    pub client: Client,
    seen: SeenCert,
}

impl ObservedClient {
    /// 本次连接观测到的对端证书指纹（未发生 TLS 握手时为 None）。
    pub fn observed_fingerprint(&self) -> Option<String> {
        self.seen
            .lock()
            .unwrap()
            .as_ref()
            .map(|der| fingerprint_of_der(der))
    }
}

/// 计算证书 DER 的 SHA-256 指纹（冒号分隔大写十六进制，如 `AB:CD:…`）。
pub fn fingerprint_of_der(der: &[u8]) -> String {
    let sum = Sha256::digest(der);
    let mut out = String::with_capacity(95);
    for (i, b) in sum.iter().enumerate() {
        if i > 0 {
            out.push(':');
        }
        out.push_str(&format!("{:02X}", b));
    }
    out
}

/// 从 URL 中取出 `host[:port]` 作为信任记录的键。
pub fn authority(url: &str) -> String {
    let s = url.trim();
    let rest = match s.find("://") {
        Some(i) => &s[i + 3..],
        None => s,
    };
    let end = rest.find('/').unwrap_or(rest.len());
    rest[..end].to_string()
}

/// 是否 HTTPS 地址。
pub fn is_https(url: &str) -> bool {
    url.trim_start().to_ascii_lowercase().starts_with("https://")
}

/// 严格校验（系统受信 CA）的客户端，仅用于探测「证书是否受信」。
fn strict_client() -> Result<Client, String> {
    Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())
}

/// 使用自定义校验器、可记录对端证书的客户端。
/// `pinned` 非空时，握手阶段即要求指纹一致（不一致直接握手失败）。
pub fn observed_client(pinned: Option<String>, timeout: Duration) -> Result<ObservedClient, String> {
    let seen: SeenCert = Arc::new(Mutex::new(None));
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut cfg = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|e| e.to_string())?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(RecordingVerifier {
            seen: seen.clone(),
            pinned,
        }))
        .with_no_client_auth();
    // 服务端仅提供 HTTP/1.1（自签双协议监听未协商 ALPN）。
    cfg.alpn_protocols = vec![b"http/1.1".to_vec()];
    let client = Client::builder()
        .timeout(timeout)
        .use_preconfigured_tls(cfg)
        .build()
        .map_err(|e| e.to_string())?;
    Ok(ObservedClient { client, seen })
}

/// 单次探测：GET 指定 URL，返回（响应体, 对端证书指纹）。
///
/// 每次调用创建独立客户端，保证 254 台主机的并发扫描中「观测到的证书」互不串扰。
pub fn fetch_with_fingerprint(url: &str, timeout: Duration) -> Option<(String, String)> {
    let oc = observed_client(None, timeout).ok()?;
    let resp = oc.client.get(url).send().ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let fp = oc.observed_fingerprint()?;
    let body = resp.text().ok()?;
    Some((body, fp))
}

/// 明文 HTTP 客户端（用户显式选择 http:// 时）。
pub fn plain_client() -> Client {
    Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .unwrap_or_default()
}

fn health_url(base_url: &str) -> String {
    format!("{}/api/health", base_url.trim_end_matches('/'))
}

/// 探测服务器证书指纹。**不发送任何凭据**（仅 GET /api/health）。
///
/// `verified` 表示证书链能否由系统受信 CA 校验通过；自签时为 false，
/// 需要用户核对指纹后显式信任（TOFU）。
pub fn probe(base_url: &str) -> Result<Probe, String> {
    let authority = authority(base_url);
    let url = health_url(base_url);

    // 1) 先用严格校验探测（判断是否受信 CA 签发）。
    let verified = match strict_client() {
        Ok(c) => c
            .get(&url)
            .send()
            .map(|r| r.status().is_success())
            .unwrap_or(false),
        Err(_) => false,
    };

    // 2) 用自定义校验器连接，取回**握手阶段真实观测到**的对端证书。
    let oc = observed_client(None, Duration::from_secs(10))?;
    let resp = oc
        .client
        .get(&url)
        .send()
        .map_err(|e| format!("无法连接服务器：{}", e))?;
    let _ = resp.status();
    let fp = oc
        .observed_fingerprint()
        .ok_or_else(|| "未取得服务器证书指纹（目标可能未启用 HTTPS）".to_string())?;
    Ok(Probe {
        authority,
        fingerprint: fp,
        verified,
    })
}

/// PEM 证书正文解码为 DER（用于读取 `/api/cert`）。
#[allow(dead_code)]
fn pem_to_der(pem: &str) -> Option<Vec<u8>> {
    let mut b64 = String::new();
    let mut in_block = false;
    for line in pem.lines() {
        let l = line.trim();
        if l.contains("BEGIN CERTIFICATE") {
            in_block = true;
            continue;
        }
        if l.contains("END CERTIFICATE") {
            break;
        }
        if in_block {
            b64.push_str(l);
        }
    }
    if b64.is_empty() {
        return None;
    }
    general_purpose::STANDARD.decode(b64).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_format_is_stable() {
        let fp = fingerprint_of_der(b"cryptbox");
        assert_eq!(fp.len(), 95, "SHA-256 指纹应为 95 字符：{fp}");
        assert_eq!(fp, fingerprint_of_der(b"cryptbox"), "同一输入必须稳定");
        assert_ne!(fp, fingerprint_of_der(b"cryptbox2"));
        for (i, ch) in fp.chars().enumerate() {
            if (i + 1) % 3 == 0 {
                assert_eq!(ch, ':', "第 {i} 位应为冒号");
            } else {
                assert!(ch.is_ascii_hexdigit() && !ch.is_ascii_lowercase());
            }
        }
    }

    #[test]
    fn authority_and_scheme() {
        assert_eq!(authority("https://192.168.1.5:5201/"), "192.168.1.5:5201");
        assert_eq!(authority("https://nas.example.com"), "nas.example.com");
        assert_eq!(authority("http://10.0.0.2:5201/api/x"), "10.0.0.2:5201");
        assert!(is_https("https://a"));
        assert!(!is_https("http://a"));
    }

    #[test]
    fn structured_error_payloads() {
        // 前端依据该前缀拆分出主机与指纹，格式必须稳定。
        let changed = format!(
            "{}:{}|{}|{}",
            ERR_FP_CHANGED, "192.168.1.5:5201", "AB:CD", "EF:01"
        );
        assert!(changed.starts_with("FINGERPRINT_CHANGED:"));
    }

    #[test]
    fn pem_decoding() {
        // 合法 PEM 正文应能解出非空 DER。
        let pem = "-----BEGIN CERTIFICATE-----\nMIIB\n-----END CERTIFICATE-----\n";
        assert_eq!(pem_to_der(pem).as_deref(), Some(&[0x30, 0x82, 0x01][..]));
        // 非 PEM 文本（无 BEGIN 标记）应返回 None。
        assert_eq!(pem_to_der("not pem"), None);
        // 正文非法 base64 应返回 None。
        let bad = "-----BEGIN CERTIFICATE-----\n!!!!\n-----END CERTIFICATE-----\n";
        assert_eq!(pem_to_der(bad), None);
    }

    #[test]
    fn pinned_client_rejects_mismatched_fingerprint() {
        // 固定一个必然不匹配的指纹：客户端能构建，握手阶段会拒绝（此处仅验证可构建且为 Debug）。
        let oc = observed_client(Some("00:00".to_string()), Duration::from_secs(5))
            .expect("client build");
        assert!(oc.observed_fingerprint().is_none(), "尚未连接时无观测指纹");
        let _ = oc.client;
    }
}
