// tasktab-board 服务端 —— 看板镜像服务（GitHub 聚合 + 静态托管）
//
// 中文说明（重要逻辑 / 架构）：
//   这是 TaskBoard「设备间同步 / 手机查看」功能的服务端。两台设备各自把三件套 push 到
//   GitHub 各 repo，本服务【定时用 GitHub API 拉各 repo 的三件套 + 最新 commit】，在服务器侧
//   用 parse.go（对齐 board.rs 契约）解析聚合成 board.json。各端（桌面 App / 手机网页）只读它。
//   「文件同步到 GitHub = 过了看板」。聚合循环见 github.go::aggregate。
//
//   兼容：旧的 POST /ingest（App 单向推）仅在【未配置 TB_REGISTRY 的兼容模式】下可用。
//   一旦启用聚合（设了 TB_REGISTRY），board.json 由聚合循环独占写入，/ingest 直接返回 409
//   拒绝——否则残留的旧版 App 仍可推送把聚合数据覆盖掉。
//
// 路由：
//   GET  /board.json  返回当前聚合的 board JSON（网页/桌面 App 轮询拉取）
//   POST /ingest      [兼容] 接收外部推来的 board JSON，原子写入；聚合模式下禁用（409）
//   GET  /            静态看板网页（web/index.html）
//   GET  /healthz     健康检查
//
// 配置（环境变量）：
//   TB_ADDR       监听地址，默认 ":8787"
//   TB_DATA       board.json 落盘路径，默认 "./data/board.json"
//   TB_WEB        静态网页目录，默认 "./web"
//   TB_REGISTRY   registry 来源："owner/repo@branch:path"(GitHub) 或本地路径。设置后启用聚合循环。
//   TB_GH_TOKEN   GitHub PAT（读私有 repo 必需）
//   TB_POLL_SEC   聚合轮询间隔秒，默认 60
package main

import (
	"context"
	"encoding/json"
	"io"
	"log"
	"net/http"
	"os"
	"path/filepath"
	"time"
)

func env(key, def string) string {
	if v := os.Getenv(key); v != "" {
		return v
	}
	return def
}

func main() {
	addr := env("TB_ADDR", ":8787")
	dataPath := env("TB_DATA", "./data/board.json")
	webDir := env("TB_WEB", "./web")

	// 确保数据目录存在
	if err := os.MkdirAll(filepath.Dir(dataPath), 0o755); err != nil {
		log.Fatalf("无法创建数据目录: %v", err)
	}

	// 聚合循环：配置了 TB_REGISTRY 即启用「GitHub 拉取 → 解析 → 写 board.json」后台循环。
	// 未配置则退回旧的纯 ingest 模式（向后兼容）。
	// aggregating 标志：聚合模式下 board.json 由聚合循环独占写入，/ingest 必须拒绝——
	// 否则残留的旧版 App 仍可 POST /ingest 把聚合数据覆盖掉（曾经的隐患）。
	aggregating := os.Getenv("TB_REGISTRY") != ""
	if aggregating {
		go runAggregateLoop(context.Background(), os.Getenv("TB_REGISTRY"), dataPath)
	} else {
		log.Printf("未设置 TB_REGISTRY，运行在兼容模式（仅 /ingest），不做 GitHub 聚合")
	}

	mux := http.NewServeMux()

	// POST /ingest —— [兼容] 收 App 推来的 board JSON，原子写盘（写临时文件 + rename，杜绝半截写入）。
	// 聚合模式下禁用：board.json 由聚合循环独占，收到 ingest 直接拒绝并打日志（防旧 push 源覆盖）。
	mux.HandleFunc("/ingest", func(w http.ResponseWriter, r *http.Request) {
		if aggregating {
			log.Printf("[ingest] 已拒绝：聚合模式下 /ingest 禁用（来源 %s，可能是残留的旧版 App 推送）", r.RemoteAddr)
			http.Error(w, "ingest disabled in aggregate mode", http.StatusConflict)
			return
		}
		if r.Method != http.MethodPost {
			http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
			return
		}
		// 限制 8MB，防止异常大 body
		body, err := io.ReadAll(io.LimitReader(r.Body, 8<<20))
		if err != nil {
			http.Error(w, "read body failed", http.StatusBadRequest)
			return
		}
		// 校验是合法 JSON（不解析结构，App 端结构即真相；这里只防垃圾写入）
		if !json.Valid(body) {
			http.Error(w, "body is not valid json", http.StatusBadRequest)
			return
		}
		if err := atomicWrite(dataPath, body); err != nil {
			log.Printf("[ingest] 写盘失败: %v", err)
			http.Error(w, "write failed", http.StatusInternalServerError)
			return
		}
		log.Printf("[ingest] 已更新 board.json（%d 字节）", len(body))
		w.WriteHeader(http.StatusNoContent)
	})

	// GET /board.json —— 网页轮询拉取当前数据。文件还不存在时返回空看板，网页不报错。
	mux.HandleFunc("/board.json", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json; charset=utf-8")
		w.Header().Set("Cache-Control", "no-store")
		b, err := os.ReadFile(dataPath)
		if err != nil {
			// 还没收到过任何推送：返回一个空但合法的看板，网页正常显示「暂无数据」
			_, _ = w.Write([]byte(`{"projects":[],"summary":{"active":0,"paused":0,"done":0,"error":0},"registry_error":null,"_empty":true}`))
			return
		}
		_, _ = w.Write(b)
	})

	mux.HandleFunc("/healthz", func(w http.ResponseWriter, r *http.Request) {
		_, _ = w.Write([]byte("ok"))
	})

	// 静态网页（GET / 与其余路径）。带 no-cache 保证更新 index.html 后手机能拿到新版。
	fs := http.FileServer(http.Dir(webDir))
	mux.Handle("/", noCache(fs))

	srv := &http.Server{
		Addr:              addr,
		Handler:           logRequests(mux),
		ReadHeaderTimeout: 10 * time.Second,
	}
	log.Printf("tasktab-board 启动：监听 %s，数据 %s，网页 %s", addr, dataPath, webDir)
	if err := srv.ListenAndServe(); err != nil {
		log.Fatalf("服务退出: %v", err)
	}
}

// runAggregateLoop 后台聚合循环：每 TB_POLL_SEC 秒拉一次 registry + 各 repo 三件套，
// 解析聚合后原子写入 dataPath。任一轮失败只打日志、保留上轮 board.json，绝不退出。
func runAggregateLoop(ctx context.Context, regSpec, dataPath string) {
	gh := newGHClient(os.Getenv("TB_GH_TOKEN"))
	interval := time.Duration(pollSeconds()) * time.Second
	if os.Getenv("TB_GH_TOKEN") == "" {
		log.Printf("[aggregate] ⚠ 未设置 TB_GH_TOKEN，私有 repo 将拉取失败、限流极低")
	}
	log.Printf("[aggregate] 启用 GitHub 聚合：registry=%s 间隔=%s", regSpec, interval)

	runOnce := func() {
		reg, err := loadRegistry(gh, regSpec)
		if err != nil {
			log.Printf("[aggregate] 读取 registry 失败（保留上轮数据）: %v", err)
			return
		}
		board := aggregate(reg, gh)
		data, err := json.Marshal(board)
		if err != nil {
			log.Printf("[aggregate] 序列化失败: %v", err)
			return
		}
		if err := atomicWrite(dataPath, data); err != nil {
			log.Printf("[aggregate] 写盘失败: %v", err)
			return
		}
		log.Printf("[aggregate] 已更新 board.json（%d 项目，%d 字节）", len(board.Projects), len(data))
	}

	runOnce() // 启动即拉一次，不等第一个 tick
	ticker := time.NewTicker(interval)
	defer ticker.Stop()
	for {
		select {
		case <-ctx.Done():
			return
		case <-ticker.C:
			runOnce()
		}
	}
}

// atomicWrite 原子写文件：写临时文件 → rename 覆盖。与 App / cra 的 registry 写入策略一致。
func atomicWrite(path string, data []byte) error {
	dir := filepath.Dir(path)
	tmp, err := os.CreateTemp(dir, ".board-*.tmp")
	if err != nil {
		return err
	}
	tmpName := tmp.Name()
	defer os.Remove(tmpName) // rename 成功后这是 no-op；失败时清理临时文件
	if _, err := tmp.Write(data); err != nil {
		tmp.Close()
		return err
	}
	if err := tmp.Close(); err != nil {
		return err
	}
	return os.Rename(tmpName, path)
}

func noCache(h http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Cache-Control", "no-cache")
		h.ServeHTTP(w, r)
	})
}

func logRequests(h http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		h.ServeHTTP(w, r)
		// 只记非静态资源的关键请求，避免日志噪音
		if r.URL.Path == "/ingest" {
			log.Printf("%s %s from %s", r.Method, r.URL.Path, r.RemoteAddr)
		}
	})
}
