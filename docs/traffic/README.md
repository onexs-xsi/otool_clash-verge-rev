# 流量增强版

在原有 Clash Verge Rev 内提供“流量”页面，侧重经过 Clash 的应用流量与长期记录。主值、排序和筛选以代理流量为准，括号显示包含 Direct 的总量。

## 使用 v14（2.6.0-otool.14）

从 [本仓库 Releases](https://github.com/onexs-xsi/otool_clash-verge-rev/releases) 下载 Windows x64 完整便携包并解压。入口为 `clash-verge.exe`，本地构建入口为 `releases/archive/v14/clash-verge.exe`。先从系统托盘彻底退出旧版，再运行新版；移动时保留整个版本文件夹。设置、订阅和既有 SQLite 历史继续沿用，WSL 监测保持只读。v12、v13 的公开下载已撤回。

v14 基于主仓库 `dev` 的 `d7ac6b5b`，上游版本为 2.6.0 dev。设置、首页和更新提示共用本仓库的发布检查；「自动检查更新」可在设置中关闭，手动检查仍可使用。设置页内置 onexs-xsi GitHub 头像，离线也可显示。

### 直接替换安装

发现新版后，「下载完整便携包」右侧提供「直接替换安装」。点击后自动下载、校验 GitHub 的 SHA-256、解压并验证包内所有文件。准备完成时展示当前程序完整路径、待安装程序的本地路径、GitHub 下载来源和旧版备份位置，点击「确认替换并重启」后才启动外部替换脚本。

脚本先检查目录权限，需要时请求 Windows 管理员授权。授权取消或预检失败时，当前程序保持运行，可以重试。脚本就绪后，程序正常保存设置和流量记录、停止内核并退出；脚本等待该进程退出后，备份原有程序文件、安装新版并重启。不会强制结束进程。替换失败时尝试恢复旧版，并保留错误结果供下次打开更新窗口查看。

此入口适用于 Windows x64 的完整 OTool 便携目录（含匹配版本的 release.json），只替换发布包管理的程序文件，其他自有文件和 AppData 不参与替换。备份保存在程序目录的 .otool-backups。该能力从 v14 开始提供，v12、v13 需先升级到 v14 才有此按钮。

### 启动时的服务更新

程序启动会检查已经安装的系统服务。当前要求 `2.8.1`；旧服务（例如 `2.7.6`）或异常服务会先显示更新/修复弹窗，等待用户选择。点击「更新服务」或「修复服务」后，随包安装器请求 Windows 管理员授权，完成后自动重启内核；确认服务可用且内核进入服务模式后才提示成功。

取消 Windows 授权后可在原弹窗重试，也可选择「本次使用用户模式」。启动检查需要处理的服务会等待用户选择；静默启动也会显示授权窗口，轻量模式不会在等待选择或授权过程中销毁它。服务已兼容时直接按原启动设置运行；未安装服务且无需服务权限时仍可使用用户模式。

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
./scripts/traffic/test-portable-update.ps1
pnpm traffic:test:package
pnpm portable --version 14
```

`14` 是当前增强版编号。后续编号必须大于已有归档，并同步修改 `traffic.edition.json` 的 `edition` 和 `upstream.version`，以及 `package.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`Cargo.lock` 的应用版本为 `上游版本-otool.编号`。打包脚本会拒绝版本不一致的发布。输出为 `releases/archive/vN`，不覆盖或移动旧版文件；`release.json` 记录版本、标签和文件校验值。

高级选项 `--skip-build` 只整理已经构建完成的原生 release 文件，不证明二进制与当前源码或版本一致。正常发布使用完整构建命令。

## 发布与更新

`traffic.edition.json` 配置本仓库身份和上游基线，CI 可通过 `VITE_TRAFFIC_REPOSITORY` 指定构建仓库。Release 标签格式为 `v2.6.0-otool.14`，完整便携包名称为 `otool_clash-verge-rev_2.6.0_traffic-v14_windows-x64_portable.zip`。检查器读取发布列表，包含已发布的预发布版，仅提供本仓库内已上传完整 Windows x64 便携包的匹配版本；草稿、上游标签和不完整包不参与更新比较。

当前版本继续共享上游应用标识、AppData 和服务，便于现有用户延续记录；它不是与官方客户端隔离并行运行的安装。若以后更换标识，应单独设计数据和服务迁移。

检测到新版后可查看更新说明、下载完整包或二次确认后直接替换安装。替换器仅接受编译时 traffic.edition.json 所指定仓库的已发布包，并验证 GitHub 提供的 SHA-256 及包内文件清单；不会调用上游应用安装器，也不嵌入 GitHub 凭据。包目前未做独立代码签名，信任边界为 HTTPS 和本仓库发布权限。内核更新仍由原有流程负责。
