// 设置模块：客户端设置（自启/自动同步/同步策略）持久化。
use rusqlite::Connection;
use std::collections::HashMap;

pub fn get_settings(db: &Connection) -> HashMap<String, String> {
    let mut map = HashMap::new();
    map.insert(
        "autostart".into(),
        crate::store::get_meta(db, "set_autostart").unwrap_or_default(),
    );
    map.insert(
        "autosync".into(),
        crate::store::get_meta(db, "set_autosync").unwrap_or_default(),
    );
    map.insert(
        "priority".into(),
        crate::store::get_meta(db, "set_priority").unwrap_or_default(),
    );
    map.insert(
        "recycle".into(),
        crate::store::get_meta(db, "set_recycle").unwrap_or_else(|| "1".into()),
    );
    map.insert(
        "recycle_days".into(),
        crate::store::get_meta(db, "set_recycle_days").unwrap_or_else(|| "30".into()),
    );
    map
}

pub fn save_settings(
    db: &Connection,
    autostart: bool,
    autosync: bool,
    priority: &str,
    recycle: bool,
    recycle_days: i64,
    app: &tauri::AppHandle,
) -> Result<(), String> {
    let priority = match priority {
        "local" | "server" | "merge" => priority,
        _ => "local",
    };
    crate::store::set_meta(db, "set_autostart", if autostart { "1" } else { "0" })?;
    crate::store::set_meta(db, "set_autosync", if autosync { "1" } else { "0" })?;
    crate::store::set_meta(db, "set_priority", priority)?;
    crate::store::set_meta(db, "set_recycle", if recycle { "1" } else { "0" })?;
    crate::store::set_meta(db, "set_recycle_days", &recycle_days.max(1).to_string())?;

    // 开机自启：注册/注销系统自启动项（Windows 注册表 Run / macOS LaunchAgent）
    use tauri_plugin_autostart::ManagerExt;
    let autolaunch = app.autolaunch();
    if autostart {
        let _ = autolaunch.enable();
    } else {
        let _ = autolaunch.disable();
    }
    Ok(())
}
