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

// ─────────── 三件套字段提取（看板字段新来源，替代 PROGRESS.md）───────────

/// AGENTS.md frontmatter 里的看板元信息（status + desc）。
struct AgentsMeta {
    status: Option<String>,
    desc: String,
}

/// 解析 AGENTS.md frontmatter 取 status / desc（卡片级元信息）。
/// 文件缺失 / 无 frontmatter / YAML 损坏 → status=None、desc 空（防御性）。
fn parse_agents_meta(agents_md: &str) -> AgentsMeta {
    let fm = match extract_frontmatter(agents_md) {
        Some(f) => f,
        None => return AgentsMeta { status: None, desc: String::new() },
    };
    let val: serde_yaml::Value = match serde_yaml::from_str(fm) {
        Ok(v) => v,
        Err(_) => return AgentsMeta { status: None, desc: String::new() },
    };
    let get = |k: &str| {
        val.get(k)
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    AgentsMeta {
        status: get("status"),
        desc: get("desc").unwrap_or_default(),
    }
}

/// 由阶段表 checkbox 算整体进度：完成数 / 总数 ×100。
/// status=done 强制 100；空表返回 0。
fn compute_progress_from_stages(stages: &[StageItem], status: &str) -> f64 {
    if status == "done" {
        return 100.0;
    }
    if stages.is_empty() {
        return 0.0;
    }
    let done = stages.iter().filter(|s| s.done).count();
    (done as f64 / stages.len() as f64 * 100.0).clamp(0.0, 100.0)
}

/// 从 INDEX.md `## 当前接力点 (Handoff)` 区解析 next 与 blocked_by。
/// Handoff 段拆为 `### 概述` / `### 明细` 两个子段：**App 只读「概述」段**，
/// 明细段是给人/agent 看的长说明，App 完全不碰。
/// 约定：概述段内每个非空内容行即一条接力项；以 `⚠` 或 `阻塞` 开头的归 blocked_by，
/// 其余归 next。列表前缀 `- `/`* ` 可选（有就剥掉），首尾 `**` 加粗标记一并剥掉——
/// 概述推荐写法是「纯文本加粗行」（`**下一步**`），旧的 `- xxx` 列表行也兼容。
/// 向后兼容：若整段没有 `### 概述` 子标题，则把第一个 `### ` 之前的内容（无子标题
/// 时即整段）当作概述解析——兼容旧的单段 Handoff 写法。
/// 区缺失 / 为空 / 整段是 HTML 注释占位 / 引用行 `>` → 跳过（防御性）。
fn extract_handoff(index_md: &str) -> (Vec<String>, Vec<String>) {
    // 标题字面可能带 "(Handoff)" 后缀，宽松匹配 "## 当前接力点"
    let section = match extract_section_prefix(index_md, "当前接力点") {
        Some(s) => s,
        None => return (vec![], vec![]),
    };
    let overview = extract_handoff_overview(&section);
    let mut next = vec![];
    let mut blocked = vec![];
    for raw in overview.lines() {
        let line = raw.trim();
        // 跳过 HTML 注释占位与引用行（说明文字，非接力项）
        if line.starts_with("<!--") || line.starts_with("-->") || line.starts_with('>') || line.is_empty() {
            continue;
        }
        // 列表前缀可选；再剥掉首尾加粗标记
        let item = line
            .strip_prefix("- ")
            .or_else(|| line.strip_prefix("* "))
            .unwrap_or(line)
            .trim()
            .trim_start_matches("**")
            .trim_end_matches("**")
            .trim();
        if item.is_empty() {
            continue;
        }
        if item.starts_with('⚠') || item.starts_with("阻塞") {
            // 去掉前缀符号与「阻塞：」标记
            let b = item
                .trim_start_matches('⚠')
                .trim_start()
                .trim_start_matches("阻塞")
                .trim_start_matches(['：', ':'])
                .trim();
            if !b.is_empty() {
                blocked.push(b.to_string());
            }
        } else {
            next.push(item.to_string());
        }
    }
    (next, blocked)
}

/// 从 Handoff 整段里切出「概述」子段供 App 解析。
/// - 有 `### 概述` 子标题：返回它到下一个 `### ` 之间的内容（明细被丢弃）。
/// - 无 `### 概述`：返回第一个 `### ` 之前的内容（兼容旧单段写法；无任何子标题则即整段）。
fn extract_handoff_overview(section: &str) -> String {
    let is_sub_heading = |l: &str| l.trim_start().starts_with("### ");
    // 宽松匹配「### 概述」（容忍标题后缀）
    let is_overview_heading =
        |l: &str| is_sub_heading(l) && l.trim_start().trim_start_matches('#').trim_start().starts_with("概述");
    let has_overview = section.lines().any(is_overview_heading);

    let mut in_overview = false;
    let mut collected = vec![];
    for line in section.lines() {
        if is_sub_heading(line) {
            // 有显式概述段：只在概述子段内收集；明细等其他 ### 一律跳出概述
            in_overview = is_overview_heading(line);
            continue; // 子标题本身不收集
        }
        // 有概述段时只收集概述内的行；无概述段时（旧写法）收集第一个 ### 之前的行，
        // 此处不会到达 ### 后的行——上面的 is_sub_heading 分支已处理并将 in_overview 置 false。
        if has_overview {
            if in_overview {
                collected.push(line);
            }
        } else {
            collected.push(line);
        }
    }
    collected.join("\n")
}

/// extract_section 的宽松版：标题以 `## <prefix>` 开头即匹配（容忍标题后缀）。
fn extract_section_prefix(md: &str, prefix: &str) -> Option<String> {
    let target = format!("## {prefix}");
    let mut lines = md.lines();
    loop {
        let line = lines.next()?;
        if line.trim().starts_with(&target) {
            break;
        }
    }
    let mut buf = String::new();
    for line in lines {
        if line.trim_start().starts_with("## ") {
            break;
        }
        buf.push_str(line);
        buf.push('\n');
    }
    let s = buf.trim().to_string();
    if s.is_empty() { None } else { Some(s) }
}

/// 抓 CHANGELOG.md 里最新（最靠上）的 `## YYYY-MM-DD` 流水条目日期。
/// 找不到 → 空串。
fn extract_changelog_date(changelog_md: &str) -> String {
    for line in changelog_md.lines() {
        let t = line.trim();
        // 形如 "## 2026-06-14 #feat ..." —— 取标题后首个 ISO 日期
        if let Some(rest) = t.strip_prefix("## ") {
            let token = rest.split_whitespace().next().unwrap_or("");
            if is_iso_date(token) {
                return token.to_string();
            }
        }
    }
    String::new()
}

/// 粗校验 YYYY-MM-DD（不校验月份天数合法性，足够过滤标题）。
fn is_iso_date(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b[..4].iter().all(u8::is_ascii_digit)
        && b[5..7].iter().all(u8::is_ascii_digit)
        && b[8..].iter().all(u8::is_ascii_digit)
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
                stages: c.stage_names,
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

    // 1) 项目路径必须存在
    if !project_root.exists() {
        return make_card(Some(ParseError::missing(format!("项目路径不存在: {}", project_root.display()))), None);
    }

    // 2) 读三件套（各自防御性：读失败 → 空串，绝不 panic）
    let agents_md = std::fs::read_to_string(project_root.join("AGENTS.md")).unwrap_or_default();
    let index_md = std::fs::read_to_string(project_root.join("INDEX.md")).unwrap_or_default();
    let changelog_md = std::fs::read_to_string(project_root.join("CHANGELOG.md")).unwrap_or_default();

    let meta = parse_agents_meta(&agents_md);
    let stages = extract_stage_list(&changelog_md);

    // 3) 判断三件套是否提供了看板信息：有 status frontmatter 或有阶段表即算"已接入"
    let has_trio_data = meta.status.is_some() || !stages.is_empty();

    if has_trio_data {
        let status = meta.status.unwrap_or_else(|| "active".into());
        let (next, blocked_by) = extract_handoff(&index_md);
        let overall = compute_progress_from_stages(&stages, &status);
        let done_count = stages.iter().filter(|s| s.done).count();
        let updated = extract_changelog_date(&changelog_md);
        return make_card(
            None,
            Some(FmComputed {
                desc: meta.desc,
                status,
                // current_stage 兼容字段：指向首个未完成阶段（1-based）
                current_stage: (done_count + 1) as i64,
                stage_progress: 0.0, // 阶段内细粒度进度已由 checkbox 取代
                overall_progress: overall,
                stage_names: stages.iter().map(|s| s.name.clone()).collect(),
                next,
                blocked_by,
                updated,
            }),
        );
    }

    // 4) 三件套无看板信息 → 降级「未接入看板」卡片（PROGRESS.md 已退役，不再回退）
    make_card(
        Some(ParseError::missing(
            "未接入看板：请在 AGENTS.md frontmatter 加 status，CHANGELOG.md 加「## 项目阶段」（可用 /outkanban 一键生成）",
        )),
        None,
    )
}

// 解析成功时的中间结果
struct FmComputed {
    desc: String,
    status: String,
    stage_names: Vec<String>,
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
/// 收集所有需要监听的看板数据文件：每个项目的三件套（AGENTS/INDEX/CHANGELOG）
/// + 兼容期保留 PROGRESS.md。文件改动 → 看板刷新。
pub fn collect_watched_files_from(registry_path: &Path) -> Vec<PathBuf> {
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
    let mut out = vec![];
    for e in &registry.projects {
        let root = expand_tilde(&e.path);
        out.push(root.join("AGENTS.md"));
        out.push(root.join("INDEX.md"));
        out.push(root.join("CHANGELOG.md"));
        out.push(root.join(&e.progress_file)); // 兼容期：未迁移项目仍看 PROGRESS.md
    }
    out
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

    /// 建一个「已接入看板」的三件套项目：AGENTS(status/desc) + CHANGELOG(阶段表) + INDEX(Handoff)。
    fn make_trio_project(tmp: &Path, dir: &str, agents: &str, changelog: &str, index: &str) -> PathBuf {
        let root = tmp.join(dir);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("AGENTS.md"), agents).unwrap();
        std::fs::write(root.join("CHANGELOG.md"), changelog).unwrap();
        std::fs::write(root.join("INDEX.md"), index).unwrap();
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

        // voice：active，4 阶段勾 3 → 75%
        let p1 = make_trio_project(
            &tmp, "voice",
            "---\nstatus: active\ndesc: 语音管线\n---\n",
            "## 项目阶段\n- [x] 需求与架构\n- [x] ASR 接入\n- [x] barge-in 状态机\n- [ ] 联调打包\n\n## 2026-06-13 #feat - x\n",
            "## 当前接力点 (Handoff)\n- 打断信号去抖逻辑\n",
        );
        // alpha：done
        let p2 = make_trio_project(
            &tmp, "alpha",
            "---\nstatus: done\n---\n",
            "## 项目阶段\n- [x] a\n- [x] b\n\n## 2026-06-10 #feat - x\n",
            "",
        );
        // beta：paused，pinned
        let p3 = make_trio_project(
            &tmp, "beta",
            "---\nstatus: paused\n---\n",
            "## 项目阶段\n- [ ] 一\n- [ ] 二\n- [ ] 三\n\n## 2026-06-01 #chore - x\n",
            "",
        );
        // broken：只有空目录，无任何三件套看板信息 → missing 降级
        let p4 = make_project(&tmp, "broken", None);

        let reg = format!(
            "version: 1\nprojects:\n  - id: voice-pipeline\n    name: 语音管线\n    path: {}\n    pinned: false\n  - id: alpha\n    name: Alpha\n    path: {}\n    pinned: false\n  - id: beta\n    name: Beta\n    path: {}\n    pinned: true\n  - id: broken\n    name: 坏项目\n    path: {}\n",
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

        // voice 进度 = 3/4 = 75%
        let voice = board.projects.iter().find(|c| c.id == "voice-pipeline").unwrap();
        assert!(voice.error.is_none());
        assert!((voice.overall_progress - 75.0).abs() < 1e-9);
        assert_eq!(voice.stages.len(), 4);
        assert_eq!(voice.next, vec!["打断信号去抖逻辑".to_string()]);

        // broken 必须降级（missing）且不崩溃
        let broken = board.projects.iter().find(|c| c.id == "broken").unwrap();
        assert_eq!(broken.error.as_ref().unwrap().kind, "missing");
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
    fn test_load_project_detail_from_trio() {
        let tmp = unique_tmp();
        let p1 = make_trio_project(
            &tmp, "voice",
            "---\nstatus: active\ndesc: 语音\n---\n",
            "## 项目阶段\n- [x] a\n- [ ] b\n\n## 2026-06-13 #feat - x\n",
            "## 当前接力点 (Handoff)\n- n1\n- n2\n- n3\n- ⚠ 阻塞：等待上游接口\n",
        );
        let reg = format!(
            "version: 1\nprojects:\n  - id: voice-pipeline\n    name: 语音管线\n    path: {}\n",
            p1.display()
        );
        let regp = write_registry(&tmp, &reg);
        let detail = load_project_from(&regp, "voice-pipeline").unwrap();
        assert!(detail.card.error.is_none());
        assert_eq!(detail.card.next.len(), 3); // 详情页完整 next
        assert_eq!(detail.card.blocked_by, vec!["等待上游接口".to_string()]);
        assert_eq!(detail.stages.len(), 2);
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

    // ───── 三件套新字段来源 ─────
    #[test]
    fn test_parse_agents_meta() {
        let m = parse_agents_meta("---\ntrio: standard-v2\nstatus: paused\ndesc: 一句话描述\n---\n# 正文");
        assert_eq!(m.status.as_deref(), Some("paused"));
        assert_eq!(m.desc, "一句话描述");
        // 无 frontmatter → 都空
        let m2 = parse_agents_meta("# 没有 frontmatter");
        assert!(m2.status.is_none());
        assert_eq!(m2.desc, "");
    }

    #[test]
    fn test_compute_progress_from_stages() {
        let s = |done: bool| StageItem { name: "x".into(), desc: "".into(), done };
        // 2/4 = 50%
        assert_eq!(compute_progress_from_stages(&[s(true), s(true), s(false), s(false)], "active"), 50.0);
        // done 强制 100
        assert_eq!(compute_progress_from_stages(&[s(false)], "done"), 100.0);
        // 空表 → 0
        assert_eq!(compute_progress_from_stages(&[], "active"), 0.0);
    }

    #[test]
    fn test_extract_handoff_next_and_blocked() {
        // 旧单段写法（无 ### 概述/明细 子标题）→ 向后兼容，整段当概述
        let idx = "# T\n\n## 当前接力点 (Handoff)\n- 写集成测试\n- ⚠ 阻塞：等待上游接口\n- 打包发布\n\n## 项目定位\nxxx";
        let (next, blocked) = extract_handoff(idx);
        assert_eq!(next, vec!["写集成测试".to_string(), "打包发布".to_string()]);
        assert_eq!(blocked, vec!["等待上游接口".to_string()]);
        // 区缺失 → 都空
        let (n2, b2) = extract_handoff("# 无 handoff");
        assert!(n2.is_empty() && b2.is_empty());
    }

    #[test]
    fn test_extract_handoff_overview_only_明细_ignored() {
        // 新两段写法：App 只取「概述」段的列表，「明细」段整段被忽略
        let idx = "# T\n\n## 当前接力点 (Handoff)\n\n### 概述\n- 打包发布\n- ⚠ 阻塞：等待签名证书\n\n### 明细\n跑 ./scripts/install.sh 构建 .app。\n- 这行明细里的列表项不该被当成 next\n- ⚠ 这行也不该进 blocked_by\n\n## 项目定位\nxxx";
        let (next, blocked) = extract_handoff(idx);
        assert_eq!(next, vec!["打包发布".to_string()]);
        assert_eq!(blocked, vec!["等待签名证书".to_string()]);
    }

    #[test]
    fn test_extract_handoff_明细_before_概述_still_isolated() {
        // 明细在前、概述在后：仍只取概述段
        let idx = "## 当前接力点 (Handoff)\n\n### 明细\n- 假 next\n\n### 概述\n- 真 next\n";
        let (next, _) = extract_handoff(idx);
        assert_eq!(next, vec!["真 next".to_string()]);
    }

    #[test]
    fn test_extract_handoff_纯文本加粗_无列表点() {
        // 新推荐写法：概述是纯文本加粗行（无 - 前缀），引用行被跳过
        let idx = "## 当前接力点 (Handoff)\n\n> 只保留最新一条。\n\n### 概述\n**跑 ./scripts/install.sh 正式打包发布**\n**⚠ 阻塞：等待签名证书**\n\n### 明细\n背景说明若干。\n";
        let (next, blocked) = extract_handoff(idx);
        assert_eq!(next, vec!["跑 ./scripts/install.sh 正式打包发布".to_string()]);
        assert_eq!(blocked, vec!["等待签名证书".to_string()]);
    }

    #[test]
    fn test_extract_handoff_skips_html_comment() {
        let idx = "## 当前接力点 (Handoff)\n<!-- 当前没有接力点。 -->\n";
        let (next, blocked) = extract_handoff(idx);
        assert!(next.is_empty() && blocked.is_empty());
    }

    #[test]
    fn test_extract_changelog_date() {
        let cl = "# CHANGELOG\n\n## 格式规范\n\n## 2026-06-14 #feat scope:x - 主题\n- Why: ...\n## 2026-06-01 #fix - 旧的\n";
        assert_eq!(extract_changelog_date(cl), "2026-06-14"); // 取最靠上的日期条目
        assert_eq!(extract_changelog_date("# 无日期条目\n## 格式规范"), "");
    }

    #[test]
    fn test_parse_entry_prefers_trio_over_progress() {
        let tmp = unique_tmp();
        // 同时有 PROGRESS（旧）和三件套（新）：应优先三件套
        let root = make_project(&tmp, "v", Some("---\nproject: v\nstatus: paused\nstages:\n  - 老阶段\ncurrent_stage: 1\n---\n"));
        std::fs::write(root.join("AGENTS.md"), "---\nstatus: active\ndesc: 新描述\n---\n").unwrap();
        std::fs::write(root.join("CHANGELOG.md"), "## 项目阶段\n- [x] 一 — d1\n- [ ] 二\n\n## 2026-06-14 #feat - x\n").unwrap();
        std::fs::write(root.join("INDEX.md"), "## 当前接力点 (Handoff)\n- 做二\n").unwrap();
        let reg = format!("version: 1\nprojects:\n  - id: v\n    name: V\n    path: {}\n", root.display());
        let regp = write_registry(&tmp, &reg);
        let board = load_board_from(&regp);
        let c = &board.projects[0];
        assert!(c.error.is_none());
        assert_eq!(c.status, "active");          // 来自 AGENTS，不是 PROGRESS 的 paused
        assert_eq!(c.desc, "新描述");
        assert_eq!(c.overall_progress, 50.0);    // 1/2 checkbox
        assert_eq!(c.next, vec!["做二".to_string()]);
        assert_eq!(c.updated, "2026-06-14");
    }

    #[test]
    fn test_load_project_reads_trio_blocks() {
        let tmp = unique_tmp();
        let root = make_trio_project(
            &tmp, "voice",
            "---\nstatus: active\n---\n",
            "## 项目阶段\n- [x] 阶段一 — 完成了\n- [ ] 阶段二\n\n## 2026-06-13 #feat - 流水\n",
            "## 项目简介\n一句话简介。\n\n## 架构图\n```mermaid\nflowchart TD\n  A --> B\n```\n",
        );
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
    fn test_unconfigured_project_is_missing() {
        // 项目目录在，但无 AGENTS status / 无 CHANGELOG 阶段表 → 「未接入看板」missing
        let tmp = unique_tmp();
        let root = make_project(&tmp, "noprog", None);
        let reg = format!("version: 1\nprojects:\n  - id: noprog\n    name: NoProg\n    path: {}\n", root.display());
        let regp = write_registry(&tmp, &reg);
        let board = load_board_from(&regp);
        assert_eq!(board.projects[0].error.as_ref().unwrap().kind, "missing");
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
    fn test_broken_agents_frontmatter_no_panic() {
        // AGENTS frontmatter 损坏但有阶段表 → 仍按阶段表渲染，status 缺省 active，不崩
        let tmp = unique_tmp();
        let root = make_trio_project(
            &tmp, "bad",
            "---\nstatus: : : broken\n  bad indent\n---\n",
            "## 项目阶段\n- [x] a\n- [ ] b\n\n## 2026-06-13 #feat - x\n",
            "",
        );
        let reg = format!("version: 1\nprojects:\n  - id: bad\n    path: {}\n", root.display());
        let regp = write_registry(&tmp, &reg);
        let board = load_board_from(&regp);
        let c = &board.projects[0];
        assert!(c.error.is_none(), "frontmatter 损坏不应崩，按阶段表降级渲染");
        assert_eq!(c.status, "active"); // frontmatter 解析失败 → 缺省 active
        assert!((c.overall_progress - 50.0).abs() < 1e-9);
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
    fn test_collect_watched_files() {
        let tmp = unique_tmp();
        let p1 = make_project(&tmp, "voice", Some("---\nproject: v\nstages:\n  - a\ncurrent_stage: 1\n---\n"));
        let p2 = make_project(&tmp, "alpha", None);
        let reg = format!(
            "version: 1\nprojects:\n  - id: voice\n    path: {}\n  - id: alpha\n    path: {}\n    progress_file: custom.md\n",
            p1.display(), p2.display()
        );
        let regp = write_registry(&tmp, &reg);
        let paths = collect_watched_files_from(&regp);
        // 每项目 4 个监听文件：AGENTS / INDEX / CHANGELOG / progress_file
        assert_eq!(paths.len(), 8);
        assert!(paths.contains(&p1.join("AGENTS.md")));
        assert!(paths.contains(&p1.join("INDEX.md")));
        assert!(paths.contains(&p1.join("CHANGELOG.md")));
        assert!(paths.contains(&p1.join("PROGRESS.md")));
        assert!(paths.contains(&p2.join("custom.md"))); // 自定义 progress_file 也展开
    }

    #[test]
    fn test_collect_watched_files_nonexistent_registry_empty() {
        let tmp = unique_tmp();
        assert!(collect_watched_files_from(&tmp.join("nope.yaml")).is_empty());
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

