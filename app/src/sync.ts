// sync.ts —— 同步徽章计算（前端把「本地 git 状态」与「服务器 commit」比对成徽章）
//
// 中文说明（重要逻辑）：
//   新「设备间同步」方案下，服务器从 GitHub 聚合看板。每张卡片要回答：「我本机这个项目，
//   改动同步到 GitHub / 看板了吗？」判定输入有两个：
//     - 本地：load_local_sync 返回的 LocalGitStatus（本地 HEAD / 脏 / ahead / behind）
//     - 远端：load_remote_board 返回的 board.json，每项目带服务器拉到的 commit
//   比对规则（优先级从上到下）：
//     1) 非 git 仓库 / 信息不足          → unknown（不显示徽章）
//     2) 远端未配置或拉不到               → offline（本地状态仍可单独提示脏/领先）
//     3) 工作区脏（有未提交改动）         → dirty（最该提醒：还没 commit）
//     4) 本地 HEAD == 服务器 commit       → synced（已同步，手机/另一台看到的就是这份）
//     5) 本地 ahead（有未 push 提交）      → ahead（待推送）
//     6) 本地 behind（上游更新了）         → behind（待拉取，另一台 push 过）
//     7) HEAD≠commit 但方向不明           → diverged（提示手动对一下）
//   App 端零智能：Rust 只给原始 git 元信息 + 原始 board.json，语义判定全在这一层（前端）。

import type { LocalGitStatus, SyncBadge, SyncKind } from "./types";

// 服务器 board.json 里我们关心的最小结构（其余字段忽略）。
// Rust load_remote_board 把整个 board.json 作为不透明 JSON 返回（App 端零智能），
// 这里用宽松类型 + 运行时取值，容忍服务器多出/缺字段。
interface RemoteProject {
  id: string;
  commit?: string | null;
}
export type RemoteBoardLike = {
  projects?: RemoteProject[];
  generated_at?: string | null;
} & Record<string, unknown>;

const LABELS: Record<SyncKind, { label: string; title: string }> = {
  synced: { label: "已同步", title: "本地与看板一致：手机/另一台设备看到的就是当前这份。" },
  dirty: { label: "未提交", title: "工作区有未提交改动，记得 commit + push 才会同步到看板。" },
  ahead: { label: "待推送", title: "本地有未 push 的提交，push 后看板和其他设备才会更新。" },
  behind: { label: "待拉取", title: "另一台设备已 push 更新，本地落后，记得 pull。" },
  diverged: { label: "已分叉", title: "本地与看板的 commit 不一致且方向不明，手动对一下 git 状态。" },
  offline: { label: "未连看板", title: "未配置或拉不到看板服务器（TB_BOARD_URL），仅显示本地 git 状态。" },
  unknown: { label: "", title: "" },
};

function badge(kind: SyncKind): SyncBadge {
  return { kind, ...LABELS[kind] };
}

/**
 * 计算单个项目的同步徽章。
 * @param local  该项目本地 git 状态；undefined = 没采集到（按 unknown）
 * @param remote 服务器 board.json（含 projects[].commit）；null = 远端未配置/拉不到
 */
export function computeSyncBadge(
  id: string,
  local: LocalGitStatus | undefined,
  remote: RemoteBoardLike | null,
): SyncBadge {
  // 1) 非 git / 信息不足
  if (!local || !local.is_git) return badge("unknown");

  // 找服务器侧该项目的 commit
  const remoteProj = remote?.projects?.find((p) => p.id === id);
  const remoteCommit = (remoteProj?.commit ?? "").trim();

  // 3) 工作区脏优先提醒（无论远端如何，先 commit 才谈得上同步）
  if (local.dirty) return badge("dirty");

  // 2) 远端未配置 / 该项目服务器没 commit → 无对比基准，只能靠本地 ahead/behind 兜底
  if (!remote || remoteCommit === "") {
    if (local.ahead) return badge("ahead");
    if (local.behind) return badge("behind");
    return badge("offline");
  }

  // 4) 本地 HEAD 与服务器 commit 一致 → 已同步
  if (local.head && local.head === remoteCommit) return badge("synced");

  // 5/6) HEAD ≠ commit：用 git 的 ahead/behind 方向判定更准
  if (local.ahead && !local.behind) return badge("ahead");
  if (local.behind && !local.ahead) return badge("behind");

  // 7) 方向不明（两边都动过 / 取不到上游）→ 分叉
  return badge("diverged");
}

/** 从 remote board.json 取服务器最后更新时间（generated_at），无则 null。 */
export function remoteUpdatedAt(remote: RemoteBoardLike | null): string | null {
  const ts = remote?.generated_at;
  return ts ? ts : null;
}

/** 把 RFC3339 时间戳格式化成本地可读的「YYYY-MM-DD HH:mm」。解析失败原样返回。 */
export function formatTs(ts: string | null): string {
  if (!ts) return "—";
  const d = new Date(ts);
  if (isNaN(d.getTime())) return ts;
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}
