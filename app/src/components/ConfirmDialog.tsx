// ConfirmDialog.tsx —— 通用确认弹窗（删除项目用）。
// 删除文案必须明确「仅从看板移除，不删除任何文件」（02 §3 / AGENTS.md 硬规则）。
import type { ReactNode } from "react";

export function ConfirmDialog({
  title,
  body,
  confirmText = "确认",
  cancelText = "取消",
  onConfirm,
  onCancel,
}: {
  title: string;
  body: ReactNode;
  confirmText?: string;
  cancelText?: string;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  return (
    <div className="modal-mask" onClick={onCancel}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <h2 className="modal-title">{title}</h2>
        <div className="modal-body">{body}</div>
        <div className="modal-actions">
          <button className="btn" onClick={onCancel}>{cancelText}</button>
          <button className="btn danger" onClick={onConfirm}>{confirmText}</button>
        </div>
      </div>
    </div>
  );
}
