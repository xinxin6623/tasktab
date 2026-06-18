// sync.rs —— 本地 git 同步状态 + 远端看板拉取（「设备间同步」功能的 App 侧）
//
// 中文说明（重要逻辑 / 架构边界）：
//   新「设备间同步」方案下，服务器从 GitHub 聚合看板，App 不再推送。App 需要回答一个问题：
//   「我本机的项目，改动 push 到 GitHub 了吗？」——即本地 HEAD 与服务器拉到的 commit 是否一致。
//   本模块提供两件事：
//     1. local_sync()        ：对每个已登记项目跑 git，拿本地 HEAD 短 SHA + 工作区是否脏。
//        纯读 git 元信息（rev-parse / status），不改任何东西，不触碰项目文件内容。
//     2. fetch_remote_board()：GET 服务器 /board.json，拿每个项目服务器侧的 commit + generated_at。
//        这是 App 端「零网络」铁律的【受控例外】：仅【只读拉取】自有服务器的聚合结果，
//        不上传、不调 LLM。受 TB_BOARD_URL 开关，未配置则返回 None（功能关闭、行为不变）。
//   前端把「本地 HEAD」与「服务器 commit」比对，算出每张卡片的同步徽章
//   （已同步 / 本地领先待 push / 落后待 pull / 脏工作区）。比对逻辑在前端，App 端保持零智能。
//
// 配置（环境变量）：
//   TB_BOARD_URL  服务器 board.json 地址，如 https://kanban.alphaxbot.xyz/board.json
//                 未设置则远端同步功能关闭（local_sync 仍可用，只是无对比基准）。

use std::process::Command;
use std::time::Duration;

use serde::Serialize;

use crate::board;

const BOARD_URL_ENV: &str = "TB_BOARD_URL";

/// 单项目的本地 git 状态。字段都按「拿不到就给空/false」防御性填充，绝不报错。
#[derive(Debug, Serialize, Clone)]
pub struct LocalGitStatus {
    pub id: String,
    /// 本地 HEAD 完整 SHA；非 git 仓库 / 取不到 → 空串
    pub head: String,
    /// 工作区是否有未提交改动（git status --porcelain 非空）
    pub dirty: bool,
    /// 本地相对上游是否领先（有未 push 的提交）。取不到上游时为 false。
    pub ahead: bool,
    /// 本地相对上游是否落后（上游有未 pull 的提交）。取不到上游时为 false。
    pub behind: bool,
    /// 是不是 git 仓库（false 时上面字段无意义，前端不显示徽章）
    pub is_git: bool,
}

/// 对 registry 里每个项目采集本地 git 状态。永不 panic。
pub fn local_sync() -> Vec<LocalGitStatus> {
    let entries = board::registry_entries();
    entries
        .into_iter()
        .map(|(id, path)| collect_git_status(&id, &path))
        .collect()
}

/// 对单个项目目录跑 git 命令采集状态。任何命令失败都降级，不报错。
fn collect_git_status(id: &str, project_path: &str) -> LocalGitStatus {
    let root = board::expand_tilde(project_path);
    let mut st = LocalGitStatus {
        id: id.to_string(),
        head: String::new(),
        dirty: false,
        ahead: false,
        behind: false,
        is_git: false,
    };

    // 是否 git 仓库
    let is_git = git(&root, &["rev-parse", "--is-inside-work-tree"])
        .map(|o| o.trim() == "true")
        .unwrap_or(false);
    if !is_git {
        return st;
    }
    st.is_git = true;

    // 本地 HEAD
    if let Some(head) = git(&root, &["rev-parse", "HEAD"]) {
        st.head = head.trim().to_string();
    }
    // 工作区脏否
    if let Some(porcelain) = git(&root, &["status", "--porcelain"]) {
        st.dirty = !porcelain.trim().is_empty();
    }
    // 相对上游 ahead/behind（无上游时 rev-list 失败 → 保持 false）
    if let Some(counts) = git(&root, &["rev-list", "--left-right", "--count", "@{upstream}...HEAD"]) {
        // 输出形如 "behind\tahead"
        let mut it = counts.split_whitespace();
        let behind = it.next().and_then(|s| s.parse::<u32>().ok()).unwrap_or(0);
        let ahead = it.next().and_then(|s| s.parse::<u32>().ok()).unwrap_or(0);
        st.behind = behind > 0;
        st.ahead = ahead > 0;
    }
    st
}

/// 在指定目录跑 git，返回 stdout（成功且 exit 0）。失败 → None（防御性，绝不 panic）。
fn git(dir: &std::path::Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git").arg("-C").arg(dir).args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8(out.stdout).ok()
}

/// GET 服务器 /board.json。未配置 TB_BOARD_URL → Ok(None)（功能关闭）。
/// 网络/服务器错误 → Err（前端 toast，但本地看板照常）。在阻塞调用里跑，调用方应放后台。
pub fn fetch_remote_board() -> Result<Option<serde_json::Value>, String> {
    let url = match std::env::var(BOARD_URL_ENV) {
        Ok(u) if !u.trim().is_empty() => u,
        _ => return Ok(None), // 未配置 = 远端同步关闭
    };
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(10))
        .build();
    match agent.get(&url).call() {
        Ok(resp) => {
            let json: serde_json::Value = resp
                .into_json()
                .map_err(|e| format!("解析服务器 board.json 失败: {e}"))?;
            Ok(Some(json))
        }
        Err(ureq::Error::Status(code, _)) => Err(format!("服务器返回 {code}")),
        Err(e) => Err(e.to_string()),
    }
}
