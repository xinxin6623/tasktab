import type { ProjectCard } from "../types";

// status 徽标中文文案
const STATUS_LABEL: Record<string, string> = {
  active: "进行中",
  paused: "已暂停",
  done: "已完成",
  unknown: "未知",
};

// 卡片点击进入详情；右上角「移除」走确认弹窗（仅从看板移除，不删文件）。
export function Card({
  p,
  onOpen,
  onDelete,
}: {
  p: ProjectCard;
  onOpen: (id: string) => void;
  onDelete: (p: ProjectCard) => void;
}) {
  // 移除按钮：阻止冒泡，避免触发卡片的进入详情
  const handleDelete = (e: React.MouseEvent) => {
    e.stopPropagation();
    onDelete(p);
  };

  // 降级卡片：格式异常 / 文件缺失（仍可进入详情看原因 + 打开文件）
  if (p.error) {
    const label = p.error.kind === "missing" ? "文件缺失" : "格式异常";
    return (
      <div className="card error clickable" onClick={() => onOpen(p.id)}>
        <div className="card-head">
          <h2 className="card-title">{p.name}</h2>
          <div className="head-right">
            <span className="badge warn">⚠ {label}</span>
            <button className="icon-btn" title="从看板移除" onClick={handleDelete}>✕</button>
          </div>
        </div>
        <div className="warn-msg">{p.error.message}</div>
        <div className="card-foot">
          <span>{p.id}</span>
        </div>
      </div>
    );
  }

  const pct = Math.round(p.overall_progress);
  const total = p.stages.length;
  const curName = p.stages[p.current_stage - 1] ?? "";
  const nextTwo = p.next.slice(0, 2); // 卡片仅显示前两条（完整列表在详情页）

  return (
    <div className="card clickable" onClick={() => onOpen(p.id)}>
      <div className="card-head">
        <h2 className="card-title">
          {p.pinned && <span className="pin">★ </span>}
          {p.name}
        </h2>
        <div className="head-right">
          <span className={`badge ${p.status}`}>{STATUS_LABEL[p.status] ?? p.status}</span>
          <button className="icon-btn" title="从看板移除" onClick={handleDelete}>✕</button>
        </div>
      </div>

      <div className="progress">
        <div className="progress-track">
          <div className="progress-fill" style={{ width: `${pct}%` }} />
        </div>
        <div className="progress-meta">
          <span>整体进度</span>
          <span>{pct}%</span>
        </div>
      </div>

      <div className="stage-line">
        阶段 {p.current_stage}/{total}
        {curName && (
          <>
            {" · "}
            <span className="cur">{curName}</span>
          </>
        )}
      </div>

      {nextTwo.length > 0 && (
        <ul className="next">
          {nextTwo.map((n, i) => (
            <li key={i}>{n}</li>
          ))}
        </ul>
      )}

      <div className="card-foot">
        <span>更新 {p.updated || "—"}</span>
      </div>
    </div>
  );
}
