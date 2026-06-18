// parse_test.go —— Go 解析器与 board.rs 的一致性测试。
// 样例数据刻意复用 board.rs 单测里的输入，断言两端输出一致，守住 SSOT。
package main

import "testing"

func TestExtractFrontmatter(t *testing.T) {
	fm, ok := extractFrontmatter("---\nproject: x\nstatus: active\n---\n# body\n")
	if !ok || !contains(fm, "project: x") || contains(fm, "body") {
		t.Fatalf("frontmatter 提取错误: %q ok=%v", fm, ok)
	}
	if _, ok := extractFrontmatter("# 没有 frontmatter\n正文"); ok {
		t.Fatal("无 frontmatter 应返回 false")
	}
}

func TestParseAgentsMeta(t *testing.T) {
	m := parseAgentsMeta("---\ntrio: standard-v2\nstatus: paused\ndesc: 一句话描述\n---\n# 正文")
	if m.status != "paused" || m.desc != "一句话描述" {
		t.Fatalf("got status=%q desc=%q", m.status, m.desc)
	}
	m2 := parseAgentsMeta("# 没有 frontmatter")
	if m2.status != "" || m2.desc != "" {
		t.Fatalf("无 frontmatter 应全空, got %+v", m2)
	}
}

func TestExtractStageList(t *testing.T) {
	md := "## 项目阶段\n- [x] 需求与架构 — 明确方案\n- [ ] provider 注入 — 专属 home\n- [X] 备份机制\n非列表行\n\n## 2026-06-13 #feat scope:x - 变更流水\n"
	items := extractStageList(md)
	if len(items) != 3 {
		t.Fatalf("期望 3 项, got %d", len(items))
	}
	if items[0] != (StageItem{"需求与架构", "明确方案", true}) {
		t.Fatalf("item0=%+v", items[0])
	}
	if items[1] != (StageItem{"provider 注入", "专属 home", false}) {
		t.Fatalf("item1=%+v", items[1])
	}
	if items[2] != (StageItem{"备份机制", "", true}) {
		t.Fatalf("item2=%+v", items[2])
	}
	// 无块 → 空
	if len(extractStageList("## 别的\n- [x] 不该被抓")) != 0 {
		t.Fatal("无项目阶段块应为空")
	}
}

func TestComputeProgress(t *testing.T) {
	s := func(done bool) StageItem { return StageItem{"x", "", done} }
	if got := computeProgressFromStages([]StageItem{s(true), s(true), s(false), s(false)}, "active"); got != 50 {
		t.Fatalf("2/4 应为 50, got %v", got)
	}
	if got := computeProgressFromStages([]StageItem{s(false)}, "done"); got != 100 {
		t.Fatalf("done 强制 100, got %v", got)
	}
	if got := computeProgressFromStages(nil, "active"); got != 0 {
		t.Fatalf("空表应为 0, got %v", got)
	}
}

func TestExtractHandoff(t *testing.T) {
	// 旧单段写法
	idx := "# T\n\n## 当前接力点 (Handoff)\n- 写集成测试\n- ⚠ 阻塞：等待上游接口\n- 打包发布\n\n## 项目定位\nxxx"
	next, blocked := extractHandoff(idx)
	if len(next) != 2 || next[0] != "写集成测试" || next[1] != "打包发布" {
		t.Fatalf("next=%v", next)
	}
	if len(blocked) != 1 || blocked[0] != "等待上游接口" {
		t.Fatalf("blocked=%v", blocked)
	}
	// 概述/明细两段：明细被忽略
	idx2 := "## 当前接力点 (Handoff)\n\n### 概述\n- 打包发布\n- ⚠ 阻塞：等待签名证书\n\n### 明细\n跑 install.sh。\n- 这行不该当 next\n- ⚠ 这行不该进 blocked\n"
	n2, b2 := extractHandoff(idx2)
	if len(n2) != 1 || n2[0] != "打包发布" || len(b2) != 1 || b2[0] != "等待签名证书" {
		t.Fatalf("两段解析错: next=%v blocked=%v", n2, b2)
	}
	// 纯文本加粗写法
	idx3 := "## 当前接力点 (Handoff)\n\n> 只保留最新一条。\n\n### 概述\n**跑 ./scripts/install.sh 正式打包发布**\n**⚠ 阻塞：等待签名证书**\n\n### 明细\n背景。\n"
	n3, b3 := extractHandoff(idx3)
	if len(n3) != 1 || n3[0] != "跑 ./scripts/install.sh 正式打包发布" {
		t.Fatalf("加粗 next=%v", n3)
	}
	if len(b3) != 1 || b3[0] != "等待签名证书" {
		t.Fatalf("加粗 blocked=%v", b3)
	}
}

func TestExtractChangelogDate(t *testing.T) {
	cl := "# CHANGELOG\n\n## 格式规范\n\n## 2026-06-14 #feat scope:x - 主题\n- Why: ...\n## 2026-06-01 #fix - 旧的\n"
	if got := extractChangelogDate(cl); got != "2026-06-14" {
		t.Fatalf("got %q", got)
	}
	if got := extractChangelogDate("# 无日期\n## 格式规范"); got != "" {
		t.Fatalf("无日期应空, got %q", got)
	}
}

func TestExtractSectionAndMermaid(t *testing.T) {
	md := "# T\n\n## 项目简介\n这是一句简介。\n第二行。\n\n## 架构图\n```mermaid\nflowchart TD\n  A --> B\n```\n"
	if got := extractSection(md, "项目简介"); got != "这是一句简介。\n第二行。" {
		t.Fatalf("intro=%q", got)
	}
	arch := extractSection(md, "架构图")
	if got := extractMermaid(arch); got != "flowchart TD\n  A --> B" {
		t.Fatalf("mermaid=%q", got)
	}
}

func TestBuildCardTrioVsMissing(t *testing.T) {
	// 已接入：active + 4 阶段勾 3 → 75%
	card := buildCard("voice", "语音管线", false, trioFiles{
		agents:    "---\nstatus: active\ndesc: 语音管线\n---\n",
		index:     "## 当前接力点 (Handoff)\n- 打断信号去抖逻辑\n",
		changelog: "## 项目阶段\n- [x] 需求与架构\n- [x] ASR 接入\n- [x] barge-in 状态机\n- [ ] 联调打包\n\n## 2026-06-13 #feat - x\n",
	})
	if card.Error != nil {
		t.Fatalf("不该降级: %+v", card.Error)
	}
	if card.OverallProgress != 75 {
		t.Fatalf("进度应 75, got %v", card.OverallProgress)
	}
	if card.Status != "active" || card.Desc != "语音管线" {
		t.Fatalf("status/desc 错: %+v", card)
	}
	if len(card.Next) != 1 || card.Next[0] != "打断信号去抖逻辑" {
		t.Fatalf("next=%v", card.Next)
	}
	if card.Updated != "2026-06-13" {
		t.Fatalf("updated=%q", card.Updated)
	}

	// 未接入：空三件套 → missing 降级
	bad := buildCard("broken", "坏项目", false, trioFiles{})
	if bad.Error == nil || bad.Error.Kind != "missing" {
		t.Fatalf("应 missing 降级, got %+v", bad.Error)
	}

	// frontmatter 损坏但有阶段表 → 仍渲染，status 缺省 active
	broken := buildCard("bad", "Bad", false, trioFiles{
		agents:    "---\nstatus: : : broken\n  bad indent\n---\n",
		changelog: "## 项目阶段\n- [x] a\n- [ ] b\n\n## 2026-06-13 #feat - x\n",
	})
	if broken.Error != nil {
		t.Fatalf("损坏 frontmatter 不该崩: %+v", broken.Error)
	}
	if broken.Status != "active" || broken.OverallProgress != 50 {
		t.Fatalf("缺省 active+50%%, got status=%q prog=%v", broken.Status, broken.OverallProgress)
	}
}

func TestParseGithubField(t *testing.T) {
	src, ok := parseGithubField("xinxin6623/tasktab@main")
	if !ok || src.owner != "xinxin6623" || src.repo != "tasktab" || src.branch != "main" {
		t.Fatalf("got %+v ok=%v", src, ok)
	}
	src2, ok2 := parseGithubField("owner/repo")
	if !ok2 || src2.branch != "" {
		t.Fatalf("无分支应 ok, got %+v", src2)
	}
	if _, ok := parseGithubField(""); ok {
		t.Fatal("空串应 false")
	}
	if _, ok := parseGithubField("noslash"); ok {
		t.Fatal("无斜杠应 false")
	}
}

func TestParseRegistrySpec(t *testing.T) {
	src, path, ok := parseRegistrySpec("xinxin6623/tasktab@main:cli/registry.yaml")
	if !ok || src.repo != "tasktab" || path != "cli/registry.yaml" {
		t.Fatalf("got src=%+v path=%q ok=%v", src, path, ok)
	}
	// 本地路径不该被当 GitHub 坐标
	if _, _, ok := parseRegistrySpec("/Users/x/registry.yaml"); ok {
		t.Fatal("本地绝对路径不该判为 GitHub spec")
	}
	if _, _, ok := parseRegistrySpec("./registry.yaml"); ok {
		t.Fatal("相对路径不该判为 GitHub spec")
	}
}

func TestSortCards(t *testing.T) {
	cards := []ProjectCard{
		{ID: "a", Name: "Alpha", Status: "done"},
		{ID: "b", Name: "Beta", Status: "paused", Pinned: true},
		{ID: "c", Name: "Voice", Status: "active"},
	}
	sortCards(cards)
	if cards[0].ID != "b" {
		t.Fatalf("pinned 应置顶, got %s", cards[0].ID)
	}
	if cards[len(cards)-1].ID != "a" {
		t.Fatalf("done 应末尾, got %s", cards[len(cards)-1].ID)
	}
}

func contains(s, sub string) bool {
	return len(s) >= len(sub) && (s == sub || indexOf(s, sub) >= 0)
}
func indexOf(s, sub string) int {
	for i := 0; i+len(sub) <= len(s); i++ {
		if s[i:i+len(sub)] == sub {
			return i
		}
	}
	return -1
}
