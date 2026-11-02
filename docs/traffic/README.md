# 流量增强版

在原有 Clash Verge Rev 内提供“流量”页面，侧重经过 Clash 的应用流量与长期记录。主值、排序和筛选以代理流量为准，括号显示包含 Direct 的总量。

## 使用 v11

免安装入口位于 `releases/archive/v11/clash-verge.exe`。先从系统托盘彻底退出旧版，再运行新版；移动时保留整个版本文件夹。设置、订阅和 v7–v10 SQLite 历史继续沿用，WSL 监测保持只读。

v11 已合并主仓库 `dev` 截至 `f149604e` 的更新，宿主版本为 2.5.10。流量接口接入主线的 Specta 类型生成与检查，既有记录及偏好继续沿用。

数据库路径右侧的括号显示主数据库文件的实际大小；WAL 写入日志、可回收空间及在线备份位于“记录管理”。大小不通过扫描统计记录计算。

## 目录

```text
src/features/traffic/          页面、组件、纯逻辑与读取生命周期
src/services/traffic/          命令与类型、图标请求及导出
src-tauri/src/core/traffic/    采集、SQLite、应用识别及只读 WSL 探针
src/locales/*/traffic.json    独立文案，其他语言按宿主规则回退
scripts/traffic/              便携版打包、测试和维护工具
docs/traffic/                 维护说明与集成边界
releases/archive/vN/          各版本完整运行目录，不提交 Git
target/reports/               本地检查报告，不提交 Git
```

上游目录及路径约定保留。[ARCHITECTURE.md](ARCHITECTURE.md) 说明每层职责和兼容约束，[UPSTREAM.md](UPSTREAM.md) 说明如何同步主线；`integration.json` 逐项列出必须保留的宿主改动及其原因。历史临时规划和一次性整理脚本已移入 `target/archive`。

## 开发、验证和构建

先按上游方式准备依赖和内核资源。检查功能边界及兼容行为：

```powershell
pnpm traffic:check
pnpm arch:check
pnpm typecheck
pnpm test
./scripts/traffic/test-native.ps1
pnpm traffic:test:package
pnpm portable --version 12
```

`12` 是 v11 之后的示例，使用未发布过的递增编号。打包直接生成 `releases/archive/vN`，不占用 `current`，不覆盖或移动旧版文件。发布包附带文件校验清单；文案源为 `portable-notes.txt`。

高级选项 `--skip-build` 只整理已经构建完成的原生 release 文件，不证明二进制与当前源码或版本一致。正常发布使用完整构建命令。

## 准备公开 fork

仓库名建议 `clash-verge-rev-with-traffic`。创建 fork 后，在 `traffic.edition.json` 填写真实仓库地址；目前不硬编码账号。GitHub 的 Traffic edition 工作流可以手动生成 Windows 免安装制品，实际云端执行需要在新仓库中验证。

当前版本继续共享上游应用标识、AppData 和服务，便于现有用户延续记录；它不是与官方客户端隔离并行运行的安装。若以后更换标识，应单独设计数据和服务迁移。

整包应用更新采用手动切换免安装目录，避免上游安装包覆盖增强功能。内核更新仍由原有流程负责；启用分支自己的自动整包更新之前，需要独立的发布源、签名和验证。
