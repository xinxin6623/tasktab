// board.rs —— 看板数据解析核心
//
// 中文说明（重要逻辑）：
// 本模块负责把「registry.yaml + 各项目 PROGRESS.md frontmatter」解析成结构化数据，
// 返回给前端渲染。设计铁律（见 AGENTS.md / 02 §1.1）：
//   1. 防御性解析：任何单个项目的格式损坏 / 文件缺失都不得 panic，降级标记后其余项目照常返回。
//   2. App 端零智能：纯确定性解析，不调用 LLM、不做网络请求。
//   3. 路径必须展开 `~` 并容忍中文路径。
//   4. schema 之外的字段一律忽略（serde 默认行为即忽略未知字段，保证向前兼容）。

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

// ───────────────────────── registry.yaml schema（02 §1.2）─────────────────────────

#[derive(Debug, Deserialize)]
struct Registry {
    // version 字段保留向前兼容，当前仅读 projects
    #[allow(dead_code)]
    #[serde(default)]
    version: Option<u32>,
    #[serde(default)]
    projects: Vec<RegistryEntry>,
}

#[derive(Debug, Deserialize)]
struct RegistryEntry {
    id: String,
    #[serde(default)]
    name: Option<String>,
    path: String,
    #[serde(default = "default_progress_file")]
    progress_file: String,
    #[serde(default)]
    pinned: bool,
    #[serde(default)]
    added: Option<String>,
}

fn default_progress_file() -> String {
    "PROGRESS.md".to_string()
}

// ───────────────────────── PROGRESS.md frontmatter schema（02 §1.1）─────────────────────────

#[derive(Debug, Deserialize)]
struct Frontmatter {
    #[serde(default)]
    project: Option<String>,
    // desc 可选：项目一句话描述，卡片展示用；缺省按空串处理（02 §1.1）
    #[serde(default)]
    desc: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    stages: Vec<String>,
    #[serde(default)]
    current_stage: Option<i64>,
    // stage_progress 可选，缺省按 0 处理（02 §1.1）
    #[serde(default)]
    stage_progress: Option<f64>,
    #[serde(default)]
    next: Vec<String>,
    #[serde(default)]
    blocked_by: Vec<String>,
    #[serde(default)]
    updated: Option<String>,
}

// ───────────────────────── 对前端输出的结构（序列化为 JSON）─────────────────────────

/// 单个项目卡片数据。error 非 None 时表示降级卡片，前端显示「⚠ ...」+ 打开文件按钮。
#[derive(Debug, Serialize, PartialEq)]
pub struct ProjectCard {
    pub id: String,
    pub name: String,
    /// 绝对路径（已展开 ~），用于「打开文件」动作
    pub path: String,
    /// PROGRESS.md 的绝对路径
    pub progress_path: String,
    pub pinned: bool,

    // —— 以下字段在正常解析时填充；降级卡片可能为默认值 ——
    /// 项目一句话描述（来自 frontmatter desc，可选；缺省为空串，前端不渲染该行）
    pub desc: String,
    pub status: String, // active | paused | done | unknown
    pub stages: Vec<String>,
    pub current_stage: i64,
    pub stage_progress: f64,
    /// 整体进度百分比 0..=100（已按 §1.1 公式计算）
    pub overall_progress: f64,
    pub next: Vec<String>,
    pub blocked_by: Vec<String>,
    pub updated: String,

    /// 降级原因：None=正常；Some("format")=格式异常；Some("missing")=文件缺失
    pub error: Option<ParseError>,
}

#[derive(Debug, Serialize, PartialEq, Clone)]
pub struct ParseError {
    /// 错误种类：format（格式异常）/ missing（文件缺失）
    pub kind: String,
    /// 人类可读说明（中文），用于卡片提示
    pub message: String,
}

impl ParseError {
    fn format(msg: impl Into<String>) -> Self {
        ParseError { kind: "format".into(), message: msg.into() }
    }
    fn missing(msg: impl Into<String>) -> Self {
        ParseError { kind: "missing".into(), message: msg.into() }
    }
}

/// load_board 的整体返回：卡片列表 + 顶部汇总计数。
#[derive(Debug, Serialize)]
pub struct Board {
    pub projects: Vec<ProjectCard>,
    pub summary: Summary,
    /// registry 本身无法读取/解析时填充（整盘失败）；正常时为 None
    pub registry_error: Option<String>,
}

#[derive(Debug, Serialize, Default)]
pub struct Summary {
    pub active: usize,
    pub paused: usize,
    pub done: usize,
    pub error: usize, // 格式异常 + 文件缺失的项目数
}

// ───────────────────────── 路径处理 ─────────────────────────

/// 展开开头的 `~` 为用户主目录。容忍中文路径（直接按字节拼接，不做编码假设）。
pub fn expand_tilde(p: &str) -> PathBuf {
    expand_tilde_with(p, home_dir())
}

/// `~` 展开的纯函数核心：home 由调用方注入，不读全局 env。
/// 单测直接测本函数，避免改写进程级 HOME 造成测试间相互污染（flaky）。
fn expand_tilde_with(p: &str, home: Option<PathBuf>) -> PathBuf {
    if let Some(home) = home {
        if p == "~" {
            return home;
        }
        if let Some(rest) = p.strip_prefix("~/") {
            return home.join(rest);
        }
    }
    PathBuf::from(p)
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

/// registry.yaml 的默认路径（~/.ai-vault/taskboard/registry.yaml）
pub fn default_registry_path() -> PathBuf {
    expand_tilde("~/.ai-vault/taskboard/registry.yaml")
}

// ───────────────────────── 核心：整体进度公式（02 §1.1）─────────────────────────

/// 计算整体进度百分比 0..=100。
/// 公式：(current_stage - 1 + stage_progress/100) / len(stages) * 100
/// status 为 done 时强制 100%。
/// 注意：调用方需保证 stages 非空、current_stage 合法，否则属于「格式异常」由上游拦截。
fn compute_overall(status: &str, current_stage: i64, stage_progress: f64, stage_count: usize) -> f64 {
    if status == "done" {
        return 100.0;
    }
    if stage_count == 0 {
        return 0.0;
    }
    let sp = stage_progress.clamp(0.0, 100.0);
    let ratio = ((current_stage - 1) as f64 + sp / 100.0) / stage_count as f64;
    (ratio * 100.0).clamp(0.0, 100.0)
}

// ───────────────────────── frontmatter 提取 ─────────────────────────

/// 从 PROGRESS.md 全文中切出 YAML frontmatter 块（首行须为 `---`，到下一个 `---` 结束）。
/// 返回 None 表示没有合法 frontmatter（缺失）。
fn extract_frontmatter(content: &str) -> Option<&str> {
    // 容忍 BOM 与前置空白
    let trimmed = content.trim_start_matches('\u{feff}');
    let mut lines = trimmed.lines();
    // 首个非空行须是 ---
    let first = lines.next()?;
    if first.trim() != "---" {
        return None;
    }
    // 找到 frontmatter 起止字节范围
    let after_first = &trimmed[trimmed.find('\n')? + 1..];
    // 逐行扫描定位关闭分隔符（单独一行 ---），返回其间的 YAML 文本
    let mut offset = 0usize;
    for line in after_first.split_inclusive('\n') {
        if line.trim_end_matches(['\n', '\r']).trim() == "---" {
            return Some(&after_first[..offset]);
        }
        offset += line.len();
    }
    None
}

/// 对外暴露 frontmatter 提取（registry.rs 沿用已有 project id 时需要）。
pub fn extract_frontmatter_pub(content: &str) -> Option<&str> {
    extract_frontmatter(content)
}

/// 提取 frontmatter 之后的正文（用于详情页 markdown 只读预览）。
/// 没有合法 frontmatter 时返回去掉 BOM 的全文（防御性：绝不崩溃）。
pub fn extract_body(content: &str) -> String {
    let trimmed = content.trim_start_matches('\u{feff}');
    // 复用提取逻辑：找到第二个 --- 之后的内容
    let mut lines_iter = trimmed.lines();
    if lines_iter.next().map(|l| l.trim()) != Some("---") {
        return trimmed.to_string();
    }
    let after_first_idx = match trimmed.find('\n') {
        Some(i) => i + 1,
        None => return String::new(),
    };
    let after_first = &trimmed[after_first_idx..];
    let mut offset = 0usize;
    for line in after_first.split_inclusive('\n') {
        if line.trim_end_matches(['\n', '\r']).trim() == "---" {
            // 正文 = 关闭分隔符行之后的全部内容
            let body_start = offset + line.len();
            return after_first[body_start..].trim_start_matches(['\n', '\r']).to_string();
        }
        offset += line.len();
    }
    // 没有关闭分隔符：视作无 frontmatter，返回全文
    trimmed.to_string()
}

// ───────────────────────── 三件套块提取（详情页用）─────────────────────────
// App 端零智能：只做确定性的 markdown 块定位与文本截取，绝不解析语义、不调 LLM。
// 任一块缺失 / 格式不符 → 返回空，由前端静默降级（不显示该区域），绝不崩溃。

/// 提取 `## <heading>` 标题到下一个 `## ` 标题（或文件末尾）之间的正文。
/// 返回去掉首尾空白的块内容；找不到该标题返回 None。
pub fn extract_section(md: &str, heading: &str) -> Option<String> {
    let target = format!("## {heading}");
    let mut lines = md.lines();
    // 定位标题行
    loop {
        let line = lines.next()?;
        if line.trim() == target {
            break;
        }
    }
    // 收集到下一个 ## 标题为止
    let mut buf = String::new();
    for line in lines {
        let t = line.trim_start();
        if t.starts_with("## ") {
            break;
        }
        buf.push_str(line);
        buf.push('\n');
    }
    let s = buf.trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// 从一段文本中取首个 ```mermaid 代码块的内容（不含围栏）。无则 None。
pub fn extract_mermaid(section: &str) -> Option<String> {
    let mut lines = section.lines();
    // 找开围栏 ```mermaid
    loop {
        let line = lines.next()?;
        if line.trim().starts_with("```mermaid") {
            break;
        }
    }
    let mut buf = String::new();
    for line in lines {
        if line.trim().starts_with("```") {
            break;
        }
        buf.push_str(line);
        buf.push('\n');
    }
    let s = buf.trim_end().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// 解析 `## 项目阶段` 块下的 markdown checkbox 列表。
/// 每行形如 `- [x] 名 — 描述` / `- [ ] 名`；`[x]`/`[X]`=完成。
/// 找不到块或无合法行 → 空 vec。
pub fn extract_stage_list(md: &str) -> Vec<StageItem> {
    let section = match extract_section(md, "项目阶段") {
        Some(s) => s,
        None => return vec![],
    };
    let mut items = vec![];
    for raw in section.lines() {
        let line = raw.trim();
        // 必须形如 - [ ] 或 - [x]
        let rest = match line
            .strip_prefix("- [")
            .or_else(|| line.strip_prefix("* ["))
        {
            Some(r) => r,
            None => continue,
        };
        // rest 形如 "x] 名 — 描述" 或 " ] 名"
        let (mark, after) = match rest.split_once(']') {
            Some(v) => v,
            None => continue,
        };
        let done = matches!(mark.trim(), "x" | "X");
        let text = after.trim();
        // 名 — 描述（破折号分隔，描述可选）。支持 em dash — / 连字 - / 全角 ——
        let (name, desc) = split_name_desc(text);
        if name.is_empty() {
            continue;
        }
        items.push(StageItem {
            name,
            desc,
            done,
        });
    }
    items
}

/// 把 "名 — 描述" 拆成 (名, 描述)。无分隔符则全部当名、描述空。
fn split_name_desc(text: &str) -> (String, String) {
    for sep in [" — ", " - ", " —— ", "—", " – "] {
        if let Some((n, d)) = text.split_once(sep) {
            return (n.trim().to_string(), d.trim().to_string());
        }
    }
    (text.to_string(), String::new())
}

// ───────────────────────── 单项目解析 ─────────────────────────

/// 解析单个 registry 条目为一张卡片。永不 panic。
fn parse_entry(entry: &RegistryEntry) -> ProjectCard {
    let name = entry.name.clone().unwrap_or_else(|| entry.id.clone());
    let project_root = expand_tilde(&entry.path);
    let progress_path = project_root.join(&entry.progress_file);

    // 基础降级卡片模板
    let make_card = |error: Option<ParseError>, fm: Option<FmComputed>| -> ProjectCard {
        match fm {
            Some(c) => ProjectCard {
                id: entry.id.clone(),
                name: name.clone(),
                path: project_root.to_string_lossy().to_string(),
                progress_path: progress_path.to_string_lossy().to_string(),
                pinned: entry.pinned,
                desc: c.desc,
                status: c.status,
                stages: c.stages,
                current_stage: c.current_stage,
                stage_progress: c.stage_progress,
                overall_progress: c.overall_progress,
                next: c.next,
                blocked_by: c.blocked_by,
                updated: c.updated,
                error,
            },
            None => ProjectCard {
                id: entry.id.clone(),
                name: name.clone(),
                path: project_root.to_string_lossy().to_string(),
                progress_path: progress_path.to_string_lossy().to_string(),
                pinned: entry.pinned,
                desc: String::new(),
                status: "unknown".into(),
                stages: vec![],
                current_stage: 0,
                stage_progress: 0.0,
                overall_progress: 0.0,
                next: vec![],
                blocked_by: vec![],
                updated: entry.added.clone().unwrap_or_default(),
                error,
            },
        }
    };

    // 1) 文件缺失检查（02 §1.2）：path 不存在 或 progress_file 缺失
    if !project_root.exists() {
        return make_card(Some(ParseError::missing(format!("项目路径不存在: {}", project_root.display()))), None);
    }
    if !progress_path.exists() {
        return make_card(Some(ParseError::missing(format!("找不到进度文件: {}", progress_path.display()))), None);
    }

    // 2) 读文件（读失败也按文件缺失降级，绝不 panic）
    let content = match std::fs::read_to_string(&progress_path) {
        Ok(c) => c,
        Err(e) => return make_card(Some(ParseError::missing(format!("读取进度文件失败: {e}"))), None),
    };

    // 3) 提取 frontmatter
    let fm_text = match extract_frontmatter(&content) {
        Some(t) => t,
        None => return make_card(Some(ParseError::format("缺少 YAML frontmatter")), None),
    };

    // 4) YAML 解析（失败 → 格式异常）
    let fm: Frontmatter = match serde_yaml::from_str(fm_text) {
        Ok(f) => f,
        Err(e) => return make_card(Some(ParseError::format(format!("YAML 解析失败: {e}"))), None),
    };

    // 5) 语义校验（02 §1.1）：stages 为空 / current_stage 越界 → 格式异常
    if fm.stages.is_empty() {
        return make_card(Some(ParseError::format("stages 为空")), None);
    }
    let current_stage = match fm.current_stage {
        Some(n) => n,
        None => return make_card(Some(ParseError::format("缺少 current_stage")), None),
    };
    if current_stage < 1 || current_stage as usize > fm.stages.len() {
        return make_card(
            Some(ParseError::format(format!(
                "current_stage={} 越界（stages 共 {} 项）",
                current_stage,
                fm.stages.len()
            ))),
            None,
        );
    }

    // 6) 正常路径：计算整体进度
    let status = fm.status.clone().unwrap_or_else(|| "active".into());
    let stage_progress = fm.stage_progress.unwrap_or(0.0);
    let overall = compute_overall(&status, current_stage, stage_progress, fm.stages.len());

    let _ = fm.project; // schema 字段保留，App 不强制校验与 registry id 一致

    make_card(
        None,
        Some(FmComputed {
            desc: fm.desc.unwrap_or_default(),
            status,
            stages: fm.stages,
            current_stage,
            stage_progress,
            overall_progress: overall,
            next: fm.next,
            blocked_by: fm.blocked_by,
            updated: fm.updated.unwrap_or_default(),
        }),
    )
}

// 解析成功时的中间结果
struct FmComputed {
    desc: String,
    status: String,
    stages: Vec<String>,
    current_stage: i64,
    stage_progress: f64,
    overall_progress: f64,
    next: Vec<String>,
    blocked_by: Vec<String>,
    updated: String,
}

// ───────────────────────── 排序与汇总 ─────────────────────────

/// 排序规则（02 §3 M2）：done 项目排末尾，pinned 置顶。
/// 综合：先按 (是否 done) 升序 → 再按 (是否 pinned) 降序 → 再按 name 稳定排序。
fn sort_cards(cards: &mut [ProjectCard]) {
    cards.sort_by(|a, b| {
        let a_done = a.status == "done";
        let b_done = b.status == "done";
        // done 排末尾
        a_done
            .cmp(&b_done)
            // pinned 置顶（pinned=true 在前）
            .then(b.pinned.cmp(&a.pinned))
            // 稳定可读：按名称
            .then(a.name.cmp(&b.name))
    });
}

fn build_summary(cards: &[ProjectCard]) -> Summary {
    let mut s = Summary::default();
    for c in cards {
        if c.error.is_some() {
            s.error += 1;
            continue;
        }
        match c.status.as_str() {
            "active" => s.active += 1,
            "paused" => s.paused += 1,
            "done" => s.done += 1,
            _ => {}
        }
    }
    s
}

// ───────────────────────── 对外入口 ─────────────────────────

/// 从指定 registry 路径加载整盘看板数据。core 实现，便于单测注入路径。
pub fn load_board_from(registry_path: &Path) -> Board {
    // registry 不存在 → 返回空看板（不是错误，可能用户还没登记任何项目）
    if !registry_path.exists() {
        return Board {
            projects: vec![],
            summary: Summary::default(),
            registry_error: None,
        };
    }

    let raw = match std::fs::read_to_string(registry_path) {
        Ok(r) => r,
        Err(e) => {
            return Board {
                projects: vec![],
                summary: Summary::default(),
                registry_error: Some(format!("读取 registry 失败: {e}")),
            }
        }
    };

    let registry: Registry = match serde_yaml::from_str(&raw) {
        Ok(r) => r,
        Err(e) => {
            return Board {
                projects: vec![],
                summary: Summary::default(),
                registry_error: Some(format!("registry.yaml 解析失败: {e}")),
            }
        }
    };

    let mut cards: Vec<ProjectCard> = registry.projects.iter().map(parse_entry).collect();
    sort_cards(&mut cards);
    let summary = build_summary(&cards);

    Board {
        projects: cards,
        summary,
        registry_error: None,
    }
}

/// 默认入口：从 ~/.ai-vault/taskboard/registry.yaml 加载。
pub fn load_board() -> Board {
    load_board_from(&default_registry_path())
}

// ───────────────────────── M4：收集需要监听的 PROGRESS.md 路径 ─────────────────────────

/// 读 registry，返回每个已登记项目「展开后的 PROGRESS.md 绝对路径」列表。
/// 仅用于文件监听（watcher.rs）；与 load_board 共用 registry 解析，保证监听集合与看板一致。
/// 防御性：registry 不存在 / 解析失败 → 返回空列表（不报错，由 load_board 那条路负责降级展示）。
/// 注意：返回的路径可能尚不存在（项目刚登记、文件还没建），监听层需自行处理「父目录监听 + 文件未创建」。
pub fn collect_progress_paths_from(registry_path: &Path) -> Vec<PathBuf> {
    if !registry_path.exists() {
        return vec![];
    }
    let raw = match std::fs::read_to_string(registry_path) {
        Ok(r) => r,
        Err(_) => return vec![],
    };
    let registry: Registry = match serde_yaml::from_str(&raw) {
        Ok(r) => r,
        Err(_) => return vec![],
    };
    registry
        .projects
        .iter()
        .map(|e| expand_tilde(&e.path).join(&e.progress_file))
        .collect()
}

// ───────────────────────── 详情页：load_project（M3）─────────────────────────

/// 单项目详情：复用卡片字段 + PROGRESS.md frontmatter 之后的正文原文（markdown 由前端确定性渲染）。
/// body 始终为原文字符串，App 端零智能、不解析正文语义。
/// 阶段分块项（来自 CHANGELOG.md `## 项目阶段` 的 checkbox 列表）。
#[derive(Debug, Serialize, PartialEq)]
pub struct StageItem {
    pub name: String,
    pub desc: String,
    pub done: bool,
}

#[derive(Debug, Serialize)]
pub struct ProjectDetail {
    pub card: ProjectCard,
    /// frontmatter 之后的正文 markdown 原文；文件缺失 / 读失败时为空字符串（防御性）。
    /// 详情页改版后前端不再渲染，保留以兼容其他潜在调用方。
    pub body: String,
    /// INDEX.md `## 项目简介` 块纯文本；缺失为空串
    pub intro: String,
    /// INDEX.md `## 架构图` 下首个 mermaid 块源码；缺失为空串
    pub arch_mermaid: String,
    /// CHANGELOG.md `## 项目阶段` 的阶段分块列表；缺失为空 vec
    pub stages: Vec<StageItem>,
}

/// 按 id 从指定 registry 加载单项目详情。core 实现，便于单测注入路径。
/// 找不到该 id 返回 Err；项目文件缺失 / 损坏不报错，落到 card.error 上由前端降级显示。
pub fn load_project_from(registry_path: &Path, project_id: &str) -> Result<ProjectDetail, String> {
    if !registry_path.exists() {
        return Err("registry 不存在".into());
    }
    let raw =
        std::fs::read_to_string(registry_path).map_err(|e| format!("读取 registry 失败: {e}"))?;
    let registry: Registry =
        serde_yaml::from_str(&raw).map_err(|e| format!("registry.yaml 解析失败: {e}"))?;

    let entry = registry
        .projects
        .iter()
        .find(|e| e.id == project_id)
        .ok_or_else(|| format!("未找到项目: {project_id}"))?;

    let card = parse_entry(entry);
    // 正文读取也要防御性：文件缺失 / 读失败 → 空正文，绝不崩溃
    let body = std::fs::read_to_string(&card.progress_path)
        .map(|c| extract_body(&c))
        .unwrap_or_default();

    // 三件套块：从项目根固定文件名读取，全部防御性（读失败 / 块缺失 → 空，绝不报错）
    let root = std::path::Path::new(&card.path);
    let index_md = std::fs::read_to_string(root.join("INDEX.md")).unwrap_or_default();
    let changelog_md = std::fs::read_to_string(root.join("CHANGELOG.md")).unwrap_or_default();

    let intro = extract_section(&index_md, "项目简介").unwrap_or_default();
    let arch_mermaid = extract_section(&index_md, "架构图")
        .and_then(|s| extract_mermaid(&s))
        .unwrap_or_default();
    let stages = extract_stage_list(&changelog_md);

    Ok(ProjectDetail {
        card,
        body,
        intro,
        arch_mermaid,
        stages,
    })
}

/// 默认入口：从默认 registry 加载单项目详情。
pub fn load_project(project_id: &str) -> Result<ProjectDetail, String> {
    load_project_from(&default_registry_path(), project_id)
}

// ═════════════════════════ 单元测试 ═════════════════════════
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    // 在临时目录造一个项目 + PROGRESS.md，返回 (项目根, 进度文件相对名)
    fn make_project(tmp: &Path, dir: &str, progress_content: Option<&str>) -> PathBuf {
        let root = tmp.join(dir);
        std::fs::create_dir_all(&root).unwrap();
        if let Some(c) = progress_content {
            let mut f = std::fs::File::create(root.join("PROGRESS.md")).unwrap();
            f.write_all(c.as_bytes()).unwrap();
        }
        root
    }

    fn write_registry(tmp: &Path, body: &str) -> PathBuf {
        let p = tmp.join("registry.yaml");
        std::fs::write(&p, body).unwrap();
        p
    }

    // 唯一临时目录（容忍中文路径：目录名里特意放中文）
    fn unique_tmp() -> PathBuf {
        let base = std::env::temp_dir().join(format!(
            "tb_测试_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&base).unwrap();
        base
    }

    // ───── 进度公式 ─────
    #[test]
    fn test_overall_formula_basic() {
        // current_stage=3, stage_progress=60, 4 阶段 → (3-1+0.6)/4 = 0.65 → 65%
        let v = compute_overall("active", 3, 60.0, 4);
        assert!((v - 65.0).abs() < 1e-9, "got {v}");
    }

    #[test]
    fn test_overall_done_forced_100() {
        // done 强制 100，无论阶段数值
        let v = compute_overall("done", 1, 0.0, 4);
        assert_eq!(v, 100.0);
    }

    #[test]
    fn test_overall_first_stage_zero_progress() {
        // current_stage=1, sp=0, 4 阶段 → 0%
        assert_eq!(compute_overall("active", 1, 0.0, 4), 0.0);
    }

    #[test]
    fn test_overall_last_stage_full() {
        // current_stage=4, sp=100, 4 阶段 → (4-1+1)/4 = 1.0 → 100%
        assert_eq!(compute_overall("active", 4, 100.0, 4), 100.0);
    }

    // ───── frontmatter 提取 ─────
    #[test]
    fn test_extract_frontmatter_ok() {
        let c = "---\nproject: x\nstatus: active\n---\n# body\n";
        let fm = extract_frontmatter(c).unwrap();
        assert!(fm.contains("project: x"));
        assert!(!fm.contains("body"));
    }

    #[test]
    fn test_extract_frontmatter_missing() {
        assert!(extract_frontmatter("# 没有 frontmatter\n正文").is_none());
    }

    // ───── 整盘解析：3 正常 + 1 坏 ─────
    #[test]
    fn test_load_board_three_good_one_bad() {
        let tmp = unique_tmp();

        let good1 = "---\nproject: voice-pipeline\nstatus: active\nstages:\n  - 需求与架构\n  - ASR 接入\n  - barge-in 状态机\n  - 联调打包\ncurrent_stage: 3\nstage_progress: 60\nnext:\n  - 打断信号去抖逻辑\n  - 写集成测试\n  - 第三条不该显示\nblocked_by: []\nupdated: 2026-06-13\n---\n# 阶段记录\n";
        let good2 = "---\nproject: alpha\nstatus: done\nstages:\n  - a\n  - b\ncurrent_stage: 2\nstage_progress: 50\nupdated: 2026-06-10\n---\n正文\n";
        let good3 = "---\nproject: beta\nstatus: paused\nstages:\n  - 一\n  - 二\n  - 三\ncurrent_stage: 1\nupdated: 2026-06-01\n---\n正文\n"; // stage_progress 缺省→0
        let bad = "---\nproject: broken\nstatus: active\nstages: []\ncurrent_stage: 5\nupdated: oops\n---\n正文\n"; // stages 空 + 越界

        let p1 = make_project(&tmp, "voice", Some(good1));
        let p2 = make_project(&tmp, "alpha", Some(good2));
        let p3 = make_project(&tmp, "beta", Some(good3));
        let p4 = make_project(&tmp, "broken", Some(bad));

        let reg = format!(
            "version: 1\nprojects:\n  - id: voice-pipeline\n    name: 语音管线\n    path: {}\n    progress_file: PROGRESS.md\n    pinned: false\n    added: 2026-06-13\n  - id: alpha\n    name: Alpha\n    path: {}\n    pinned: false\n  - id: beta\n    name: Beta\n    path: {}\n    pinned: true\n  - id: broken\n    name: 坏项目\n    path: {}\n",
            p1.display(), p2.display(), p3.display(), p4.display()
        );
        let regp = write_registry(&tmp, &reg);

        let board = load_board_from(&regp);
        assert!(board.registry_error.is_none());
        assert_eq!(board.projects.len(), 4);

        // 汇总：active=1(voice), paused=1(beta), done=1(alpha), error=1(broken)
        assert_eq!(board.summary.active, 1);
        assert_eq!(board.summary.paused, 1);
        assert_eq!(board.summary.done, 1);
        assert_eq!(board.summary.error, 1);

        // 排序：pinned 的 beta 应在最前；done 的 alpha 应在末尾
        assert_eq!(board.projects.first().unwrap().id, "beta", "pinned 应置顶");
        assert_eq!(board.projects.last().unwrap().id, "alpha", "done 应排末尾");

        // voice 进度 = 65%
        let voice = board.projects.iter().find(|c| c.id == "voice-pipeline").unwrap();
        assert!(voice.error.is_none());
        assert!((voice.overall_progress - 65.0).abs() < 1e-9);
        assert_eq!(voice.current_stage, 3);
        assert_eq!(voice.stages.len(), 4);

        // broken 必须是 format 错误且不崩溃
        let broken = board.projects.iter().find(|c| c.id == "broken").unwrap();
        assert_eq!(broken.error.as_ref().unwrap().kind, "format");
    }

    // ───── 详情页：正文提取 + load_project ─────
    #[test]
    fn test_extract_body_after_frontmatter() {
        let c = "---\nproject: x\nstatus: active\n---\n## 阶段记录\n\n正文内容\n";
        assert_eq!(extract_body(c), "## 阶段记录\n\n正文内容\n");
    }

    #[test]
    fn test_extract_body_no_frontmatter_returns_full() {
        let c = "# 没有 frontmatter\n正文";
        assert_eq!(extract_body(c), c);
    }

    #[test]
    fn test_load_project_returns_body() {
        let tmp = unique_tmp();
        let good = "---\nproject: voice-pipeline\nstatus: active\nstages:\n  - a\n  - b\ncurrent_stage: 2\nstage_progress: 30\nnext:\n  - n1\n  - n2\n  - n3\nblocked_by:\n  - 等待上游接口\nupdated: 2026-06-13\n---\n## 阶段记录\n\n- 干了点活\n";
        let p1 = make_project(&tmp, "voice", Some(good));
        let reg = format!(
            "version: 1\nprojects:\n  - id: voice-pipeline\n    name: 语音管线\n    path: {}\n",
            p1.display()
        );
        let regp = write_registry(&tmp, &reg);
        let detail = load_project_from(&regp, "voice-pipeline").unwrap();
        assert!(detail.card.error.is_none());
        assert_eq!(detail.card.next.len(), 3); // 详情页要完整 next（不止两条）
        assert_eq!(detail.card.blocked_by, vec!["等待上游接口".to_string()]);
        assert!(detail.body.contains("干了点活"));
        // 未知 id 报错
        assert!(load_project_from(&regp, "ghost").is_err());
    }

    #[test]
    fn test_load_project_missing_file_empty_body_no_panic() {
        let tmp = unique_tmp();
        let root = make_project(&tmp, "noprog", None);
        let reg = format!(
            "version: 1\nprojects:\n  - id: noprog\n    path: {}\n",
            root.display()
        );
        let regp = write_registry(&tmp, &reg);
        let detail = load_project_from(&regp, "noprog").unwrap();
        assert_eq!(detail.card.error.as_ref().unwrap().kind, "missing");
        assert_eq!(detail.body, ""); // 正文读取防御性，缺失时空串
        // 三件套块缺失时也都是空（防御性降级）
        assert_eq!(detail.intro, "");
        assert_eq!(detail.arch_mermaid, "");
        assert!(detail.stages.is_empty());
    }

    // ───── 三件套块提取 ─────
    #[test]
    fn test_extract_section_basic() {
        let md = "# T\n\n## 项目简介\n这是一句简介。\n第二行。\n\n## 架构图\n图内容\n";
        assert_eq!(
            extract_section(md, "项目简介").unwrap(),
            "这是一句简介。\n第二行。"
        );
        assert_eq!(extract_section(md, "架构图").unwrap(), "图内容");
        assert!(extract_section(md, "不存在").is_none());
    }

    #[test]
    fn test_extract_mermaid_ok_and_none() {
        let sec = "前言\n```mermaid\nflowchart TD\n  A --> B\n```\n后语";
        assert_eq!(extract_mermaid(sec).unwrap(), "flowchart TD\n  A --> B");
        assert!(extract_mermaid("没有代码块").is_none());
    }

    #[test]
    fn test_extract_stage_list_parses_checkbox() {
        let md = "## 项目阶段\n- [x] 需求与架构 — 明确方案\n- [ ] provider 注入 — 专属 home\n- [X] 备份机制\n非列表行\n\n## 2026-06-13 #feat scope:x - 变更流水\n";
        let items = extract_stage_list(md);
        assert_eq!(items.len(), 3);
        assert_eq!(items[0], StageItem { name: "需求与架构".into(), desc: "明确方案".into(), done: true });
        assert_eq!(items[1], StageItem { name: "provider 注入".into(), desc: "专属 home".into(), done: false });
        // 无破折号描述 → desc 空；大写 X 也算完成
        assert_eq!(items[2], StageItem { name: "备份机制".into(), desc: "".into(), done: true });
    }

    #[test]
    fn test_extract_stage_list_no_block_empty() {
        // 没有「## 项目阶段」块 → 空（不误抓其他列表）
        assert!(extract_stage_list("## 别的\n- [x] 不该被抓").is_empty());
    }

    #[test]
    fn test_load_project_reads_trio_blocks() {
        let tmp = unique_tmp();
        let good = "---\nproject: voice\nstatus: active\nstages:\n  - a\ncurrent_stage: 1\nupdated: 2026-06-13\n---\n正文\n";
        let root = make_project(&tmp, "voice", Some(good));
        // 写 INDEX.md / CHANGELOG.md 到项目根
        std::fs::write(
            root.join("INDEX.md"),
            "## 项目简介\n一句话简介。\n\n## 架构图\n```mermaid\nflowchart TD\n  A --> B\n```\n",
        )
        .unwrap();
        std::fs::write(
            root.join("CHANGELOG.md"),
            "## 项目阶段\n- [x] 阶段一 — 完成了\n- [ ] 阶段二\n\n## 2026-06-13 #feat - 流水\n",
        )
        .unwrap();
        let reg = format!(
            "version: 1\nprojects:\n  - id: voice\n    name: V\n    path: {}\n",
            root.display()
        );
        let regp = write_registry(&tmp, &reg);
        let d = load_project_from(&regp, "voice").unwrap();
        assert_eq!(d.intro, "一句话简介。");
        assert_eq!(d.arch_mermaid, "flowchart TD\n  A --> B");
        assert_eq!(d.stages.len(), 2);
        assert!(d.stages[0].done);
        assert!(!d.stages[1].done);
    }

    // ───── 防御性：各类坏数据 ─────
    #[test]
    fn test_missing_progress_file() {
        let tmp = unique_tmp();
        let root = make_project(&tmp, "noprog", None); // 不写 PROGRESS.md
        let reg = format!("version: 1\nprojects:\n  - id: noprog\n    name: NoProg\n    path: {}\n", root.display());
        let regp = write_registry(&tmp, &reg);
        let board = load_board_from(&regp);
        let c = &board.projects[0];
        assert_eq!(c.error.as_ref().unwrap().kind, "missing");
    }

    #[test]
    fn test_missing_project_path() {
        let tmp = unique_tmp();
        let reg = format!(
            "version: 1\nprojects:\n  - id: ghost\n    name: Ghost\n    path: {}/不存在的目录\n",
            tmp.display()
        );
        let regp = write_registry(&tmp, &reg);
        let board = load_board_from(&regp);
        assert_eq!(board.projects[0].error.as_ref().unwrap().kind, "missing");
    }

    #[test]
    fn test_no_frontmatter_is_format_error() {
        let tmp = unique_tmp();
        let root = make_project(&tmp, "nofm", Some("# 只有正文没有 frontmatter\n"));
        let reg = format!("version: 1\nprojects:\n  - id: nofm\n    path: {}\n", root.display());
        let regp = write_registry(&tmp, &reg);
        let board = load_board_from(&regp);
        assert_eq!(board.projects[0].error.as_ref().unwrap().kind, "format");
    }

    #[test]
    fn test_broken_yaml_is_format_error() {
        let tmp = unique_tmp();
        // 故意写坏 YAML：缩进/冒号混乱
        let bad = "---\nproject: x\nstages:\n  - a\n   - b   bad indent: : :\ncurrent_stage: [unterminated\n---\n";
        let root = make_project(&tmp, "badyaml", Some(bad));
        let reg = format!("version: 1\nprojects:\n  - id: badyaml\n    path: {}\n", root.display());
        let regp = write_registry(&tmp, &reg);
        let board = load_board_from(&regp);
        // 不崩溃，且被标记 format
        assert_eq!(board.projects[0].error.as_ref().unwrap().kind, "format");
    }

    #[test]
    fn test_unknown_fields_ignored() {
        let tmp = unique_tmp();
        // schema 外字段 foo/bar 应被忽略，不影响正常解析（向前兼容）
        let c = "---\nproject: fwd\nstatus: active\nstages:\n  - a\n  - b\ncurrent_stage: 1\nstage_progress: 50\nfoo: 123\nbar:\n  nested: true\nupdated: 2026-06-13\n---\n";
        let root = make_project(&tmp, "fwd", Some(c));
        let reg = format!("version: 1\nprojects:\n  - id: fwd\n    path: {}\n", root.display());
        let regp = write_registry(&tmp, &reg);
        let board = load_board_from(&regp);
        let card = &board.projects[0];
        assert!(card.error.is_none(), "未知字段不应导致错误");
        assert!((card.overall_progress - 25.0).abs() < 1e-9); // (1-1+0.5)/2=0.25
    }

    #[test]
    fn test_empty_registry_returns_empty_board() {
        let tmp = unique_tmp();
        let regp = write_registry(&tmp, "version: 1\nprojects: []\n");
        let board = load_board_from(&regp);
        assert!(board.projects.is_empty());
        assert!(board.registry_error.is_none());
    }

    #[test]
    fn test_nonexistent_registry_is_empty_not_error() {
        let tmp = unique_tmp();
        let board = load_board_from(&tmp.join("nope.yaml"));
        assert!(board.projects.is_empty());
        assert!(board.registry_error.is_none());
    }

    #[test]
    fn test_broken_registry_yaml_sets_registry_error() {
        let tmp = unique_tmp();
        let regp = write_registry(&tmp, "version: 1\nprojects: [ this is : : broken\n");
        let board = load_board_from(&regp);
        assert!(board.registry_error.is_some());
        assert!(board.projects.is_empty());
    }

    // ───── M4：监听路径收集 ─────
    #[test]
    fn test_collect_progress_paths() {
        let tmp = unique_tmp();
        let p1 = make_project(&tmp, "voice", Some("---\nproject: v\nstages:\n  - a\ncurrent_stage: 1\n---\n"));
        let p2 = make_project(&tmp, "alpha", None); // 文件还没建，仍应收集其预期路径
        let reg = format!(
            "version: 1\nprojects:\n  - id: voice\n    path: {}\n  - id: alpha\n    path: {}\n    progress_file: custom.md\n",
            p1.display(), p2.display()
        );
        let regp = write_registry(&tmp, &reg);
        let paths = collect_progress_paths_from(&regp);
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0], p1.join("PROGRESS.md"));
        assert_eq!(paths[1], p2.join("custom.md")); // 自定义 progress_file 也要展开
    }

    #[test]
    fn test_collect_progress_paths_nonexistent_registry_empty() {
        let tmp = unique_tmp();
        assert!(collect_progress_paths_from(&tmp.join("nope.yaml")).is_empty());
    }

    #[test]
    fn test_expand_tilde() {
        // 注入 home，不改写全局 HOME，避免与并发测试相互污染
        let home = Some(PathBuf::from("/Users/test"));
        assert_eq!(expand_tilde_with("~", home.clone()), PathBuf::from("/Users/test"));
        assert_eq!(expand_tilde_with("~/.ai-vault", home.clone()), PathBuf::from("/Users/test/.ai-vault"));
        assert_eq!(expand_tilde_with("/abs/路径/中文", home.clone()), PathBuf::from("/abs/路径/中文"));
        // home 缺失时 ~ 原样保留，不 panic
        assert_eq!(expand_tilde_with("~/x", None), PathBuf::from("~/x"));
    }
}
