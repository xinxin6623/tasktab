// github.go —— 从 GitHub API 拉取各项目三件套 + 最新 commit，聚合成 board.json
//
// 中文说明（重要逻辑 / 架构边界）：
//   这是新「设备间同步」方案的服务端核心。数据流：
//     两台设备各自 push 三件套到 GitHub 各 repo → 本服务定时用 GitHub API 拉每个 repo 的
//     AGENTS/INDEX/CHANGELOG 原文 + 最新 commit SHA → parse.go 解析 → 聚合 board.json。
//   各端（桌面 App / 手机网页）只读这份 board.json。「文件同步到 GitHub = 过了看板」。
//
//   为什么用 GitHub API 而不 clone：服务器只需读三份 markdown，API 拉文件更轻、不落盘、
//   不必管多仓工作区。代价是受 API 限流（带 token 5000 次/小时，对几个 repo+60s 轮询绰绰有余）。
//
// 配置（环境变量）：
//   TB_REGISTRY   registry.yaml 内容来源。两种取值：
//                   - 形如 "owner/repo@branch:path"   → 从 GitHub 拉 registry（推荐，单一真相）
//                   - 本地路径                          → 读本地文件（本地联调用）
//   TB_GH_TOKEN   GitHub PAT（读私有 repo 必需）。未设则仅能读公开 repo 且限流极低。
//   TB_POLL_SEC   轮询间隔秒，默认 60。
package main

import (
	"encoding/base64"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"os"
	"strconv"
	"strings"
	"sync"
	"time"
)

// ───────────────────────── registry schema（对齐 02 §1.2 + 新增 github 字段）─────────────────────────

type registryEntry struct {
	ID     string `yaml:"id"`
	Name   string `yaml:"name"`
	Path   string `yaml:"path"`
	Pinned bool   `yaml:"pinned"`
	Added  string `yaml:"added"`
	Github string `yaml:"github"` // 新增：owner/repo@branch，服务器据此拉文件；空则该项目跳过镜像
}

type registry struct {
	Version  int             `yaml:"version"`
	Projects []registryEntry `yaml:"projects"`
}

// ghSource 解析后的 GitHub 来源坐标。
type ghSource struct {
	owner  string
	repo   string
	branch string // 空 = 用仓库默认分支
}

// parseGithubField 解析 "owner/repo@branch" / "owner/repo"。失败返回 ok=false（该项目跳过）。
func parseGithubField(s string) (ghSource, bool) {
	s = strings.TrimSpace(s)
	if s == "" {
		return ghSource{}, false
	}
	branch := ""
	if i := strings.LastIndex(s, "@"); i >= 0 {
		branch = s[i+1:]
		s = s[:i]
	}
	parts := strings.SplitN(s, "/", 2)
	if len(parts) != 2 || parts[0] == "" || parts[1] == "" {
		return ghSource{}, false
	}
	return ghSource{owner: parts[0], repo: parts[1], branch: branch}, true
}

// ───────────────────────── GitHub API 客户端 ─────────────────────────

type ghClient struct {
	token string
	http  *http.Client
}

func newGHClient(token string) *ghClient {
	return &ghClient{token: token, http: &http.Client{Timeout: 20 * time.Second}}
}

func (c *ghClient) do(url string) ([]byte, int, error) {
	req, err := http.NewRequest(http.MethodGet, url, nil)
	if err != nil {
		return nil, 0, err
	}
	req.Header.Set("Accept", "application/vnd.github+json")
	req.Header.Set("X-GitHub-Api-Version", "2022-11-28")
	if c.token != "" {
		req.Header.Set("Authorization", "Bearer "+c.token)
	}
	resp, err := c.http.Do(req)
	if err != nil {
		return nil, 0, err
	}
	defer resp.Body.Close()
	body, err := io.ReadAll(io.LimitReader(resp.Body, 16<<20))
	return body, resp.StatusCode, err
}

// fetchFile 读 repo 指定路径文件内容（Contents API，base64 解码）。
// 404 → ("", nil)（文件缺失是正常情况，由解析层防御性降级）；其他错误返回 err。
func (c *ghClient) fetchFile(src ghSource, path string) (string, error) {
	url := fmt.Sprintf("https://api.github.com/repos/%s/%s/contents/%s", src.owner, src.repo, path)
	if src.branch != "" {
		url += "?ref=" + src.branch
	}
	body, code, err := c.do(url)
	if err != nil {
		return "", err
	}
	if code == http.StatusNotFound {
		return "", nil // 文件不存在 → 空（防御性）
	}
	if code != http.StatusOK {
		return "", fmt.Errorf("GitHub contents %s/%s/%s 返回 %d: %s", src.owner, src.repo, path, code, snippet(body))
	}
	var payload struct {
		Content  string `json:"content"`
		Encoding string `json:"encoding"`
	}
	if err := json.Unmarshal(body, &payload); err != nil {
		return "", err
	}
	if payload.Encoding == "base64" {
		// GitHub 在 content 里插了换行，DecodeString 不容忍，先去掉
		raw, err := base64.StdEncoding.DecodeString(strings.ReplaceAll(payload.Content, "\n", ""))
		if err != nil {
			return "", err
		}
		return string(raw), nil
	}
	return payload.Content, nil
}

// fetchLatestCommit 取该分支最新 commit SHA（App 拿本地 HEAD 比对，做同步徽章）。
// 失败返回 ("", err)；空分支用 HEAD。
func (c *ghClient) fetchLatestCommit(src ghSource) (string, error) {
	ref := src.branch
	if ref == "" {
		ref = "HEAD"
	}
	url := fmt.Sprintf("https://api.github.com/repos/%s/%s/commits/%s", src.owner, src.repo, ref)
	body, code, err := c.do(url)
	if err != nil {
		return "", err
	}
	if code != http.StatusOK {
		return "", fmt.Errorf("GitHub commits %s/%s@%s 返回 %d: %s", src.owner, src.repo, ref, code, snippet(body))
	}
	var payload struct {
		SHA string `json:"sha"`
	}
	if err := json.Unmarshal(body, &payload); err != nil {
		return "", err
	}
	return payload.SHA, nil
}

// ───────────────────────── 聚合：registry → 拉取 → 解析 → Board ─────────────────────────

// Board 是聚合后推给各端的整盘数据（对齐 board.rs::PushBoard + 新增 generated_at）。
type Board struct {
	Projects      []ProjectCard `json:"projects"`
	Summary       Summary       `json:"summary"`
	RegistryError *string       `json:"registry_error"`
	GeneratedAt   string        `json:"generated_at"` // 服务器本轮聚合完成时间（RFC3339），各端显示「最后更新」
}

// aggregate 跑一轮完整聚合：解析 registry → 逐项目 GitHub 拉三件套+commit → 解析 → 排序汇总。
// 任一项目拉取/解析失败都降级为 error 卡片，绝不中断整盘（防御性铁律）。
// 各项目【并发】拉取（每个项目内的 4 个 API 也并发），把整盘聚合耗时从「串行累加」压到
// 「最慢单个项目」量级——repo 多时差异显著（实测 7 repo 串行 ~18s → 并发 ~3s）。
func aggregate(reg registry, gh *ghClient) Board {
	now := time.Now().UTC().Format(time.RFC3339)
	cards := make([]ProjectCard, len(reg.Projects))
	var wg sync.WaitGroup
	for i, e := range reg.Projects {
		wg.Add(1)
		go func(i int, e registryEntry) {
			defer wg.Done()
			cards[i] = aggregateOne(e, gh, now)
		}(i, e)
	}
	wg.Wait()
	sortCards(cards)
	return Board{
		Projects:    cards,
		Summary:     buildSummary(cards),
		GeneratedAt: now,
	}
}

// aggregateOne 聚合单个项目：拉三件套 + commit（二者并发）→ 解析组装。永不 panic。
func aggregateOne(e registryEntry, gh *ghClient, now string) ProjectCard {
	src, ok := parseGithubField(e.Github)
	name := e.Name
	if name == "" {
		name = e.ID
	}
	if !ok {
		// 没配 github：该项目无法走 GitHub 镜像，降级提示（而非静默消失）
		return ProjectCard{
			ID: e.ID, Name: name, Pinned: e.Pinned, Status: "unknown",
			Stages: []string{}, Next: []string{}, BlockedBy: []string{}, StageItems: []StageItem{},
			Error: &ParseError{Kind: "missing", Message: "未配置 github 字段，无法镜像到看板（registry 该项目加 github: owner/repo@branch）"},
		}
	}

	// 三件套 + commit 并发拉取
	var files trioFiles
	var ferr, cerr error
	var commit string
	var w sync.WaitGroup
	w.Add(2)
	go func() { defer w.Done(); files, ferr = fetchTrio(gh, src) }()
	go func() { defer w.Done(); commit, cerr = gh.fetchLatestCommit(src) }()
	w.Wait()

	card := buildCard(e.ID, name, e.Pinned, files)
	card.Github = e.Github
	card.Branch = src.branch
	card.Commit = commit
	card.SyncedAt = now
	if ferr != nil && card.Error == nil {
		card.Error = &ParseError{Kind: "missing", Message: "GitHub 拉取三件套失败: " + ferr.Error()}
	}
	if cerr != nil {
		card.Commit = "" // commit 拉不到不致命：徽章无法比对，但卡片内容仍可用
	}
	return card
}

// fetchTrio 拉一个 repo 的三件套原文（三份【并发】）。各自防御性：单份失败不影响其余，
// 只要不是全部失败就返回 nil err（让解析层按缺省降级）。
func fetchTrio(gh *ghClient, src ghSource) (trioFiles, error) {
	var f trioFiles
	type res struct {
		content string
		err     error
	}
	paths := []struct {
		path string
		dst  *string
	}{
		{"AGENTS.md", &f.agents},
		{"INDEX.md", &f.index},
		{"CHANGELOG.md", &f.changelog},
	}
	results := make([]res, len(paths))
	var wg sync.WaitGroup
	for i, item := range paths {
		wg.Add(1)
		go func(i int, path string) {
			defer wg.Done()
			c, err := gh.fetchFile(src, path)
			results[i] = res{c, err}
		}(i, item.path)
	}
	wg.Wait()

	var firstErr error
	var okCount int
	for i, r := range results {
		if r.err != nil {
			if firstErr == nil {
				firstErr = r.err
			}
			continue
		}
		*paths[i].dst = r.content
		okCount++
	}
	// 三份全失败才算整体失败；部分成功交给解析层防御性降级
	if okCount == 0 && firstErr != nil {
		return f, firstErr
	}
	return f, nil
}

// ───────────────────────── registry 来源：GitHub 或本地 ─────────────────────────

// loadRegistry 按 TB_REGISTRY 取 registry：GitHub 坐标(owner/repo@branch:path) 或本地路径。
func loadRegistry(gh *ghClient, spec string) (registry, error) {
	var reg registry
	raw, err := readRegistryRaw(gh, spec)
	if err != nil {
		return reg, err
	}
	if err := yamlUnmarshal(raw, &reg); err != nil {
		return reg, fmt.Errorf("registry.yaml 解析失败: %w", err)
	}
	return reg, nil
}

// readRegistryRaw 取 registry 原文：含 ":" 且前半是 owner/repo@branch → GitHub；否则本地文件。
func readRegistryRaw(gh *ghClient, spec string) ([]byte, error) {
	if src, path, ok := parseRegistrySpec(spec); ok {
		content, err := gh.fetchFile(src, path)
		if err != nil {
			return nil, fmt.Errorf("从 GitHub 拉 registry 失败: %w", err)
		}
		if content == "" {
			return nil, fmt.Errorf("GitHub registry 为空或不存在: %s", spec)
		}
		return []byte(content), nil
	}
	return os.ReadFile(spec)
}

// parseRegistrySpec 解析 "owner/repo@branch:path"。无 ":path" 或不像 GitHub 坐标 → ok=false。
func parseRegistrySpec(spec string) (ghSource, string, bool) {
	i := strings.LastIndex(spec, ":")
	if i < 0 {
		return ghSource{}, "", false
	}
	coord, path := spec[:i], spec[i+1:]
	// Windows 盘符之类不会带 "@" 与 "/"，用 parseGithubField 校验
	src, ok := parseGithubField(coord)
	if !ok || path == "" {
		return ghSource{}, "", false
	}
	return src, path, true
}

// ───────────────────────── 辅助 ─────────────────────────

func snippet(b []byte) string {
	s := string(b)
	if len(s) > 200 {
		return s[:200]
	}
	return s
}

func pollSeconds() int {
	if v := os.Getenv("TB_POLL_SEC"); v != "" {
		if n, err := strconv.Atoi(v); err == nil && n >= 5 {
			return n
		}
	}
	return 60
}
