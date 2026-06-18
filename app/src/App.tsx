import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import type { AddResult, Board, LocalGitStatus, ProjectCard } from "./types";
import { Card } from "./components/Card";
import { Detail } from "./components/Detail";
import { ConfirmDialog } from "./components/ConfirmDialog";
import { ToastProvider, useToast } from "./components/Toast";
import { computeSyncBadge, formatTs, remoteUpdatedAt, type RemoteBoardLike } from "./sync";

// 视图状态：看板 or 某个项目详情
type View = { kind: "board" } | { kind: "detail"; id: string };

function BoardView() {
  const [board, setBoard] = useState<Board | null>(null);
  const [loadErr, setLoadErr] = useState<string | null>(null);
  const [view, setView] = useState<View>({ kind: "board" });
  // 待删除项目（确认弹窗）
  const [pendingDelete, setPendingDelete] = useState<ProjectCard | null>(null);
  // 设备间同步：本地 git 状态（按 id 索引）+ 服务器聚合的 board.json（取 commit / generated_at）
  const [localSync, setLocalSync] = useState<Record<string, LocalGitStatus>>({});
  const [remoteBoard, setRemoteBoard] = useState<RemoteBoardLike | null>(null);
  const toast = useToast();

  const refresh = useCallback(() => {
    invoke<Board>("load_board")
      .then((b) => {
        setBoard(b);
        setLoadErr(null);
      })
      .catch((e) => setLoadErr(String(e)));
  }, []);

  // 刷新同步状态：并行拉「本地 git 状态」+「服务器 board.json」，失败不影响本地看板（徽章降级）。
  const refreshSync = useCallback(() => {
    invoke<LocalGitStatus[]>("load_local_sync")
      .then((list) => {
        const map: Record<string, LocalGitStatus> = {};
        for (const s of list) map[s.id] = s;
        setLocalSync(map);
      })
      .catch(() => setLocalSync({})); // 取不到本地 git 状态 → 徽章按 unknown，看板照常
    invoke<RemoteBoardLike | null>("load_remote_board")
      .then((b) => setRemoteBoard(b ?? null))
      .catch(() => setRemoteBoard(null)); // 远端拉不到 → offline 徽章，本地看板照常
  }, []);

  useEffect(() => {
    refresh();
    refreshSync();
  }, [refresh, refreshSync]);

  // M4：订阅 Rust 侧 notify 监听发出的 board-changed 事件（registry.yaml / 任一三件套变更，
  // 去抖 500ms 后触发），收到就整体重拉看板，用户无需任何手动操作。
  // 设备间同步：文件一变可能意味着本地刚提交/改动，顺带刷新同步徽章。
  // 组件卸载时 unlisten，避免重复订阅。
  useEffect(() => {
    const unlistenPromise = listen("board-changed", () => {
      refresh();
      refreshSync();
    });
    return () => {
      unlistenPromise.then((unlisten) => unlisten());
    };
  }, [refresh, refreshSync]);

  // 设备间同步：定时（30s）刷新一次同步徽章——另一台设备 push 后服务器侧 commit 会变，
  // 即便本地文件没动，也要让「待拉取」徽章及时更新。本地 git 状态也一并刷新。
  useEffect(() => {
    const timer = setInterval(refreshSync, 30_000);
    return () => clearInterval(timer);
  }, [refreshSync]);

  // 添加项目：目录选择对话框 → Rust add_project（登记逻辑与 cra.py 对齐）→ 刷新
  const handleAdd = async () => {
    try {
      const dir = await openDialog({ directory: true, multiple: false, title: "选择项目目录" });
      if (!dir || typeof dir !== "string") return; // 用户取消
      const r = await invoke<AddResult>("add_project", { path: dir, name: null });
      toast(r.created_template ? `已添加并生成模板：${r.name}` : `已添加：${r.name}`);
      refresh();
    } catch (e) {
      toast(`添加失败：${e}`);
    }
  };

  // 确认删除：仅从 registry 移除，不动项目文件
  const confirmDelete = async () => {
    if (!pendingDelete) return;
    const target = pendingDelete;
    setPendingDelete(null);
    try {
      await invoke("remove_project", { id: target.id });
      toast(`已从看板移除：${target.name}（项目文件未改动）`);
      refresh();
    } catch (e) {
      toast(`移除失败：${e}`);
    }
  };

  // 详情页
  if (view.kind === "detail") {
    return <Detail id={view.id} onBack={() => { setView({ kind: "board" }); refresh(); }} />;
  }

  if (loadErr) {
    return <div className="notice">加载看板失败：{loadErr}</div>;
  }
  if (!board) {
    return <div className="notice">加载中…</div>;
  }

  const { summary, projects, registry_error } = board;

  return (
    <>
      <div className="topbar">
        <div className="topbar-row">
          <h1>TaskBoard</h1>
          <button className="btn primary" onClick={handleAdd}>+ 添加项目</button>
        </div>
        <div className="summary">
          <span className="count"><span className="dot active" />进行中 {summary.active}</span>
          <span className="count"><span className="dot paused" />已暂停 {summary.paused}</span>
          <span className="count"><span className="dot done" />已完成 {summary.done}</span>
          {summary.error > 0 && (
            <span className="count"><span className="dot error" />异常 {summary.error}</span>
          )}
          {/* 设备间同步：服务器（看板镜像）最后聚合时间。未连服务器时不显示。 */}
          {remoteBoard && remoteUpdatedAt(remoteBoard) && (
            <span className="count sync-updated" title="看板服务器最后从 GitHub 聚合的时间">
              ⟳ 看板 {formatTs(remoteUpdatedAt(remoteBoard))}
            </span>
          )}
        </div>
      </div>

      {registry_error && (
        <div className="notice">登记表读取异常：{registry_error}</div>
      )}

      {projects.length === 0 && !registry_error ? (
        <div className="notice">
          还没有登记任何项目。点右上角 <b>+ 添加项目</b>，或用 <code>cra add &lt;项目路径&gt;</code> 登记。
        </div>
      ) : (
        <div className="grid">
          {projects.map((p) => (
            <Card
              key={p.id}
              p={p}
              sync={computeSyncBadge(p.id, localSync[p.id], remoteBoard)}
              onOpen={(id) => setView({ kind: "detail", id })}
              onDelete={(c) => setPendingDelete(c)}
            />
          ))}
        </div>
      )}

      {pendingDelete && (
        <ConfirmDialog
          title="从看板移除项目"
          body={
            <>
              <p>确定移除 <b>{pendingDelete.name}</b>？</p>
              <p className="modal-note">仅从看板移除，<b>不删除任何文件</b>。项目目录与 PROGRESS.md 保持原样，可随时重新添加。</p>
            </>
          }
          confirmText="移除"
          onConfirm={confirmDelete}
          onCancel={() => setPendingDelete(null)}
        />
      )}
    </>
  );
}

export default function App() {
  // Toast 在最外层提供，详情页与看板都能用
  return (
    <ToastProvider>
      <BoardView />
    </ToastProvider>
  );
}
