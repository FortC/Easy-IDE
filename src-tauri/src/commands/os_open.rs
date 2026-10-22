//! 系统级打开：Windows 右键菜单 / 双击 md 文件拉起应用。
//! 启动参数中的文件路径 → 定位所属 vault（没有则以其所在文件夹建库）→ 打开笔记。

use crate::state::AppState;
use tauri::{AppHandle, State};

#[derive(serde::Serialize)]
pub struct OpenNoteResult {
    pub root: String,
    pub notes: Vec<crate::index::model::NoteIndex>,
    pub rel: String,
    /// 临时打开时的原始文件绝对路径；保存时前端据此回写原文件。
    /// vault 模式恒为 None（编辑的就是原文件）。
    pub origin_abs: Option<String>,
}

// ---- 临时文件来源映射（rel 文件名 → 原始绝对路径），持久化到配置目录 ----

fn temp_origins_file(app: &AppHandle) -> std::path::PathBuf {
    crate::config::config_dir(app).join("temp-origins.json")
}

fn load_temp_origins(path: &std::path::Path) -> std::collections::HashMap<String, String> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn store_temp_origins(path: &std::path::Path, map: &std::collections::HashMap<String, String>) {
    if let Ok(s) = serde_json::to_string(map) {
        let _ = std::fs::write(path, s);
    }
}

/// 取出启动时传入的待打开文件（读取后清空）
#[tauri::command]
pub fn get_pending_file(state: State<'_, AppState>) -> Option<String> {
    let mut pf = state.pending_file.lock().ok()?;
    pf.take()
}

/// 打开一个磁盘上的 md/canvas 文件：
/// 1) 在已知 vault 列表中找包含它的库（最长路径前缀）；
/// 2) 找不到则把它所在文件夹作为新库打开；
/// 3) 返回 vault 上下文 + 文件相对路径，前端直接展示。
/// 打开工作区是重 IO（缓存加载/监听启动），放后台线程避免冻结窗口。
#[tauri::command]
pub async fn open_path_from_os(
    app: AppHandle,
    path: String,
) -> Result<OpenNoteResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let abs = std::path::PathBuf::from(&path);
        if !abs.is_absolute() {
            return Err("必须使用绝对路径".into());
        }
        let abs_norm = abs.to_string_lossy().replace('\\', "/");
        let abs_norm = abs_norm.trim_end_matches('/').to_string();
        let lower = abs_norm.to_lowercase();

        // 找包含该文件的 vault（最长前缀优先）
        let vaults = crate::config::load_vaults(&app);
        let mut best: Option<String> = None;
        for v in &vaults {
            let vp = v.path.replace('\\', "/");
            let vp = vp.trim_end_matches('/').to_string();
            let vpl = vp.to_lowercase();
            if lower.starts_with(&format!("{}/", vpl)) {
                match &best {
                    Some(b) if b.len() >= vp.len() => {}
                    _ => best = Some(vp),
                }
            }
        }

        let root_str = match best {
            Some(v) => v,
            None => {
                // 不属于任何库：以文件所在文件夹为库
                abs_norm
                    .rsplit_once('/')
                    .map(|(dir, _)| dir.to_string())
                    .unwrap_or_else(|| abs_norm.clone())
            }
        };

        let rel = abs_norm
            .strip_prefix(&root_str)
            .unwrap_or("")
            .trim_start_matches('/')
            .to_string();

        let res = super::vault::open_vault_inner(
            app,
            std::path::PathBuf::from(root_str.clone()),
            root_str,
        )?;
        Ok(OpenNoteResult {
            root: res.root,
            notes: res.notes,
            rel,
            origin_abs: None,
        })
    })
    .await
    .map_err(|e| format!("后台任务异常：{}", e))?
}

// ---- Windows 右键菜单注册（绿色版也可用，写 HKCU 无需管理员） ----

fn write_context_reg(exe: &str) -> Result<(), String> {
    use winreg::enums::*;
    use winreg::RegKey;
    let cmd = format!("\"{}\" \"%1\"", exe);
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let classes = hkcu
        .open_subkey_with_flags("Software\\Classes", KEY_WRITE)
        .map_err(|e| format!("打开注册表失败：{}", e))?;

    for ext in ["md", "markdown", "canvas"] {
        // 1) 直接的右键菜单项："用 EasyIDE 打开"
        let shell_path = format!("SystemFileAssociations\\.{}\\shell\\EasyIDE", ext);
        let (key, _) = classes
            .create_subkey(&shell_path)
            .map_err(|e| format!("写入注册表失败：{}", e))?;
        key.set_value("", &"用 EasyIDE 打开").map_err(|e| e.to_string())?;
        key.set_value("Icon", &exe).map_err(|e| e.to_string())?;
        let (cmd_key, _) = key
            .create_subkey("command")
            .map_err(|e| format!("写入注册表失败：{}", e))?;
        cmd_key.set_value("", &cmd).map_err(|e| e.to_string())?;

        // 2) "打开方式"候选列表
        let (prog_key, _) = classes
            .create_subkey("EasyIDE.file")
            .map_err(|e| format!("写入注册表失败：{}", e))?;
        prog_key
            .set_value("", &"EasyIDE 文档")
            .map_err(|e| e.to_string())?;
        let (icon_key, _) = prog_key
            .create_subkey("DefaultIcon")
            .map_err(|e| format!("写入注册表失败：{}", e))?;
        icon_key
            .set_value("", &format!("{},0", exe))
            .map_err(|e| e.to_string())?;
        let (open_key, _) = prog_key
            .create_subkey(r"shell\open\command")
            .map_err(|e| format!("写入注册表失败：{}", e))?;
        open_key.set_value("", &cmd).map_err(|e| e.to_string())?;
        let (ext_key, _) = classes
            .create_subkey(format!(".{}\\OpenWithProgids", ext))
            .map_err(|e| format!("写入注册表失败：{}", e))?;
        ext_key.set_value("EasyIDE.file", &"").map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn remove_context_reg() -> Result<(), String> {
    use winreg::enums::*;
    use winreg::RegKey;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let classes = hkcu
        .open_subkey_with_flags("Software\\Classes", KEY_WRITE)
        .map_err(|e| format!("打开注册表失败：{}", e))?;
    for ext in ["md", "markdown", "canvas"] {
        let _ = classes.delete_subkey_all(format!("SystemFileAssociations\\.{}\\shell\\EasyIDE", ext));
        let _ = classes.delete_subkey_all(format!(".{}\\OpenWithProgids", ext));
        let _ = classes.delete_subkey_all("EasyIDE.file");
    }
    Ok(())
}



/// 临时文件打开：拷贝到内置临时目录后以该目录为 vault 打开（拷贝 + 打开放后台线程）
#[tauri::command]
pub async fn open_as_temp(
    app: AppHandle,
    path: String,
) -> Result<OpenNoteResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let src = std::path::PathBuf::from(&path);
        if !src.is_absolute() || !src.exists() {
            return Err("文件不存在".into());
        }

        // 临时目录：app_config_dir/temp/
        let temp_dir = crate::config::config_dir(&app).join("temp");
        std::fs::create_dir_all(&temp_dir).map_err(|e| format!("创建临时目录失败：{}", e))?;

        // 拷贝文件
        let name = src.file_name().unwrap_or_default().to_string_lossy().to_string();
        let dst = temp_dir.join(&name);
        std::fs::copy(&src, &dst).map_err(|e| format!("拷贝失败：{}", e))?;

        // 记录「临时副本 → 原始文件」映射并持久化，保存时前端据此回写原文件
        let origins_path = temp_origins_file(&app);
        let mut origins = load_temp_origins(&origins_path);
        origins.insert(
            name.clone(),
            src.to_string_lossy().to_string(),
        );
        store_temp_origins(&origins_path, &origins);

        // 以临时目录为 vault 打开
        let root_str = temp_dir.to_string_lossy().to_string();
        let res = super::vault::open_vault_inner(app, temp_dir, root_str)?;
        Ok(OpenNoteResult {
            root: res.root,
            notes: res.notes,
            rel: name,
            origin_abs: Some(src.to_string_lossy().to_string()),
        })
    })
    .await
    .map_err(|e| format!("后台任务异常：{}", e))?
}


/// 临时文件保存回写：按「临时副本文件名」查来源映射，把内容写回原始文件。
/// 返回 Ok(true) = 已回写；Ok(false) = 该文件不是临时打开（无映射，调用方可忽略）。
/// 映射存在但原始文件丢失时返回 Err —— 不能静默吞掉用户的编辑。
#[tauri::command]
pub fn save_temp_back(app: AppHandle, path: String, content: String) -> Result<bool, String> {
    let origins_path = temp_origins_file(&app);
    let origins = load_temp_origins(&origins_path);
    let Some(origin) = origins.get(&path) else {
        return Ok(false);
    };
    let dst = std::path::PathBuf::from(origin);
    if !dst.exists() {
        return Err(format!("原始文件已不存在：{}", origin));
    }
    std::fs::write(&dst, content).map_err(|e| format!("回写原文件失败：{}", e))?;
    Ok(true)
}

/// 打开 Windows 设置 → 默认应用（让用户设EasyIDE 为 .md 默认打开方式）
/// 走 CREATE_NO_WINDOW：否则 `cmd /C start` 会闪一个黑窗。
#[tauri::command]
pub fn open_default_apps_settings() -> Result<(), String> {
    crate::toolchain::hidden_new("cmd")
        .args(["/C", "start", "", "ms-settings:defaultapps"])
        .spawn()
        .map_err(|e| format!("Open settings failed: {}", e))?;
    Ok(())
}

/// 注册右键菜单（使用当前运行的 exe 路径）
#[tauri::command]
pub fn register_context_menu() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| format!("获取程序路径失败：{}", e))?;
    let exe = exe.to_string_lossy().to_string();
    write_context_reg(&exe)
}

/// 取消注册
#[tauri::command]
pub fn unregister_context_menu() -> Result<(), String> {
    remove_context_reg()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temp_origins_roundtrip() {
        let dir = std::env::temp_dir().join(format!("easyide-test-origins-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("temp-origins.json");

        assert!(load_temp_origins(&file).is_empty(), "不存在的映射文件应返回空表");

        let mut map = std::collections::HashMap::new();
        map.insert("a.md".to_string(), r"C:\docs\a.md".to_string());
        map.insert("b.md".to_string(), "/home/u/b.md".to_string());
        store_temp_origins(&file, &map);

        let loaded = load_temp_origins(&file);
        assert_eq!(loaded.get("a.md").unwrap(), r"C:\docs\a.md");
        assert_eq!(loaded.get("b.md").unwrap(), "/home/u/b.md");

        // 同名文件再次临时打开 → 覆盖旧来源（语义：映射始终对应当前副本）
        map.insert("a.md".to_string(), r"C:\other\a.md".to_string());
        store_temp_origins(&file, &map);
        let reloaded = load_temp_origins(&file);
        assert_eq!(reloaded.len(), 2);
        assert_eq!(reloaded.get("a.md").unwrap(), r"C:\other\a.md");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn temp_origins_tolerates_corrupt_file() {
        let dir = std::env::temp_dir().join(format!("easyide-test-origins-bad-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("temp-origins.json");
        std::fs::write(&file, "{not valid json").unwrap();
        assert!(load_temp_origins(&file).is_empty(), "损坏的映射文件应回退为空表而非报错");
        std::fs::remove_dir_all(&dir).ok();
    }
}
