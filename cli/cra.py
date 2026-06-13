#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = [
#     "pyyaml>=6.0",
#     "click>=8.1",
# ]
# ///
"""cra —— TaskBoard 项目登记 CLI。

子命令:
  cra add <path> [--name NAME]   登记项目;无 PROGRESS.md 则按 02 §1.1 生成模板
  cra remove <id>                从 registry 移除登记(不动项目文件)
  cra list                       表格输出所有项目及整体进度百分比

数据契约权威来源:同步看板files/02-实现步骤.md §1.1 / §1.2,逐字段对齐。
"""

import os
import re
import sys
import tempfile
from datetime import date
from pathlib import Path

import click
import yaml

# registry 默认路径。可被环境变量覆盖以便隔离测试,但默认仍为契约规定路径。
# 02 §1.2: ~/.ai-vault/taskboard/registry.yaml
DEFAULT_REGISTRY = "~/.ai-vault/taskboard/registry.yaml"
REGISTRY_VERSION = 1
PROGRESS_FILENAME = "PROGRESS.md"


def registry_path() -> Path:
    """解析 registry 路径:优先环境变量 CRA_REGISTRY(测试隔离用),否则用契约默认值。
    始终展开 ~ 并容忍中文路径(Path 对 unicode 路径天然支持)。"""
    raw = os.environ.get("CRA_REGISTRY", DEFAULT_REGISTRY)
    return Path(raw).expanduser()


def kebab_case(name: str) -> str:
    """把目录名转成 kebab-case 作为 project id。
    规则:非字母数字(含中文以外)统一转连字符,折叠多重连字符,去首尾连字符,转小写。
    中文等 unicode 字符予以保留(看板需要可读 id,且契约只示例了英文 kebab)。"""
    # 先把空白与常见分隔符替换为连字符
    s = name.strip().lower()
    # 把任何非 [字母数字/中日韩等文字字符] 的连续片段折叠为单个连字符
    s = re.sub(r"[^0-9a-z一-鿿]+", "-", s)
    s = re.sub(r"-{2,}", "-", s).strip("-")
    return s or "project"


def load_registry() -> dict:
    """读取 registry;不存在则返回初始化结构(含 version: 1)。
    解析失败时直接报错退出,避免在损坏文件上继续写入。"""
    path = registry_path()
    if not path.exists():
        return {"version": REGISTRY_VERSION, "projects": []}
    try:
        with path.open("r", encoding="utf-8") as f:
            data = yaml.safe_load(f)
    except yaml.YAMLError as e:
        raise click.ClickException(f"registry 解析失败({path}): {e}")
    if data is None:
        data = {}
    if not isinstance(data, dict):
        raise click.ClickException(f"registry 格式异常,期望映射: {path}")
    # 容错补全必需字段
    data.setdefault("version", REGISTRY_VERSION)
    data.setdefault("projects", [])
    if not isinstance(data["projects"], list):
        raise click.ClickException(f"registry 的 projects 字段必须是列表: {path}")
    return data


def write_registry_atomic(data: dict) -> None:
    """原子写入 registry:写临时文件 → fsync → os.rename(同目录,保证原子性)。
    02 §1.2 / AGENTS.md 硬规则:杜绝半截写入损坏 registry。"""
    path = registry_path()
    path.parent.mkdir(parents=True, exist_ok=True)
    # 临时文件必须与目标在同一目录,否则 rename 可能跨设备失败、丧失原子性
    fd, tmp_name = tempfile.mkstemp(
        dir=str(path.parent), prefix=".registry.", suffix=".tmp"
    )
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as f:
            yaml.safe_dump(
                data,
                f,
                allow_unicode=True,  # 容忍中文 name / path
                sort_keys=False,
                default_flow_style=False,
            )
            f.flush()
            os.fsync(f.fileno())
        os.rename(tmp_name, path)  # 同目录 rename,POSIX 保证原子替换
    except Exception:
        # 出错时清理临时文件,避免残留
        if os.path.exists(tmp_name):
            os.unlink(tmp_name)
        raise


def progress_template(project_id: str) -> str:
    """按 02 §1.1 schema 生成 PROGRESS.md 模板。
    占位:一个阶段、current_stage=1、stage_progress=0,status 默认 active。"""
    today = date.today().isoformat()
    frontmatter = {
        "project": project_id,
        "desc": "",  # 项目一句话描述,卡片展示用(≤30 字);留空则卡片不显示该行
        "status": "active",
        "stages": ["需求与架构"],  # 占位阶段
        "current_stage": 1,
        "stage_progress": 0,
        "next": [],
        "blocked_by": [],
        "updated": today,
    }
    fm_text = yaml.safe_dump(
        frontmatter, allow_unicode=True, sort_keys=False, default_flow_style=False
    )
    return f"""---
{fm_text}---

## 阶段记录

(自由 markdown 正文,App 仅做只读预览渲染,不解析)
"""


def parse_frontmatter(md_path: Path) -> dict | None:
    """解析 PROGRESS.md frontmatter。解析失败 / 缺失返回 None(调用方降级处理,绝不崩溃)。"""
    try:
        text = md_path.read_text(encoding="utf-8")
    except OSError:
        return None
    # frontmatter 形如 ---\n ... \n---
    m = re.match(r"^---\s*\n(.*?)\n---\s*(\n|$)", text, re.DOTALL)
    if not m:
        return None
    try:
        data = yaml.safe_load(m.group(1))
    except yaml.YAMLError:
        return None
    return data if isinstance(data, dict) else None


def overall_progress(fm: dict) -> float | None:
    """按 02 §1.1 公式计算整体进度百分比(0-100)。
    公式: (current_stage - 1 + stage_progress/100) / len(stages)
    status==done 强制 100%。无法计算(越界/空 stages/字段缺失)返回 None。"""
    status = fm.get("status")
    stages = fm.get("stages")
    if status == "done":
        return 100.0
    if not isinstance(stages, list) or len(stages) == 0:
        return None
    n = len(stages)
    current = fm.get("current_stage")
    if not isinstance(current, int) or current < 1 or current > n:
        return None
    # stage_progress 可选,缺省按 0 处理
    sp = fm.get("stage_progress", 0)
    if not isinstance(sp, (int, float)):
        sp = 0
    ratio = (current - 1 + sp / 100.0) / n
    return round(ratio * 100, 1)


@click.group()
def cli():
    """cra —— TaskBoard 项目登记 CLI。"""
    pass


@cli.command()
@click.argument("path")
@click.option("--name", default=None, help="看板显示名(缺省取目录名)")
def add(path: str, name: str | None):
    """登记项目;无 PROGRESS.md 则生成模板,写入 registry。"""
    proj_dir = Path(path).expanduser().resolve()
    # 校验路径
    if not proj_dir.exists():
        raise click.ClickException(f"路径不存在: {proj_dir}")
    if not proj_dir.is_dir():
        raise click.ClickException(f"不是目录: {proj_dir}")

    reg = load_registry()
    abs_path = str(proj_dir)

    # 已存在同 path 则报错(去重以解析后的绝对路径为准)
    for p in reg["projects"]:
        existing = Path(str(p.get("path", ""))).expanduser()
        try:
            existing = existing.resolve()
        except OSError:
            pass
        if str(existing) == abs_path:
            raise click.ClickException(
                f"该路径已登记(id={p.get('id')}): {abs_path}"
            )

    project_id = kebab_case(proj_dir.name)
    # id 唯一性:若已存在同 id,追加短后缀避免冲突
    existing_ids = {p.get("id") for p in reg["projects"]}
    if project_id in existing_ids:
        suffix = 2
        while f"{project_id}-{suffix}" in existing_ids:
            suffix += 1
        project_id = f"{project_id}-{suffix}"

    # 若无 PROGRESS.md 则按 schema 生成模板
    progress_md = proj_dir / PROGRESS_FILENAME
    if not progress_md.exists():
        progress_md.write_text(progress_template(project_id), encoding="utf-8")
        click.echo(f"已生成模板: {progress_md}")
    else:
        # 已有 PROGRESS.md 时复用其 project id(保持与文件一致)
        fm = parse_frontmatter(progress_md)
        if fm and isinstance(fm.get("project"), str) and fm["project"].strip():
            file_id = fm["project"].strip()
            if file_id not in existing_ids:
                project_id = file_id
        click.echo(f"沿用已有 PROGRESS.md: {progress_md}")

    display_name = name if name else proj_dir.name
    entry = {
        "id": project_id,
        "name": display_name,
        "path": abs_path,
        "progress_file": PROGRESS_FILENAME,
        "pinned": False,
        "added": date.today().isoformat(),
    }
    reg["projects"].append(entry)
    write_registry_atomic(reg)
    click.echo(f"已登记: {project_id}  ({display_name})  -> {abs_path}")


@cli.command()
@click.argument("project_id")
def remove(project_id: str):
    """从 registry 移除登记(不触碰项目文件)。"""
    reg = load_registry()
    before = len(reg["projects"])
    reg["projects"] = [p for p in reg["projects"] if p.get("id") != project_id]
    after = len(reg["projects"])
    if before == after:
        raise click.ClickException(f"未找到 id: {project_id}")
    write_registry_atomic(reg)
    click.echo(f"已移除: {project_id}(项目文件未改动)")


@cli.command(name="list")
def list_cmd():
    """终端表格输出所有项目及整体进度百分比。"""
    reg = load_registry()
    projects = reg["projects"]
    if not projects:
        click.echo("registry 为空,用 `cra add <path>` 登记项目。")
        return

    rows = []
    for p in projects:
        pid = str(p.get("id", "?"))
        pname = str(p.get("name", ""))
        proj_path = Path(str(p.get("path", ""))).expanduser()
        pf = p.get("progress_file", PROGRESS_FILENAME)
        md_path = proj_path / pf
        if not md_path.exists():
            prog_str = "⚠ 文件缺失"
        else:
            fm = parse_frontmatter(md_path)
            if fm is None:
                prog_str = "⚠ 格式异常"
            else:
                prog = overall_progress(fm)
                prog_str = "⚠ 格式异常" if prog is None else f"{prog:.1f}%"
        rows.append((pid, pname, prog_str, str(proj_path)))

    # 计算列宽(按显示宽度,中文占 2)
    def disp_width(s: str) -> int:
        w = 0
        for ch in s:
            w += 2 if ord(ch) > 0x2E7F else 1
        return w

    headers = ("ID", "NAME", "PROGRESS", "PATH")
    cols = list(zip(*([headers] + rows)))
    widths = [max(disp_width(str(c)) for c in col) for col in cols]

    def pad(s: str, w: int) -> str:
        return s + " " * (w - disp_width(s))

    def fmt_row(cells) -> str:
        return "  ".join(pad(str(c), widths[i]) for i, c in enumerate(cells))

    click.echo(fmt_row(headers))
    click.echo("  ".join("-" * w for w in widths))
    for r in rows:
        click.echo(fmt_row(r))


if __name__ == "__main__":
    cli()
