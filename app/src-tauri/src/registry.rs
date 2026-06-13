// registry.rs —— App 内增删项目的登记逻辑（M3）
//
// 中文说明（重要逻辑 / 数据契约对齐）：
// 本模块在 Rust 侧重新实现 cli/cra.py 的 add / remove 逻辑，避免 App 依赖 Python 环境。
// **必须与 cra.py 写出的 registry.yaml / PROGRESS.md 格式完全一致**（M3 双向一致验收项）：
//   - registry schema 见 02 §1.2：version: 1 + projects[]（id/name/path/progress_file/pinned/added）
//   - PROGRESS.md 模板见 02 §1.1：frontmatter + 「## 阶段记录」正文
//   - 字段写入顺序与 cra.py 的 yaml.safe_dump(sort_keys=False) 保持一致
//   - 写入必须原子：写临时文件 → fsync → rename（同目录），杜绝半截写入损坏 registry
//
// App 端零智能、零网络。所有路径展开 ~ 并容忍中文路径。

use crate::board::expand_tilde;
use serde::Serialize;
use std::io::Write;
use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;

const PROGRESS_FILENAME: &str = "PROGRESS.md";
const REGISTRY_VERSION: u32 = 1;

/// 操作结果回传给前端：成功时带新登记的 id 与是否生成了模板。
#[derive(Debug, Serialize)]
pub struct AddResult {
    pub id: String,
    pub name: String,
    pub path: String,
    /// true 表示本次新生成了 PROGRESS.md 模板；false 表示沿用已有文件
    pub created_template: bool,
}

// ───────────────────────── 工具：日期 / kebab-case ─────────────────────────

/// 返回本地日期 ISO 字符串 YYYY-MM-DD。
/// 本地时区的今天日期 YYYY-MM-DD，与 cra.py 的 date.today() 一致。
/// 优先调系统 `date +%F`（拿本地时区，避免 UTC 跨午夜与 cra.py 差一天）；
/// 失败时回退 UTC 天数推算（无第三方依赖，仅在 date 不可用时降级）。
fn today_iso() -> String {
    if let Ok(out) = std::process::Command::new("date").arg("+%F").output() {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if looks_like_iso_date(&s) {
                return s;
            }
        }
    }
    // 回退：UTC 天数换算公历（跨午夜窗口可能与本地差一天，仅降级用）
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = (secs / 86_400) as i64;
    let (y, m, d) = civil_from_days(days);
    format!("{:04}-{:02}-{:02}", y, m, d)
}

// 把 1970-01-01 起的天数转成公历年月日（Howard Hinnant 算法）
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as i64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

/// 目录名转 kebab-case 作为 project id，规则与 cra.py 的 kebab_case 对齐：
/// 小写 → 非 [0-9a-z 中日韩文字] 连续片段折叠为单连字符 → 去重连字符 → 去首尾连字符 → 空则 "project"。
fn kebab_case(name: &str) -> String {
    let lower = name.trim().to_lowercase();
    let mut out = String::new();
    let mut prev_dash = false;
    for ch in lower.chars() {
        // 与 cra.py 的正则 [^0-9a-z一-鿿] 等价：保留 ASCII 字母数字与 CJK 统一表意文字区
        let keep = ch.is_ascii_digit()
            || ('a'..='z').contains(&ch)
            || ('\u{4e00}'..='\u{9fff}').contains(&ch);
        if keep {
            out.push(ch);
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    let trimmed = out.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "project".to_string()
    } else {
        trimmed
    }
}

// ───────────────────────── registry 读 / 原子写 ─────────────────────────

/// 极简 registry 表示：保留原始 projects 为有序映射列表，写回时按 cra.py 字段顺序输出。
/// 为保证与 cra.py 双向一致，这里不依赖 serde 重排，而是手工读出再手工写回。
struct RegistryDoc {
    version: u32,
    projects: Vec<ProjectEntry>,
}

#[derive(Clone)]
struct ProjectEntry {
    id: String,
    name: String,
    path: String,
    progress_file: String,
    pinned: bool,
    added: String,
}

fn read_registry(path: &Path) -> Result<RegistryDoc, String> {
    if !path.exists() {
        return Ok(RegistryDoc {
            version: REGISTRY_VERSION,
            projects: vec![],
        });
    }
    let raw = std::fs::read_to_string(path).map_err(|e| format!("读取 registry 失败: {e}"))?;
    let val: serde_yaml::Value =
        serde_yaml::from_str(&raw).map_err(|e| format!("registry 解析失败: {e}"))?;
    if val.is_null() {
        return Ok(RegistryDoc {
            version: REGISTRY_VERSION,
            projects: vec![],
        });
    }
    let map = val
        .as_mapping()
        .ok_or_else(|| "registry 格式异常，期望映射".to_string())?;
    let version = map
        .get(serde_yaml::Value::from("version"))
        .and_then(|v| v.as_u64())
        .map(|v| v as u32)
        .unwrap_or(REGISTRY_VERSION);
    let mut projects = vec![];
    if let Some(list) = map
        .get(serde_yaml::Value::from("projects"))
        .and_then(|v| v.as_sequence())
    {
        for item in list {
            let m = match item.as_mapping() {
                Some(m) => m,
                None => return Err("registry 的某个 project 不是映射".to_string()),
            };
            let get_str = |k: &str| -> Option<String> {
                m.get(serde_yaml::Value::from(k))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            };
            projects.push(ProjectEntry {
                id: get_str("id").unwrap_or_default(),
                name: get_str("name").unwrap_or_default(),
                path: get_str("path").unwrap_or_default(),
                progress_file: get_str("progress_file")
                    .unwrap_or_else(|| PROGRESS_FILENAME.to_string()),
                pinned: m
                    .get(serde_yaml::Value::from("pinned"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
                added: get_str("added").unwrap_or_default(),
            });
        }
    }
    Ok(RegistryDoc { version, projects })
}

/// 用 PyYAML 风格序列化一个标量字符串值：
/// PyYAML safe_dump 对纯 ASCII / 中文普通字符串通常不加引号（plain style），
/// 但路径含特殊字符时会加引号。为稳妥与可读，这里复刻 PyYAML 的"按需加引号"行为的常见子集：
/// 仅当字符串需要时加单引号（含 `: ` / 前导特殊符 / 可被误解析为非字符串的字面量）。
fn yaml_scalar(s: &str) -> String {
    if needs_quote(s) {
        // PyYAML 单引号转义：内部单引号翻倍
        format!("'{}'", s.replace('\'', "''"))
    } else {
        s.to_string()
    }
}

/// 判断字符串是否形如 ISO 日期 YYYY-MM-DD（PyYAML 会把它当 timestamp 并加引号）。
fn looks_like_iso_date(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return false;
    }
    b.iter().enumerate().all(|(i, &c)| {
        if i == 4 || i == 7 {
            c == b'-'
        } else {
            c.is_ascii_digit()
        }
    })
}

fn needs_quote(s: &str) -> bool {
    if s.is_empty() {
        return true;
    }
    // 会被 YAML 解析成非字符串的字面量
    let lower = s.to_ascii_lowercase();
    if matches!(
        lower.as_str(),
        "true" | "false" | "null" | "~" | "yes" | "no" | "on" | "off"
    ) {
        return true;
    }
    // 纯数字 / 看起来像数字
    if s.parse::<f64>().is_ok() {
        return true;
    }
    // 看起来像 YAML 时间戳（ISO 日期 YYYY-MM-DD）——PyYAML 会给它加引号，逐字节对齐
    if looks_like_iso_date(s) {
        return true;
    }
    let first = s.chars().next().unwrap();
    if "!&*?|>%@`\"'#,[]{}".contains(first) || first == '-' || first == ' ' {
        return true;
    }
    // 含 ": " 或以 ":" 结尾或含 " #"
    if s.contains(": ") || s.ends_with(':') || s.contains(" #") {
        return true;
    }
    false
}

/// 原子写 registry：按 cra.py 字段顺序逐行生成 YAML（version → projects → 每项 id/name/path/progress_file/pinned/added），
/// 写临时文件 → fsync → rename（同目录，POSIX 原子替换）。
fn write_registry_atomic(path: &Path, doc: &RegistryDoc) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建 registry 目录失败: {e}"))?;
    }
    let mut out = String::new();
    out.push_str(&format!("version: {}\n", doc.version));
    if doc.projects.is_empty() {
        out.push_str("projects: []\n");
    } else {
        out.push_str("projects:\n");
        for p in &doc.projects {
            // PyYAML block 序列：列表项 "- " 与父键同列，后续字段对齐到 "- " 之后两空格
            out.push_str(&format!("- id: {}\n", yaml_scalar(&p.id)));
            out.push_str(&format!("  name: {}\n", yaml_scalar(&p.name)));
            out.push_str(&format!("  path: {}\n", yaml_scalar(&p.path)));
            out.push_str(&format!(
                "  progress_file: {}\n",
                yaml_scalar(&p.progress_file)
            ));
            out.push_str(&format!("  pinned: {}\n", if p.pinned { "true" } else { "false" }));
            out.push_str(&format!("  added: {}\n", yaml_scalar(&p.added)));
        }
    }

    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let tmp = parent.join(format!(".registry.{}.tmp", std::process::id()));
    {
        let mut f = std::fs::File::create(&tmp).map_err(|e| format!("写临时 registry 失败: {e}"))?;
        f.write_all(out.as_bytes())
            .map_err(|e| format!("写临时 registry 失败: {e}"))?;
        f.flush().ok();
        f.sync_all().ok();
    }
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("替换 registry 失败: {e}")
    })?;
    Ok(())
}

// ───────────────────────── 对外：add / remove（core，便于单测注入路径）─────────────────────────

/// App 内添加项目。逻辑与 cra.py 的 add 对齐：
/// 校验路径 → 已登记同 path 报错 → 生成 id → 追加并原子写 registry（不再生成 PROGRESS.md）。
pub fn add_project_to(
    registry_path: &Path,
    raw_path: &str,
    name: Option<&str>,
) -> Result<AddResult, String> {
    let proj_dir = expand_tilde(raw_path);
    // 校验路径（与 cra.py 一致）
    if !proj_dir.exists() {
        return Err(format!("路径不存在: {}", proj_dir.display()));
    }
    if !proj_dir.is_dir() {
        return Err(format!("不是目录: {}", proj_dir.display()));
    }
    // 规整为绝对路径（canonicalize 对齐 cra.py 的 resolve()）
    let abs = proj_dir
        .canonicalize()
        .unwrap_or(proj_dir.clone())
        .to_string_lossy()
        .to_string();

    let mut doc = read_registry(registry_path)?;

    // 已存在同 path 报错（以解析后的绝对路径比较）
    for p in &doc.projects {
        let existing = expand_tilde(&p.path);
        let existing_abs = existing
            .canonicalize()
            .unwrap_or(existing)
            .to_string_lossy()
            .to_string();
        if existing_abs == abs {
            return Err(format!("该路径已登记(id={}): {}", p.id, abs));
        }
    }

    let dir_name = proj_dir
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "project".to_string());
    let mut project_id = kebab_case(&dir_name);

    let existing_ids: std::collections::HashSet<String> =
        doc.projects.iter().map(|p| p.id.clone()).collect();

    // id 唯一性：已存在同 id 则追加短后缀
    if existing_ids.contains(&project_id) {
        let mut suffix = 2;
        while existing_ids.contains(&format!("{project_id}-{suffix}")) {
            suffix += 1;
        }
        project_id = format!("{project_id}-{suffix}");
    }

    // PROGRESS.md 已退役：add 只登记 registry，不再生成任何文件（与 cra.py 一致）。
    // 看板展示字段从三件套读，登记后用 /outkanban 生成。
    let created_template = false;

    let display_name = name
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.to_string())
        .unwrap_or(dir_name);

    doc.projects.push(ProjectEntry {
        id: project_id.clone(),
        name: display_name.clone(),
        path: abs.clone(),
        progress_file: PROGRESS_FILENAME.to_string(),
        pinned: false,
        added: today_iso(),
    });
    write_registry_atomic(registry_path, &doc)?;

    Ok(AddResult {
        id: project_id,
        name: display_name,
        path: abs,
        created_template,
    })
}

/// App 内删除项目。仅从 registry 移除登记，**绝不触碰项目文件**（02 §3 / AGENTS.md）。
pub fn remove_project_from(registry_path: &Path, project_id: &str) -> Result<(), String> {
    let mut doc = read_registry(registry_path)?;
    let before = doc.projects.len();
    doc.projects.retain(|p| p.id != project_id);
    if doc.projects.len() == before {
        return Err(format!("未找到 id: {project_id}"));
    }
    write_registry_atomic(registry_path, &doc)
}

// ═════════════════════════ 单元测试 ═════════════════════════
#[cfg(test)]
mod tests {
    use super::*;

    fn unique_tmp() -> PathBuf {
        let base = std::env::temp_dir().join(format!(
            "tb_reg_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&base).unwrap();
        base
    }

    #[test]
    fn test_kebab_case_matches_cra() {
        assert_eq!(kebab_case("Voice Pipeline"), "voice-pipeline");
        assert_eq!(kebab_case("My_Cool.Project!!"), "my-cool-project");
        assert_eq!(kebab_case("  --weird-- "), "weird");
        assert_eq!(kebab_case("语音管线"), "语音管线");
        assert_eq!(kebab_case("@@@"), "project");
    }


    #[test]
    fn test_add_only_registers_no_file() {
        let tmp = unique_tmp();
        let proj = tmp.join("voice");
        std::fs::create_dir_all(&proj).unwrap();
        let reg = tmp.join("registry.yaml");

        let r = add_project_to(&reg, proj.to_str().unwrap(), None).unwrap();
        assert!(!r.created_template, "PROGRESS.md 已退役，add 不再建文件");
        assert_eq!(r.id, "voice");
        assert!(!proj.join("PROGRESS.md").exists(), "不应生成 PROGRESS.md");

        // registry 可被读回；项目因无三件套看板信息显示「未接入」降级
        let board = crate::board::load_board_from(&reg);
        assert!(board.registry_error.is_none());
        assert_eq!(board.projects.len(), 1);
        assert_eq!(board.projects[0].id, "voice");
        assert_eq!(board.projects[0].error.as_ref().unwrap().kind, "missing");
    }

    #[test]
    fn test_add_duplicate_path_errors() {
        let tmp = unique_tmp();
        let proj = tmp.join("dup");
        std::fs::create_dir_all(&proj).unwrap();
        let reg = tmp.join("registry.yaml");
        add_project_to(&reg, proj.to_str().unwrap(), None).unwrap();
        let err = add_project_to(&reg, proj.to_str().unwrap(), None).unwrap_err();
        assert!(err.contains("已登记"), "got {err}");
        // 不应重复
        let doc = read_registry(&reg).unwrap();
        assert_eq!(doc.projects.len(), 1);
    }

    #[test]
    fn test_remove_only_touches_registry() {
        let tmp = unique_tmp();
        let proj = tmp.join("keepfiles");
        std::fs::create_dir_all(&proj).unwrap();
        // 用户项目里的文件（remove 绝不能动）
        let user_file = proj.join("README.md");
        std::fs::write(&user_file, "用户内容").unwrap();
        let reg = tmp.join("registry.yaml");
        let r = add_project_to(&reg, proj.to_str().unwrap(), None).unwrap();

        remove_project_from(&reg, &r.id).unwrap();
        // 项目文件必须完好（仅 registry 改动）
        assert!(user_file.exists(), "删除后用户文件必须保留");
        let doc = read_registry(&reg).unwrap();
        assert!(doc.projects.is_empty());

        // 删不存在的 id 报错
        assert!(remove_project_from(&reg, "ghost").is_err());
    }

    #[test]
    fn test_atomic_write_roundtrip_field_order() {
        // 验证写出的 YAML 字段顺序与 cra.py 一致（id/name/path/progress_file/pinned/added）
        let tmp = unique_tmp();
        let proj = tmp.join("order");
        std::fs::create_dir_all(&proj).unwrap();
        let reg = tmp.join("registry.yaml");
        add_project_to(&reg, proj.to_str().unwrap(), Some("订单系统")).unwrap();
        let raw = std::fs::read_to_string(&reg).unwrap();
        assert!(raw.starts_with("version: 1\nprojects:\n"));
        let id_pos = raw.find("id:").unwrap();
        let name_pos = raw.find("name:").unwrap();
        let path_pos = raw.find("path:").unwrap();
        let pf_pos = raw.find("progress_file:").unwrap();
        let pinned_pos = raw.find("pinned:").unwrap();
        let added_pos = raw.find("added:").unwrap();
        assert!(id_pos < name_pos);
        assert!(name_pos < path_pos);
        assert!(path_pos < pf_pos);
        assert!(pf_pos < pinned_pos);
        assert!(pinned_pos < added_pos);
        // 中文 name 应被 allow_unicode 原样写出
        assert!(raw.contains("name: 订单系统"));
    }
}

// ───── 双向一致性：App add 写出的 registry/PROGRESS.md 必须与 cra.py 字节一致 + 可被 cra list 读取 ─────
//
// 该测试需要 uv + cli/cra.py（本机环境有；无头 CI 无 uv 时自动跳过，不算失败）。
// 设置 TB_SKIP_CRA=1 可强制跳过。
#[cfg(test)]
mod xcheck {
    use super::*;
    use std::process::Command;

    fn cra_py_path() -> Option<PathBuf> {
        // 从 src-tauri 向上找到仓库根的 cli/cra.py
        let cargo_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")); // .../app/src-tauri
        let candidate = cargo_dir.join("../../cli/cra.py");
        candidate.canonicalize().ok()
    }

    fn unique_tmp() -> PathBuf {
        let base = std::env::temp_dir().join(format!(
            "tb_xc_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&base).unwrap();
        base
    }

    fn uv_available() -> bool {
        Command::new("uv")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    #[test]
    fn test_app_add_byte_identical_to_cra_and_listable() {
        if std::env::var("TB_SKIP_CRA").is_ok() || !uv_available() {
            eprintln!("跳过 cra 一致性测试（无 uv 或显式跳过）");
            return;
        }
        let cra = match cra_py_path() {
            Some(p) => p,
            None => {
                eprintln!("跳过：找不到 cli/cra.py");
                return;
            }
        };

        let base = unique_tmp();
        // App 侧：用真实 add 逻辑写
        let app_dir = base.join("app_side");
        std::fs::create_dir_all(app_dir.join("语音管线")).unwrap();
        std::fs::create_dir_all(app_dir.join("order-system")).unwrap();
        let app_reg = app_dir.join("registry.yaml");
        add_project_to(&app_reg, app_dir.join("语音管线").to_str().unwrap(), Some("语音管线"))
            .unwrap();
        add_project_to(&app_reg, app_dir.join("order-system").to_str().unwrap(), None).unwrap();

        // cra.py 侧：同样两个项目
        let py_dir = base.join("py_side");
        std::fs::create_dir_all(py_dir.join("语音管线")).unwrap();
        std::fs::create_dir_all(py_dir.join("order-system")).unwrap();
        let py_reg = py_dir.join("registry.yaml");
        // 注意：其他测试用 set_var 污染了进程级 HOME（test_expand_tilde），
        // 并行运行时会泄漏给 uv 导致缓存目录权限错误。这里给子进程显式传一个可写 HOME。
        let real_home = base.join("home");
        std::fs::create_dir_all(&real_home).unwrap();
        let run_cra = |args: &[&str], reg: &Path| {
            let out = Command::new("uv")
                .arg("run")
                .arg(&cra)
                .args(args)
                .env("CRA_REGISTRY", reg)
                .env("HOME", &real_home)
                .output()
                .expect("运行 cra.py 失败");
            if !out.status.success() {
                panic!(
                    "cra {:?} 失败: stdout={} stderr={}",
                    args,
                    String::from_utf8_lossy(&out.stdout),
                    String::from_utf8_lossy(&out.stderr)
                );
            }
            out
        };
        run_cra(&["add", py_dir.join("语音管线").to_str().unwrap(), "--name", "语音管线"], &py_reg);
        run_cra(&["add", py_dir.join("order-system").to_str().unwrap()], &py_reg);

        // 1) registry 字节一致（归一化各自的临时路径前缀后）
        let app_yaml = std::fs::read_to_string(&app_reg)
            .unwrap()
            .replace(app_dir.to_str().unwrap(), "BASE");
        let py_yaml = std::fs::read_to_string(&py_reg)
            .unwrap()
            .replace(py_dir.to_str().unwrap(), "BASE");
        assert_eq!(app_yaml, py_yaml, "registry 必须与 cra.py 字节一致");

        // 2) PROGRESS.md 已退役：两边都不再生成该文件
        assert!(!app_dir.join("语音管线/PROGRESS.md").exists(), "App add 不应生成 PROGRESS.md");
        assert!(!py_dir.join("语音管线/PROGRESS.md").exists(), "cra add 不应生成 PROGRESS.md");

        // 3) cra list 能正确读取 App 写出的 registry（双向一致）
        let out = run_cra(&["list"], &app_reg);
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(out.status.success(), "cra list 应成功: {stdout}");
        assert!(stdout.contains("语音管线"), "cra list 应列出 App 添加的项目: {stdout}");
        assert!(stdout.contains("order-system"), "cra list 应列出 App 添加的项目: {stdout}");
    }
}
