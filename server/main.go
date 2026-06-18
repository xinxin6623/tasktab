// tasktab-board 服务端 —— 极简看板镜像服务（纯标准库，零外部依赖）
//
// 中文说明（重要逻辑）：
//   这是 TaskBoard「手机查看」功能的服务端。它本身零智能、不解析三件套——
//   解析仍由 Mac App 的 Rust 后端做，App 把解析好的 board.json 单向 POST 过来，
//   本服务只负责「收下来存好 + 静态托管看板网页」。设计目标：跑在 4GB ECS 上，
//   常驻内存极小（纯标准库单二进制，约几 MB）。
//
// 路由：
//   POST /ingest      接收 App 推来的 board JSON，原子写入 dataPath（全公开，按 James 选择不鉴权）
//   GET  /board.json  返回当前 board JSON（网页轮询拉取）
//   GET  /            静态看板网页（web/index.html）
//   GET  /healthz     健康检查
//
// 配置（环境变量，均有缺省）：
//   TB_ADDR     监听地址，默认 ":8787"
//   TB_DATA     board.json 落盘路径，默认 "./data/board.json"
//   TB_WEB      静态网页目录，默认 "./web"
package main

import (
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

	mux := http.NewServeMux()

	// POST /ingest —— 收 App 推来的 board JSON，原子写盘（写临时文件 + rename，杜绝半截写入）
	mux.HandleFunc("/ingest", func(w http.ResponseWriter, r *http.Request) {
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
