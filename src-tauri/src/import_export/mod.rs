// 导入导出模块：CSV/TXT 解析与导出（兼容 Chrome/Edge 导出的 CSV、GBK 编码）。
use csv::{ReaderBuilder, WriterBuilder};
use std::fs;
use std::io::Write;

use crate::store::Entry;

pub fn export_txt(path: &str, list: &[Entry]) -> Result<i64, String> {
    let mut f = fs::File::create(path).map_err(|e| e.to_string())?;
    let _ = writeln!(f, "密匣导出");
    let _ = writeln!(
        f,
        "导出时间: {}",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    );
    let _ = writeln!(f, "{:=>60}", "");
    for e in list {
        let _ = writeln!(f, "标题: {}", e.title);
        let _ = writeln!(f, "用户名: {}", e.username);
        let _ = writeln!(f, "密码: {}", e.password);
        let _ = writeln!(f, "网址: {}", e.url);
        let _ = writeln!(f, "分类: {}", e.category);
        let _ = writeln!(f, "备注: {}", e.notes);
        let _ = writeln!(f, "{:->60}", "");
    }
    Ok(list.len() as i64)
}

/// CSV 公式注入防护：Excel/WPS 会将以 = + - @ \t \r 开头的字段当作公式执行
/// （如 DDE 注入）。导出时统一前置单引号，强制按文本处理。
///
/// R11-10：Excel/WPS 会忽略单元格里的**前导空白**，因此 `" =1+1"` 这类同样危险。
/// 判定必须纵向拉开到第一个非空白字符（原实现只看首字符，可被前导空格绕过）。
fn csv_safe(v: &str) -> String {
    let first_danger = v.starts_with(['=', '+', '-', '@', '\t', '\r']);
    let after_space_danger = v.trim_start().starts_with(['=', '+', '-', '@']);
    if first_danger || after_space_danger {
        format!("'{v}")
    } else {
        v.to_string()
    }
}

pub fn export_csv(path: &str, list: &[Entry]) -> Result<i64, String> {
    let mut f = fs::File::create(path).map_err(|e| e.to_string())?;
    f.write_all(b"\xef\xbb\xbf").map_err(|e| e.to_string())?; // UTF-8 BOM
    let mut wtr = WriterBuilder::new().from_writer(f);
    wtr.write_record(&["name", "username", "password", "url", "category", "notes"])
        .map_err(|e| e.to_string())?;
    for e in list {
        wtr.write_record(&[
            csv_safe(&e.title),
            csv_safe(&e.username),
            csv_safe(&e.password),
            csv_safe(&e.url),
            csv_safe(&e.category),
            csv_safe(&e.notes),
        ])
        .map_err(|e| e.to_string())?;
    }
    wtr.flush().map_err(|e| e.to_string())?;
    Ok(list.len() as i64)
}

fn normalize_header(s: &str) -> String {
    s.trim_start_matches('\u{feff}')
        .trim()
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}

fn match_field(h: &str) -> Option<&'static str> {
    let h = normalize_header(h);
    let fields: &[(&str, &[&str])] = &[
        (
            "password",
            &["password", "pass", "pwd", "密码", "口令", "密碼", "パスワード", "비밀번호"],
        ),
        (
            "username",
            &["username", "user", "login", "account", "用户名", "账号", "账户", "登录"],
        ),
        ("url", &["url", "uri", "website", "网址", "網址", "链接", "地址"]),
        ("notes", &["note", "remark", "备注", "说明", "注释", "備註", "メモ", "메모"]),
        ("category", &["category", "group", "folder", "分类", "分组", "目录", "分類", "분류"]),
        ("title", &["title", "name", "名称", "标题", "网站", "站点", "应用", "服务"]),
    ];
    for (field, aliases) in fields {
        for alias in *aliases {
            if h.contains(&normalize_header(alias)) {
                return Some(field);
            }
        }
    }
    None
}

fn decode(data: &[u8]) -> String {
    if data.starts_with(b"\xef\xbb\xbf") || std::str::from_utf8(data).is_ok() {
        String::from_utf8_lossy(data).to_string()
    } else {
        let (cow, _, _) = encoding_rs::GBK.decode(data);
        cow.to_string()
    }
}

pub fn parse_csv_file(path: &str) -> Result<Vec<Entry>, String> {
    let data = fs::read(path).map_err(|e| e.to_string())?;
    parse_csv_data(&data)
}

fn parse_csv_data(data: &[u8]) -> Result<Vec<Entry>, String> {
    let decoded = decode(data);
    let mut rdr = ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(decoded.as_bytes());
    let headers = rdr.headers().map_err(|e| e.to_string())?.clone();
    let mut col_map: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for (i, h) in headers.iter().enumerate() {
        if let Some(f) = match_field(h) {
            col_map.entry(f).or_insert(i);
        }
    }
    if !col_map.contains_key("title") && !col_map.contains_key("password") {
        for (i, f) in ["title", "username", "password", "url", "notes", "category"]
            .iter()
            .enumerate()
        {
            col_map.entry(f).or_insert(i);
        }
    }
    let get = |row: &csv::StringRecord, field: &str| -> String {
        col_map
            .get(field)
            .and_then(|&i| row.get(i))
            .unwrap_or("")
            .trim()
            .to_string()
    };
    let mut entries = Vec::new();
    for result in rdr.records() {
        let row = result.map_err(|e| e.to_string())?;
        let e = Entry {
            id: 0,
            uuid: String::new(),
            pinned: false,
            local_rev: 0,
            sort_order: 0,
            title: get(&row, "title"),
            username: get(&row, "username"),
            password: get(&row, "password"),
            url: get(&row, "url"),
            category: get(&row, "category"),
            notes: get(&row, "notes"),
            created_at: String::new(),
            updated_at: String::new(),
            deleted: false,
        };
        if e.title.is_empty() && e.username.is_empty() && e.password.is_empty() {
            continue;
        }
        entries.push(e);
    }
    Ok(entries)
}

pub fn parse_txt_file(path: &str) -> Result<Vec<Entry>, String> {
    let data = fs::read(path).map_err(|e| e.to_string())?;
    parse_txt_data(&data)
}

#[cfg(test)]
mod tests {
    use super::{csv_safe, parse_txt_data};

    #[test]
    fn dangerous_prefixes_are_prefixed() {
        for d in ["=cmd|'/c calc'!A0", "+SUM(A1)", "-1", "@import", "\tx", "\ry"] {
            assert!(csv_safe(d).starts_with('\''), "应加前缀: {}", d);
        }
    }

    // R11-10 回归：前导空白 + 公式字符同样危险（Excel/WPS 会忽略前导空白）。
    #[test]
    fn dangerous_prefixes_with_leading_space_are_prefixed() {
        for d in [" =1+1", "   @SUM(A1)", "\t=cmd|'/c calc'!A0", " \t-CMD()"] {
            assert!(
                csv_safe(d).starts_with('\''),
                "前导空白后接公式字符应加前缀: {d:?}"
            );
        }
    }

    #[test]
    fn normal_values_untouched() {
        assert_eq!(csv_safe("hello"), "hello");
        assert_eq!(csv_safe(""), "");
        // 中间位置的 = 不转义，只有开头才危险
        assert_eq!(csv_safe("a=b"), "a=b");
        assert_eq!(csv_safe("user@mail.com"), "user@mail.com");
    }

    // 回归：TXT 导入遇到全角冒号 '：'（3 字节 UTF-8）时，早期实现按 `line[idx + 1..]`
    // 取值会切在字符中间，触发 "byte index N is not a char boundary" panic。中文导出的
    // 密码文件普遍使用全角冒号，这是可被普通用户导入动作触发的崩溃。修复后按字符实际
    // 字节长度推进，且半角/全角两种写法都要正确解析。
    #[test]
    fn txt_full_width_colon_does_not_panic_and_parses() {
        let data = "标题： 我的账号\n用户名： alice\n密码： secret：123\n".as_bytes();
        let entries = parse_txt_data(data).expect("全角冒号 TXT 应能解析");
        assert_eq!(entries.len(), 1, "应解析出 1 条");
        assert_eq!(entries[0].title, "我的账号");
        assert_eq!(entries[0].username, "alice");
        // 值内部的第二个全角冒号应原样保留
        assert_eq!(entries[0].password, "secret：123");
    }

    #[test]
    fn txt_half_width_colon_still_parses() {
        let data = "标题: 我的账号\n密码: secret123\n".as_bytes();
        let entries = parse_txt_data(data).expect("半角冒号 TXT 应能解析");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].title, "我的账号");
        assert_eq!(entries[0].password, "secret123");
    }
}

fn parse_txt_data(data: &[u8]) -> Result<Vec<Entry>, String> {
    let decoded = decode(data);
    let text = decoded.replace("\r\n", "\n").replace('\r', "\n");
    let mut entries: Vec<Entry> = Vec::new();
    let mut cur = Entry {
        id: 0,
        uuid: String::new(),
        pinned: false,
        local_rev: 0,
        sort_order: 0,
        title: String::new(),
        username: String::new(),
        password: String::new(),
        url: String::new(),
        category: String::new(),
        notes: String::new(),
        created_at: String::new(),
        updated_at: String::new(),
        deleted: false,
    };
    let mut has_cur = false;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let is_sep = line.trim_matches('-').is_empty() || line.trim_matches('=').is_empty();
        if is_sep {
            if line.contains('-') {
                if has_cur && (!cur.title.is_empty() || !cur.username.is_empty() || !cur.password.is_empty()) {
                    entries.push(cur.clone());
                }
                cur = Entry {
                    id: 0,
                    uuid: String::new(),
                    pinned: false,
                    local_rev: 0,
                    sort_order: 0,
                    title: String::new(),
                    username: String::new(),
                    password: String::new(),
                    url: String::new(),
                    category: String::new(),
                    notes: String::new(),
                    created_at: String::new(),
                    updated_at: String::new(),
                    deleted: false,
                };
                has_cur = false;
            }
            continue;
        }
        if let Some((sep_pos, sep)) = line.char_indices().find(|(_, c)| *c == ':' || *c == '：') {
            // 分隔符可能是半角 ':'（1 字节）或全角 '：'（3 字节），必须按字符实际字节长度
            // 推进，否则 `line[sep_pos + 1..]` 会切在全角字符中间，触发
            // "byte index N is not a char boundary" panic（全角冒号是中文文件里的常见写法）。
            let _ = sep;
            let key = line[..sep_pos].trim();
            let val = line[sep_pos + sep.len_utf8()..].trim();
            match match_field(key) {
                Some("title") => {
                    if has_cur && (!cur.title.is_empty() || !cur.username.is_empty() || !cur.password.is_empty()) {
                        entries.push(cur.clone());
                    }
                    cur = Entry {
                        id: 0,
                        uuid: String::new(),
                        pinned: false,
                        local_rev: 0,
                        sort_order: 0,
                        title: String::new(),
                        username: String::new(),
                        password: String::new(),
                        url: String::new(),
                        category: String::new(),
                        notes: String::new(),
                        created_at: String::new(),
                        updated_at: String::new(),
                        deleted: false,
                    };
                    cur.title = val.to_string();
                    has_cur = true;
                }
                Some("username") => {
                    cur.username = val.to_string();
                    has_cur = true;
                }
                Some("password") => {
                    cur.password = val.to_string();
                    has_cur = true;
                }
                Some("url") => {
                    cur.url = val.to_string();
                    has_cur = true;
                }
                Some("category") => {
                    cur.category = val.to_string();
                    has_cur = true;
                }
                Some("notes") => {
                    cur.notes = val.to_string();
                    has_cur = true;
                }
                _ => {}
            }
        }
    }
    if has_cur && (!cur.title.is_empty() || !cur.username.is_empty() || !cur.password.is_empty()) {
        entries.push(cur);
    }
    Ok(entries)
}
