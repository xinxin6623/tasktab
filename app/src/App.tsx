import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import type { AddResult, Board, ProjectCard } from "./types";
import { Card } from "./components/Card";
import { Detail } from "./components/Detail";
import { ConfirmDialog } from "./components/ConfirmDialog";
import { ToastProvider, useToast } from "./components/Toast";

// 视图状态：看板 or 某个项目详情
type View = { kind: "board" } | { kind: "detail"; id: string };

function BoardView() {
  const [board, setBoard] = useState<Board | null>(null);
  const [loadErr, setLoadErr] = useState<string | null>(null);
  const [view, setView] = useState<View>({ kind: "board" });
  // 待删除项目（确认弹窗）
  const [pendingDelete, setPendingDelete] = useState<ProjectCard | null>(null);
  const toast = useToast();

  const refresh = useCallback(() => {
    invoke<Board>("load_board")
      .then((b) => {
        setBoard(b);
        setLoadErr(null);
      })
      .catch((e) => setLoadErr(String(e)));
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  // M4：订阅 Rust 侧 notify 监听发出的 board-changed 事件（registry.yaml / 任一 PROGRESS.md 变更，
  // 去抖 500ms 后触发），收到就整体重拉看板，用户无需任何手动操作。
  // 组件卸载时 unlisten，避免重复订阅。
  useEffect(() => {
    const unlistenPromise = listen("board-changed", () => {
      refresh();
    });
    return () => {
      unlistenPromise.then((unlisten) => unlisten());
    };
  }, [refresh]);

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
