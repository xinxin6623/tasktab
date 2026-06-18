// push.rs —— 看板镜像推送（「手机查看」功能的 App 侧出口）
//
// 中文说明（重要逻辑 / 架构边界）：
//   本模块是项目「App 端零网络」铁律的【唯一、受控例外】。要点：
//   - 它只做【单向输出】：把已经解析好的 board.json 原样 POST 到 James 自己的服务器，
//     供手机浏览器只读查看。它【不】取任何数据、不调用任何 LLM、不影响看板解析与渲染。
//   - 解析仍 100% 由 board::load_board() 完成；本模块拿到的就是那份结构体，序列化后推走。
//   - 完全由环境变量 TB_PUSH_URL 开关：未设置 = 功能关闭，App 行为与之前完全一致。
//     不写死任何地址/密钥，不进 git。
//
//   推送时机：watcher 每次 emit board-changed 时顺带调用 push_board_async()，
//   即「文件一变 → 桌面看板刷新 → 同时把最新快照推到服务器」。推送在独立线程里跑，
//   失败只打日志、绝不影响 App（服务器挂了/断网了，桌面看板照常工作）。
//
// 配置（环境变量）：
//   TB_PUSH_URL   服务端 /ingest 完整地址，如 https://board.alphaxbot.xyz/ingest
//                 未设置则推送整体关闭。
//   TB_PUSH_TOKEN 可选。设置则带 Authorization: Bearer <token>（服务端当前全公开，留作以后加固用）。

use std::time::Duration;

use crate::board;

const PUSH_URL_ENV: &str = "TB_PUSH_URL";
const PUSH_TOKEN_ENV: &str = "TB_PUSH_TOKEN";

/// 异步推送当前看板快照到服务器。在独立线程里跑，立即返回，不阻塞调用方（watcher）。
/// 未配置 TB_PUSH_URL 时直接返回（功能关闭）。
pub fn push_board_async() {
    let url = match std::env::var(PUSH_URL_ENV) {
        Ok(u) if !u.trim().is_empty() => u,
        _ => return, // 未配置 = 功能关闭
    };
    let token = std::env::var(PUSH_TOKEN_ENV).ok().filter(|t| !t.trim().is_empty());

    std::thread::spawn(move || {
        // 在推送线程内重新拉一次最新看板（带详情，供手机端详情页用），保证推的是当前快照。
        let board = board::load_push_board();
        let body = match serde_json::to_vec(&board) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("[push] 序列化看板失败: {e}");
                return;
            }
        };
        if let Err(e) = post_json(&url, &body, token.as_deref()) {
            // 网络/服务器问题不影响桌面看板，只记日志。
            eprintln!("[push] 推送看板到 {url} 失败（桌面看板不受影响）: {e}");
        } else {
            eprintln!("[push] 已推送看板快照到 {url}（{} 字节）", body.len());
        }
    });
}

/// 用 ureq 发一个带超时的 POST application/json。阻塞，但跑在推送线程里。
fn post_json(url: &str, body: &[u8], token: Option<&str>) -> Result<(), String> {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(10))
        .build();
    let mut req = agent
        .post(url)
        .set("Content-Type", "application/json; charset=utf-8");
    if let Some(t) = token {
        req = req.set("Authorization", &format!("Bearer {t}"));
    }
    match req.send_bytes(body) {
        Ok(_) => Ok(()),
        Err(ureq::Error::Status(code, _)) => Err(format!("服务端返回 {code}")),
        Err(e) => Err(e.to_string()),
    }
}
