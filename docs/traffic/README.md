# 流量增强版

在原有 Clash Verge Rev 内提供“流量”页面，侧重经过 Clash 的应用流量与长期记录。主值、排序和筛选以代理流量为准，括号显示包含 Direct 的总量。

## 使用 v12（2.6.0-otool.12）

从 [本仓库 Releases](https://github.com/onexs-xsi/otool_clash-verge-rev/releases) 下载 Windows x64 完整便携包并解压。入口为 `clash-verge.exe`，本地构建入口为 `releases/archive/v12/clash-verge.exe`。先从系统托盘彻底退出旧版，再运行新版；移动时保留整个版本文件夹。设置、订阅和既有 SQLite 历史继续沿用，WSL 监测保持只读。

v12 已合并主仓库 `dev` 截至 `d7ac6b5b` 的更新，上游版本为 2.6.0 dev。设置、首页和更新提示共用本仓库的发布检查；「自动检查更新」可在设置中关闭，手动检查仍可使用。设置页新增内置的 onexs-xsi GitHub 头像，离线也可显示。

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

`12` 是当前增强版编号。后续编号必须大于已有归档，并同步修改 `traffic.edition.json` 的 `edition` 和 `upstream.version`，以及 `package.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`Cargo.lock` 的应用版本为 `上游版本-otool.编号`。打包脚本会拒绝版本不一致的发布。输出为 `releases/archive/vN`，不覆盖或移动旧版文件；`release.json` 记录版本、标签和文件校验值。

高级选项 `--skip-build` 只整理已经构建完成的原生 release 文件，不证明二进制与当前源码或版本一致。正常发布使用完整构建命令。

## 发布与更新

`traffic.edition.json` 配置本仓库身份和上游基线，CI 可通过 `VITE_TRAFFIC_REPOSITORY` 指定构建仓库。Release 标签格式为 `v2.6.0-otool.12`，完整便携包名称为 `otool_clash-verge-rev_2.6.0_traffic-v12_windows-x64_portable.zip`。检查器读取发布列表，包含已发布的预发布版，仅提供本仓库内已上传完整 Windows x64 便携包的匹配版本；草稿、上游标签和不完整包不参与更新比较。

当前版本继续共享上游应用标识、AppData 和服务，便于现有用户延续记录；它不是与官方客户端隔离并行运行的安装。若以后更换标识，应单独设计数据和服务迁移。

检测到新版后可在程序中查看更新说明，并在浏览器下载完整包。整包应用更新采用手动切换免安装目录；内核更新仍由原有流程负责。程序不嵌入 GitHub 凭据。将来若支持自动替换程序，需要另行配置本分支的签名和安装流程。
