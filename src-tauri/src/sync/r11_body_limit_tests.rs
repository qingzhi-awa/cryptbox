// R11-04 回归测试：客户端对**响应体**必须有大小上限。
//
// 背景：reqwest 对响应体没有默认上限，`text()/json()/bytes()` 会一次性读入内存。
// 客户端可能连到恶意/被控的同步服务器；且 `network::scan_lan` 会探测整个 /24 网段，
// 网段内任一主机都可成为应答方。对端只要持续发送数据即可耗尽客户端内存（DoS）。
// 修复后所有响应体统一经 `read_body_capped` 受限读取。

use super::{read_body_capped, read_body_capped_string, MAX_PROBE_BODY};
use std::io::{Read, Write};
use std::net::TcpListener;

/// 起一个一次性 HTTP 服务返回给定的响应体，返回其 base_url。
///
/// `content_length = false` 用于构造"无 Content-Length、以连接关闭界定正文"的响应，
/// 以覆盖不依赖声明长度的**流式读取**路径。
fn spawn_server(body: Vec<u8>, content_length: bool) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("绑定本地端口");
    let addr = listener.local_addr().expect("读取本地地址");
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let mut s = match stream {
                Ok(s) => s,
                Err(_) => break,
            };
            let mut buf = [0u8; 2048];
            let _ = s.read(&mut buf); // 读掉请求行/头即可
            let head = if content_length {
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                )
            } else {
                "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\n"
                    .to_string()
            };
            let _ = s.write_all(head.as_bytes());
            let _ = s.write_all(&body);
            let _ = s.flush();
            // s 在此处 drop → 关闭连接（无 Content-Length 时即靠此界定正文结束）。
        }
    });
    format!("http://{addr}")
}

fn get(url: &str) -> reqwest::blocking::Response {
    reqwest::blocking::Client::new()
        .get(url)
        .send()
        .expect("发送请求失败")
}

#[test]
fn read_body_capped_rejects_oversize_by_content_length() {
    // 服务端**声明**了超过上限的长度：应在读取前就拒绝。
    let base = spawn_server(vec![b'a'; 200_000], true);
    let resp = get(&format!("{base}/api/vault"));
    let err = read_body_capped(resp, 64 * 1024).expect_err("声明超限的响应必须被拒绝");
    assert!(err.contains("过大"), "错误信息应说明响应体过大：{err}");
}

#[test]
fn read_body_capped_rejects_oversize_streamed() {
    // 未声明长度（流式/连接关闭界定）：也必须在超过上限时中断。
    let base = spawn_server(vec![b'b'; 200_000], false);
    let resp = get(&format!("{base}/api/vault"));
    let err = read_body_capped(resp, 64 * 1024).expect_err("流式超限的响应必须被拒绝");
    assert!(err.contains("过大"), "错误信息应说明响应体过大：{err}");
}

#[test]
fn read_body_capped_accepts_normal_response() {
    let base = spawn_server(b"{\"initialized\":true}".to_vec(), true);
    let resp = get(&format!("{base}/api/status"));
    let s = read_body_capped_string(resp, MAX_PROBE_BODY).expect("正常响应应可读取");
    assert_eq!(s, "{\"initialized\":true}");
}
