// lib.rs —— Tauri 入口 + command 注册
//
// 中文说明：
// load_board / load_project 把 board.rs 解析出的结构化数据序列化为 JSON 交给前端。
// 前端零解析（02 §2 推荐方案）。App 端零智能：不调用任何 LLM、不做网络请求。
// M3 新增：详情页（load_project）、动作按钮（open_progress / open_in_editor）、
//          App 内增删（add_project / remove_project，登记逻辑见 registry.rs，与 cra.py 对齐）。

mod board;
mod push;
mod registry;
mod watcher;

use board::{Board, ProjectDetail};
use registry::AddResult;

/// Tauri command：加载整盘看板数据。永不 panic——单项目错误降级为卡片 error 标记。
#[tauri::command]
fn load_board() -> Board {
    board::load_board()
}

/// Tauri command：加载单项目详情（卡片字段 + 正文原文）。找不到 id 返回 Err。
#[tauri::command]
fn load_project(id: String) -> Result<ProjectDetail, String> {
    board::load_project(&id)
}

/// Tauri command：用系统默认程序打开 PROGRESS.md（`open <file>`）。
/// 通过 std::process::Command 调 macOS `open`，不依赖 shell 插件权限。
#[tauri::command]
fn open_progress(path: String) -> Result<(), String> {
    let p = board::expand_tilde(&path);
    if !p.exists() {
        return Err(format!("文件不存在: {}", p.display()));
    }
    std::process::Command::new("open")
        .arg(&p)
        .spawn()
        .map_err(|e| format!("打开文件失败: {e}"))?;
    Ok(())
}

/// Tauri command：用 VS Code 打开项目目录。
/// 先试 `code <path>`；`code` 不在 PATH 时降级 `open -a "Visual Studio Code" <path>`；
/// 再失败则返回 Err，前端 toast 提示。
#[tauri::command]
fn open_in_editor(path: String) -> Result<(), String> {
    let p = board::expand_tilde(&path);
    if !p.exists() {
        return Err(format!("项目路径不存在: {}", p.display()));
    }
    // 1) 试 PATH 里的 code
    let code_ok = std::process::Command::new("code").arg(&p).spawn().is_ok();
    if code_ok {
        return Ok(());
    }
    // 2) 降级：open -a "Visual Studio Code"
    let fallback = std::process::Command::new("open")
        .arg("-a")
        .arg("Visual Studio Code")
        .arg(&p)
        .spawn();
    match fallback {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("未找到 VS Code（PATH 无 code，且 open -a 失败: {e}）")),
    }
}

/// Tauri command：App 内添加项目。登记逻辑与 cra.py 对齐（registry.rs）。
#[tauri::command]
fn add_project(path: String, name: Option<String>) -> Result<AddResult, String> {
    registry::add_project_to(&board::default_registry_path(), &path, name.as_deref())
}

/// Tauri command：App 内删除项目。仅从 registry 移除，绝不触碰项目文件。
#[tauri::command]
fn remove_project(id: String) -> Result<(), String> {
    registry::remove_project_from(&board::default_registry_path(), &id)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        // M4：在 setup 钩子拿到 app_handle，起后台线程跑 notify 文件监听。
        // registry 目录先 create_dir_all，保证首次启动（还没 cra add 过）也能监听到它，
        // 之后 cra add 第一次写 registry 就能被捕获、自动出现卡片。监听失败不阻断 App 启动。
        .setup(|app| {
            let _ = std::fs::create_dir_all(
                board::default_registry_path()
                    .parent()
                    .unwrap_or_else(|| std::path::Path::new(".")),
            );
            watcher::spawn(app.handle().clone());
            // 「手机查看」：启动即推一次初始快照，手机端不必等到首次文件变化才有数据。
            // 受 TB_PUSH_URL 控制，未配置则 no-op（见 push.rs）。
            push::push_board_async();
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            load_board,
            load_project,
            open_progress,
            open_in_editor,
            add_project,
            remove_project
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
