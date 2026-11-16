# 流量版发布目录

- `archive/v12/clash-verge.exe`：v12（2.6.0-otool.12）的直接运行入口。
- `archive/v*/`：每个版本拥有独立的完整免安装目录，移动时保留整个版本目录。

所有新版本直接发布到自己的 `archive/vN`，不再使用 `current`。打包脚本不读取、覆盖或移动既有版本中的文件，因此可以继续运行旧版并构建新版。已有版本号不能重新发布，版本及文件校验值记录在每个发布包的 `release.json` 中。

v6 已核对归档至 `archive/v6`，根目录旧副本已清理。整理记录位于 `target/reports/traffic-v6-archive-cleanup.json`。

订阅、设置、日志和流量记录仍保存在 Windows 用户的 AppData 目录，不放入发布目录。
