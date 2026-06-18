// parse.go —— 三件套解析器（Go 重实现，逐字段对齐 App 端 board.rs）
//
// ⚠️ SSOT 警告（务必读）：
//   本文件是 app/src-tauri/src/board.rs 解析逻辑的【第二份实现】。看板字段 schema 的
//   唯一权威是 `同步看板files/02-实现步骤.md` §1.1b；board.rs 与本文件都是它的派生实现。
//   任何契约变更（字段、checkbox 规则、Handoff 概述/明细拆分、日期提取…）必须【同时】
//   改 board.rs 和本文件，否则桌面端与手机镜像端会出现解析漂移。
//   每个函数都标注了它对应 board.rs 的哪个函数，改动时按图索骥。
//
// 为什么要两份：服务器走 GitHub API 直接读各 repo 的三件套原文，在服务器侧解析聚合，
//   各端（桌面 App / 手机网页）只读结果。服务器是 Go，不便复用 Rust，故重写一份并用
//   契约文档 + 本警告钉死一致性。设计取舍见 server/README.md「为什么解析逻辑有两份」。
//
// 防御性铁律（与 board.rs 一致）：任何块/文件缺失、frontmatter 损坏都不得 panic，
//   该字段取缺省（status=active、进度 0、列表空），绝不影响其他项目。
package main

import (
	"strings"

	"gopkg.in/yaml.v3"
)

// ───────────────────────── 输出结构（对齐 board.rs ProjectCard / PushProject）─────────────────────────
// JSON 字段名与 board.rs serde 输出【逐字段一致】，这样手机网页与桌面端共用同一套字段名。

// StageItem 对齐 board.rs::StageItem（CHANGELOG「## 项目阶段」的单个 checkbox）。
type StageItem struct {
	Name string `json:"name"`
	Desc string `json:"desc"`
	Done bool   `json:"done"`
}

// ParseError 对齐 board.rs::ParseError（降级卡片原因）。
type ParseError struct {
	Kind    string `json:"kind"`    // missing | format
	Message string `json:"message"` // 中文说明
}

// ProjectCard 对齐 board.rs::ProjectCard + PushProject 的平铺字段。
// 额外多出「同步 / 来源」字段（本方案新增，board.rs 暂无对应，详见 §1.1b 增补）：
//   github / commit / branch / synced_at —— 用于各端显示「服务器最后更新时间」与同步徽章。
type ProjectCard struct {
	ID    string `json:"id"`
	Name  string `json:"name"`
	Pinned bool  `json:"pinned"`

	Desc          string   `json:"desc"`
	Status        string   `json:"status"` // active | paused | done | unknown
	Stages        []string `json:"stages"`
	CurrentStage  int64    `json:"current_stage"`
	StageProgress float64  `json:"stage_progress"`
	OverallProgress float64 `json:"overall_progress"`
	Next      []string `json:"next"`
	BlockedBy []string `json:"blocked_by"`
	Updated   string   `json:"updated"`

	// 详情块（对齐 PushProject 的 intro / arch_mermaid / stage_items / handoff_detail）
	Intro         string      `json:"intro"`
	ArchMermaid   string      `json:"arch_mermaid"`
	StageItems    []StageItem `json:"stage_items"`
	HandoffDetail string      `json:"handoff_detail"`

	// —— 本方案新增：GitHub 来源 + 同步锚点 ——
	Github   string `json:"github"`    // owner/repo@branch
	Commit   string `json:"commit"`    // 服务器拉到的最新 commit SHA（App 拿本地 HEAD 比对）
	Branch   string `json:"branch"`    // 镜像分支
	SyncedAt string `json:"synced_at"` // 服务器拉取该 repo 的时间（RFC3339）

	Error *ParseError `json:"error"` // 非 nil = 降级卡片
}

// ───────────────────────── frontmatter 提取（对齐 board.rs::extract_frontmatter）─────────────────────────

// extractFrontmatter 切出 YAML frontmatter 块（首个非空行须为 ---，到下一个 --- 结束）。
// 无合法 frontmatter 返回 ("", false)。对齐 board.rs::extract_frontmatter。
func extractFrontmatter(content string) (string, bool) {
	trimmed := strings.TrimPrefix(content, "\ufeff") // 容忍 BOM
	lines := strings.Split(trimmed, "\n")
	if len(lines) == 0 || strings.TrimSpace(lines[0]) != "---" {
		return "", false
	}
	var buf []string
	for _, line := range lines[1:] {
		if strings.TrimSpace(strings.TrimRight(line, "\r")) == "---" {
			return strings.Join(buf, "\n"), true
		}
		buf = append(buf, line)
	}
	return "", false
}

// agentsMeta 对齐 board.rs::AgentsMeta（AGENTS.md frontmatter 的 status / desc）。
type agentsMeta struct {
	status string // 空 = 未提供
	desc   string
}

// parseAgentsMeta 解析 AGENTS.md frontmatter 取 status / desc。
// 缺失 / YAML 损坏 → 都空（防御性）。对齐 board.rs::parse_agents_meta。
func parseAgentsMeta(agentsMD string) agentsMeta {
	fm, ok := extractFrontmatter(agentsMD)
	if !ok {
		return agentsMeta{}
	}
	var m map[string]any
	if err := yaml.Unmarshal([]byte(fm), &m); err != nil {
		return agentsMeta{}
	}
	get := func(k string) string {
		if v, ok := m[k].(string); ok {
			return strings.TrimSpace(v)
		}
		return ""
	}
	return agentsMeta{status: get("status"), desc: get("desc")}
}

// ───────────────────────── 块提取（对齐 board.rs::extract_section / extract_section_prefix）─────────────────────────

// extractSection 取 `## <heading>` 到下一个 `## ` 之间的正文（trim 后）。找不到返回 ""。
// 对齐 board.rs::extract_section。
func extractSection(md, heading string) string {
	target := "## " + heading
	lines := strings.Split(md, "\n")
	i := 0
	for ; i < len(lines); i++ {
		if strings.TrimSpace(lines[i]) == target {
			break
		}
	}
	if i >= len(lines) {
		return ""
	}
	var buf []string
	for _, line := range lines[i+1:] {
		if strings.HasPrefix(strings.TrimLeft(line, " \t"), "## ") {
			break
		}
		buf = append(buf, line)
	}
	return strings.TrimSpace(strings.Join(buf, "\n"))
}

// extractSectionPrefix 是 extractSection 的宽松版：标题以 `## <prefix>` 开头即匹配
// （容忍标题后缀，如「## 当前接力点 (Handoff)」）。对齐 board.rs::extract_section_prefix。
func extractSectionPrefix(md, prefix string) string {
	target := "## " + prefix
	lines := strings.Split(md, "\n")
	i := 0
	for ; i < len(lines); i++ {
		if strings.HasPrefix(strings.TrimSpace(lines[i]), target) {
			break
		}
	}
	if i >= len(lines) {
		return ""
	}
	var buf []string
	for _, line := range lines[i+1:] {
		if strings.HasPrefix(strings.TrimLeft(line, " \t"), "## ") {
			break
		}
		buf = append(buf, line)
	}
	return strings.TrimSpace(strings.Join(buf, "\n"))
}

// extractMermaid 取一段文本里首个 ```mermaid 代码块内容（不含围栏）。无则 ""。
// 对齐 board.rs::extract_mermaid。
func extractMermaid(section string) string {
	lines := strings.Split(section, "\n")
	i := 0
	for ; i < len(lines); i++ {
		if strings.HasPrefix(strings.TrimSpace(lines[i]), "```mermaid") {
			break
		}
	}
	if i >= len(lines) {
		return ""
	}
	var buf []string
	for _, line := range lines[i+1:] {
		if strings.HasPrefix(strings.TrimSpace(line), "```") {
			break
		}
		buf = append(buf, line)
	}
	return strings.TrimRight(strings.Join(buf, "\n"), "\n ")
}

// ───────────────────────── 阶段表（对齐 board.rs::extract_stage_list / split_name_desc）─────────────────────────

// extractStageList 解析 CHANGELOG「## 项目阶段」下的 checkbox 列表。
// 每行 `- [x] 名 — 描述` / `- [ ] 名`；[x]/[X]=完成。对齐 board.rs::extract_stage_list。
func extractStageList(md string) []StageItem {
	section := extractSection(md, "项目阶段")
	if section == "" {
		return nil
	}
	var items []StageItem
	for _, raw := range strings.Split(section, "\n") {
		line := strings.TrimSpace(raw)
		var rest string
		switch {
		case strings.HasPrefix(line, "- ["):
			rest = strings.TrimPrefix(line, "- [")
		case strings.HasPrefix(line, "* ["):
			rest = strings.TrimPrefix(line, "* [")
		default:
			continue
		}
		idx := strings.IndexByte(rest, ']')
		if idx < 0 {
			continue
		}
		mark := strings.TrimSpace(rest[:idx])
		after := strings.TrimSpace(rest[idx+1:])
		done := mark == "x" || mark == "X"
		name, desc := splitNameDesc(after)
		if name == "" {
			continue
		}
		items = append(items, StageItem{Name: name, Desc: desc, Done: done})
	}
	return items
}

// splitNameDesc 把 "名 — 描述" 拆成 (名, 描述)。对齐 board.rs::split_name_desc。
// 分隔符顺序须与 Rust 一致（先长后短，避免 "—" 抢在 " — " 前匹配）。
func splitNameDesc(text string) (string, string) {
	for _, sep := range []string{" — ", " - ", " —— ", "—", " – "} {
		if i := strings.Index(text, sep); i >= 0 {
			return strings.TrimSpace(text[:i]), strings.TrimSpace(text[i+len(sep):])
		}
	}
	return text, ""
}

// computeProgressFromStages 由 checkbox 算整体进度：完成数/总数×100。
// status=done 强制 100；空表 0。对齐 board.rs::compute_progress_from_stages。
func computeProgressFromStages(stages []StageItem, status string) float64 {
	if status == "done" {
		return 100
	}
	if len(stages) == 0 {
		return 0
	}
	done := 0
	for _, s := range stages {
		if s.Done {
			done++
		}
	}
	v := float64(done) / float64(len(stages)) * 100
	if v < 0 {
		v = 0
	}
	if v > 100 {
		v = 100
	}
	return v
}

// ───────────────────────── Handoff（对齐 board.rs::extract_handoff 系列）─────────────────────────

// extractHandoff 从 INDEX「## 当前接力点」区解析 (next, blockedBy)。
// 只读「### 概述」子段；⚠/阻塞 前缀归 blockedBy。对齐 board.rs::extract_handoff。
func extractHandoff(indexMD string) (next, blocked []string) {
	section := extractSectionPrefix(indexMD, "当前接力点")
	if section == "" {
		return nil, nil
	}
	overview := extractHandoffOverview(section)
	for _, raw := range strings.Split(overview, "\n") {
		line := strings.TrimSpace(raw)
		if line == "" || strings.HasPrefix(line, "<!--") || strings.HasPrefix(line, "-->") || strings.HasPrefix(line, ">") {
			continue
		}
		item := line
		if strings.HasPrefix(item, "- ") {
			item = item[2:]
		} else if strings.HasPrefix(item, "* ") {
			item = item[2:]
		}
		item = strings.TrimSpace(item)
		item = strings.TrimPrefix(item, "**")
		item = strings.TrimSuffix(item, "**")
		item = strings.TrimSpace(item)
		if item == "" {
			continue
		}
		if strings.HasPrefix(item, "⚠") || strings.HasPrefix(item, "阻塞") {
			b := strings.TrimPrefix(item, "⚠")
			b = strings.TrimSpace(b)
			b = strings.TrimPrefix(b, "阻塞")
			b = strings.TrimLeft(b, "：: ")
			b = strings.TrimSpace(b)
			if b != "" {
				blocked = append(blocked, b)
			}
		} else {
			next = append(next, item)
		}
	}
	return next, blocked
}

// extractHandoffOverview 从 Handoff 整段切出「概述」子段。
// 有「### 概述」：取它到下一个 `### ` 之间；无：取第一个 `### ` 之前（兼容旧单段写法）。
// 对齐 board.rs::extract_handoff_overview。
func extractHandoffOverview(section string) string {
	isSubHeading := func(l string) bool { return strings.HasPrefix(strings.TrimLeft(l, " \t"), "### ") }
	isOverview := func(l string) bool {
		if !isSubHeading(l) {
			return false
		}
		t := strings.TrimLeft(strings.TrimLeft(l, " \t"), "#")
		return strings.HasPrefix(strings.TrimSpace(t), "概述")
	}
	lines := strings.Split(section, "\n")
	hasOverview := false
	for _, l := range lines {
		if isOverview(l) {
			hasOverview = true
			break
		}
	}
	inOverview := false
	var collected []string
	for _, line := range lines {
		if isSubHeading(line) {
			inOverview = isOverview(line)
			continue
		}
		if hasOverview {
			if inOverview {
				collected = append(collected, line)
			}
		} else {
			collected = append(collected, line)
		}
	}
	return strings.Join(collected, "\n")
}

// extractHandoffDetail 切出「### 明细」子段原文（手机镜像页展示用）。
// 无明细 → ""。对齐 board.rs::extract_handoff_detail。
func extractHandoffDetail(indexMD string) string {
	section := extractSectionPrefix(indexMD, "当前接力点")
	if section == "" {
		return ""
	}
	isSubHeading := func(l string) bool { return strings.HasPrefix(strings.TrimLeft(l, " \t"), "### ") }
	isDetail := func(l string) bool {
		if !isSubHeading(l) {
			return false
		}
		t := strings.TrimLeft(strings.TrimLeft(l, " \t"), "#")
		return strings.HasPrefix(strings.TrimSpace(t), "明细")
	}
	inDetail := false
	var collected []string
	for _, line := range strings.Split(section, "\n") {
		if isSubHeading(line) {
			inDetail = isDetail(line)
			continue
		}
		if inDetail {
			collected = append(collected, line)
		}
	}
	return strings.TrimSpace(strings.Join(collected, "\n"))
}

// ───────────────────────── CHANGELOG 日期（对齐 board.rs::extract_changelog_date）─────────────────────────

// extractChangelogDate 取最靠上的 `## YYYY-MM-DD` 条目日期。无则 ""。
// 对齐 board.rs::extract_changelog_date。
func extractChangelogDate(changelogMD string) string {
	for _, line := range strings.Split(changelogMD, "\n") {
		t := strings.TrimSpace(line)
		if rest, ok := strings.CutPrefix(t, "## "); ok {
			token := strings.Fields(rest)
			if len(token) > 0 && isISODate(token[0]) {
				return token[0]
			}
		}
	}
	return ""
}

// isISODate 粗校验 YYYY-MM-DD。对齐 board.rs::is_iso_date。
func isISODate(s string) bool {
	if len(s) != 10 || s[4] != '-' || s[7] != '-' {
		return false
	}
	for i, c := range s {
		if i == 4 || i == 7 {
			continue
		}
		if c < '0' || c > '9' {
			return false
		}
	}
	return true
}

// ───────────────────────── 单项目组装（对齐 board.rs::parse_entry + load_push_board_from）─────────────────────────

// trioFiles 是某项目从 GitHub 拉到的三件套原文（任一可能为空 = 该文件缺失）。
type trioFiles struct {
	agents    string
	index     string
	changelog string
}

// buildCard 把三件套原文 + registry 元信息组装成一张卡片。永不 panic。
// 对齐 board.rs::parse_entry 的「已接入 vs 未接入」判定 + load_push_board_from 的详情拼装。
func buildCard(id, name string, pinned bool, files trioFiles) ProjectCard {
	card := ProjectCard{
		ID: id, Name: name, Pinned: pinned,
		Status: "unknown",
		Stages: []string{}, Next: []string{}, BlockedBy: []string{},
		StageItems: []StageItem{},
	}

	meta := parseAgentsMeta(files.agents)
	stageItems := extractStageList(files.changelog)

	// 「已接入看板」判定：有 status frontmatter 或有阶段表（对齐 board.rs has_trio_data）
	hasTrioData := meta.status != "" || len(stageItems) > 0
	if !hasTrioData {
		card.Error = &ParseError{
			Kind:    "missing",
			Message: "未接入看板：请在 AGENTS.md frontmatter 加 status，CHANGELOG.md 加「## 项目阶段」（可用 /outkanban 一键生成）",
		}
		return card
	}

	status := meta.status
	if status == "" {
		status = "active"
	}
	next, blocked := extractHandoff(files.index)
	doneCount := 0
	for _, s := range stageItems {
		if s.Done {
			doneCount++
		}
	}
	stageNames := make([]string, 0, len(stageItems))
	for _, s := range stageItems {
		stageNames = append(stageNames, s.Name)
	}
	if len(next) == 0 {
		next = []string{}
	}
	if len(blocked) == 0 {
		blocked = []string{}
	}

	card.Desc = meta.desc
	card.Status = status
	card.Stages = stageNames
	card.CurrentStage = int64(doneCount + 1)
	card.OverallProgress = computeProgressFromStages(stageItems, status)
	card.Next = next
	card.BlockedBy = blocked
	card.Updated = extractChangelogDate(files.changelog)

	// 详情块（对齐 PushProject）
	card.Intro = extractSection(files.index, "项目简介")
	if arch := extractSection(files.index, "架构图"); arch != "" {
		card.ArchMermaid = extractMermaid(arch)
	}
	card.StageItems = stageItems
	card.HandoffDetail = extractHandoffDetail(files.index)
	return card
}

// ───────────────────────── 汇总 + 排序（对齐 board.rs::build_summary / sort_cards）─────────────────────────

type Summary struct {
	Active int `json:"active"`
	Paused int `json:"paused"`
	Done   int `json:"done"`
	Error  int `json:"error"`
}

func buildSummary(cards []ProjectCard) Summary {
	var s Summary
	for _, c := range cards {
		if c.Error != nil {
			s.Error++
			continue
		}
		switch c.Status {
		case "active":
			s.Active++
		case "paused":
			s.Paused++
		case "done":
			s.Done++
		}
	}
	return s
}

// sortCards：done 排末尾，pinned 置顶，再按 name。对齐 board.rs::sort_cards。
func sortCards(cards []ProjectCard) {
	// 稳定排序：先 name，再 pinned，再 done，逐层叠（Go sort.SliceStable 保序）
	stableSortBy(cards, func(a, b ProjectCard) int {
		aDone, bDone := boolToInt(a.Status == "done"), boolToInt(b.Status == "done")
		if aDone != bDone {
			return aDone - bDone // done(1) 排后
		}
		ap, bp := boolToInt(a.Pinned), boolToInt(b.Pinned)
		if ap != bp {
			return bp - ap // pinned(1) 排前
		}
		return strings.Compare(a.Name, b.Name)
	})
}
