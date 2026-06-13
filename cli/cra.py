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
  cra add <path> [--name NAME]   登记项目进 registry(不生成任何文件;看板字段从三件套读)
  cra remove <id>                从 registry 移除登记(不动项目文件)
  cra list                       表格输出所有项目及整体进度(进度从 CHANGELOG 项目阶段算)

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


def progress_from_changelog(changelog_path: Path) -> float | None:
    """从 CHANGELOG.md「## 项目阶段」checkbox 算整体进度(0-100)。
    完成数/总数 ×100。块缺失 / 无 checkbox → None(未接入看板)。
    与 App 端 board.rs::compute_progress_from_stages 语义一致。"""
    try:
        text = changelog_path.read_text(encoding="utf-8")
    except OSError:
        return None
    # 定位「## 项目阶段」到下一个 ## 之间
    lines = text.splitlines()
    in_section = False
    total = 0
    done = 0
    for line in lines:
        stripped = line.strip()
        if stripped.startswith("## "):
            in_section = stripped == "## 项目阶段"
            continue
        if not in_section:
            continue
        m = re.match(r"^[-*]\s+\[([ xX])\]", stripped)
        if m:
            total += 1
            if m.group(1) in ("x", "X"):
                done += 1
    if total == 0:
        return None
    return round(done / total * 100, 1)


@click.group()
def cli():
    """cra —— TaskBoard 项目登记 CLI。"""
    pass


@cli.command()
@click.argument("path")
@click.option("--name", default=None, help="看板显示名(缺省取目录名)")
def add(path: str, name: str | None):
    """登记项目进 registry(原子写)。不生成任何文件——看板字段从三件套读。"""
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

    # PROGRESS.md 已退役：cra add 只登记 registry，不再生成任何文件。
    # 看板展示字段（status/进度/简介/架构图/阶段表）从三件套读，用 /outkanban 一键生成。
    click.echo("提示: 看板字段从三件套读，登记后用 /outkanban 生成展示信息")

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
        # 进度从三件套 CHANGELOG.md「## 项目阶段」checkbox 算（PROGRESS.md 已退役）
        prog = progress_from_changelog(proj_path / "CHANGELOG.md")
        prog_str = "⚠ 未接入" if prog is None else f"{prog:.1f}%"
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
