import type { ProjectCard, SyncBadge } from "../types";

// status 徽标中文文案
const STATUS_LABEL: Record<string, string> = {
  active: "进行中",
  paused: "已暂停",
  done: "已完成",
  unknown: "未知",
};

// 同步徽章小组件：kind=unknown 时不渲染（非 git / 信息不足）。
function SyncTag({ sync }: { sync?: SyncBadge }) {
  if (!sync || sync.kind === "unknown" || !sync.label) return null;
  return (
    <span className={`sync-badge ${sync.kind}`} title={sync.title}>
      {sync.label}
    </span>
  );
}

// 卡片点击进入详情；右上角「移除」走确认弹窗（仅从看板移除，不删文件）。
export function Card({
  p,
  sync,
  onOpen,
  onDelete,
}: {
  p: ProjectCard;
  sync?: SyncBadge;
  onOpen: (id: string) => void;
  onDelete: (p: ProjectCard) => void;
}) {
  // 移除按钮：阻止冒泡，避免触发卡片的进入详情
  const handleDelete = (e: React.MouseEvent) => {
    e.stopPropagation();
    onDelete(p);
  };

  // 降级卡片：未接入看板 / 格式异常（仍可进入详情看原因 + 打开文件）
  if (p.error) {
    const label = p.error.kind === "missing" ? "未接入看板" : "格式异常";
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

      {p.desc && <div className="card-desc">{p.desc}</div>}

      {nextTwo.length > 0 && (
        <ul className="next">
          {nextTwo.map((n, i) => (
            <li key={i}>{n}</li>
          ))}
        </ul>
      )}

      <div className="card-foot">
        <span>更新 {p.updated || "—"}</span>
        <SyncTag sync={sync} />
      </div>
    </div>
  );
}
