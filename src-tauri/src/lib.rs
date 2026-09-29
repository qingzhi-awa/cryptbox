// CryPtBox Tauri 客户端库入口。
// 模块按功能分类：app（命令注册）/ crypto（加密）/ store（存储）/
// sync（同步）/ import_export（导入导出）/ settings（设置）/
// network（局域网扫描）/ tray（系统托盘）。
mod app;
mod crypto;
mod import_export;
mod network;
mod settings;
mod store;
mod sync;
mod tray;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .setup(|app| {
            app::init(app.handle()).map_err(std::io::Error::other)?;
            tray::setup_tray(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app::init_db,
            app::setup_master,
            app::unlock,
            app::lock,
            app::is_unlocked,
            app::list_entries,
            app::save_entry,
            app::delete_entry,
            app::list_trash,
            app::restore_entry,
            app::purge_entry,
            app::empty_trash,
            app::export_txt,
            app::export_csv,
            app::import_csv,
            app::import_txt,
            app::save_text_file,
            app::get_server_config,
            app::sync_register,
            app::sync_login,
            app::sync_check,
            app::sync_send_code,
            app::push_vault,
            app::pull_vault,
            app::merge_vault,
            app::scan_lan,
            app::get_settings,
            app::save_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running CryPtBox");
}
