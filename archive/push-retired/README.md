# push.rs（已退役 2026-06-18）

App 端「单向推送 board.json 到服务器」的旧实现。属于上一版「手机查看」方案
（App 推 → 服务器哑存储 → 手机只读）。

2026-06-18 改为「设备间同步」方案后退役：服务器改为自己从 GitHub API 聚合各 repo
三件套（server/github.go + parse.go），App 不再推送、改为只读拉取服务器 board.json
并显示同步徽章（app/src-tauri/src/sync.rs）。

保留此文件仅作历史快照，勿复活。新架构见 server/README.md 与 CLAUDE.md「设备间同步」节。
