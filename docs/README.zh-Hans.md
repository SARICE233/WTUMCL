> **WTUMCL** 是基于 [SJMCL](https://github.com/UNIkeEN/SJMCL) 的二次开发分支，由 **武汉纺织大学 MC 社区（WTUMC）** 维护。
> 原项目版权归 SJMCL 团队（© 2024-2026）所有，以 GPLv3 及附加条款（见 [LICENSE.EXTRA](../LICENSE.EXTRA)）发布。
> 上游仓库：https://github.com/UNIkeEN/SJMCL · 本分支仓库：https://github.com/SARICE233/WTUMCL

<img src="figs/banner.png" alt="WTUMCL" width="640" />

[![Test Build](https://img.shields.io/github/actions/workflow/status/SARICE233/WTUMCL/test.yml?label=test%20build&logo=github&style=for-the-badge)](https://github.com/SARICE233/WTUMCL/blob/main/.github/workflows/test.yml)
![Downloads](https://img.shields.io/github/downloads/SARICE233/WTUMCL/total?logo=github&style=for-the-badge)
![Stars](https://img.shields.io/github/stars/SARICE233/WTUMCL?style=for-the-badge)

[English](../README.md) · **简体中文** · [繁體中文](README.zh-Hant.md)

## 简介

WTUMCL 是面向**武汉纺织大学 MC 社区（WTUMC）**的 SJMCL 社区版启动器。它保留上游启动器的核心工作流——多游戏目录与多实例、多账户登录、资源下载和扩展系统——并默认接入 WTUMC 自己的服务：社区新闻源、社区认证服务器与社区皮肤加载。

目标很直接：社区成员安装启动器后，用社区账户登录，即可进入社区服务器，并让皮肤正常显示，无需手动修改配置文件。

## 功能特性

- **WTUMC 社区集成**：默认使用社区新闻源 https://wtumc.top:64053/api/news，启动器的发现页无需手动配置即可显示社区公告与文章。
- **社区账户与皮肤**：内置社区认证服务器 https://wtumc.top:64053/api/yggdrasil；启动器会规范化认证服务器返回的皮肤域名（去除协议头、端口与路径），确保社区皮肤在游戏内正常加载。
- **跨平台支持**：兼容 Windows 10/11、macOS 10.15+ 与 Linux（webkit2gtk 4.1）。
- **高效的实例管理**：支持多个游戏目录与实例，集中管理所有实例资源（如存档、模组、资源包、光影包、截图等）与设置。
- **便捷的资源下载**：支持从 CurseForge 与 Modrinth 等源下载游戏客户端、模组加载器、各类游戏资源与整合包。
- **多账户系统支持**：内置 Microsoft 登录与第三方认证服务器支持，兼容 authlib-injector 认证服务器以及 Yggdrasil Connect 的 OAuth 登录流程规范提案。
- **外部服务协同**：通过 `wtumcl://` 深度链接与 MCP 服务，与外部网页、程序及 Agent 服务协同工作，提供一系列便捷功能与自动化能力。
- **开放扩展系统**：沿用 SJMCL 的扩展系统与扩展包格式，现有社区扩展仍可使用，也可按同样方式为 WTUMCL 开发新扩展。

> 注意：部分功能可能受地区、运行平台或程序分发类型限制。

### 技术栈

[![Tauri](https://img.shields.io/badge/Tauri-v2-FFC131?style=for-the-badge&logo=tauri&logoColor=white&labelColor=24C8DB)](https://tauri.app/)
[![Next JS](https://img.shields.io/badge/next.js-000000?style=for-the-badge&logo=nextdotjs&logoColor=white)](https://nextjs.org/)
[![Chakra UI](https://img.shields.io/badge/chakra_ui-v2-38B2AC?style=for-the-badge&logo=chakraui&logoColor=white&labelColor=319795)](https://v2.chakra-ui.com/)

## WTUMC 社区服务

| 服务           | 地址                                  | 用途                                                 |
| -------------- | ------------------------------------- | ---------------------------------------------------- |
| 社区站点       | https://wtumc.top:64053               | 社区主页、公告与用户入口                             |
| 社区新闻源     | https://wtumc.top:64053/api/news      | 启动器默认新闻数据源                                 |
| 社区认证服务器 | https://wtumc.top:64053/api/yggdrasil | 社区账户使用的 Yggdrasil / authlib-injector 认证端点 |

以上服务由 WTUMC 运营，是社区成员推荐使用的默认配置。其他兼容 authlib-injector 的认证服务器账户与皮肤仍可手动添加。

## 开始使用

请从 [GitHub Releases](https://github.com/SARICE233/WTUMCL/releases) 下载最新构建。构建产物由本仓库的 GitHub Actions 工作流生成。WTUMCL 不维护官方网站下载页，也不发布 Homebrew / Snap / AUR / Winget 软件包；如果您需要本分支，请不要安装上游 SJMCL 的发行包。

WTUMCL 目前支持以下平台：

| 平台    | 系统版本                          | 架构                        | 提供的分发类型                   |
| ------- | --------------------------------- | --------------------------- | -------------------------------- |
| Windows | 10 及以上                         | `aarch64`, `i686`, `x86_64` | 安装版 `.exe`，便携版 `.exe`     |
| macOS   | 10.15 及以上                      | `aarch64`, `x86_64`         | `.app`，`.app.tar.gz`            |
| Linux   | webkit2gtk 4.1（如 Ubuntu 22.04） | `aarch64`, `x86_64`         | `.deb`，`.rpm`，便携版二进制文件 |

## 从源码构建

自行构建 WTUMCL 前，请准备：

- Node.js 22 或更高版本，以及 pnpm 10 或更高版本；
- Rust 1.98.1 或更高版本；
- 对应操作系统的 [Tauri 系统依赖](https://tauri.app/start/prerequisites/)。

然后克隆仓库并构建：

```bash
git clone https://github.com/SARICE233/WTUMCL.git
cd WTUMCL
pnpm install
pnpm tauri dev
```

生成正式构建：

```bash
pnpm tauri build
```

## 开发与贡献

欢迎通过 [Issues](https://github.com/SARICE233/WTUMCL/issues) 与 [Pull Requests](https://github.com/SARICE233/WTUMCL/pulls) 参与本分支的开发，请遵循[本分支的贡献指南](CONTRIBUTING.zh-Hans.md)。WTUMCL 独立维护：改动直接落地本仓、不向上游提交，上游贡献流程不适用于本分支。

### 开发扩展

WTUMCL 沿用 SJMCL 的扩展系统与扩展包格式。现有扩展保持兼容，新扩展也可以按相同方式面向 WTUMCL 开发。扩展 API 与脚手架请参阅上游仓库。

### 上游贡献者

![贡献者](https://contrib.rocks/image?repo=UNIkeEN/SJMCL "贡献者")

## 版权与许可

版权所有 © 2024-2026 SJMCL 团队。

本分支自 2026-10-20 起对上游代码有所修改，修改 © 2026 武汉纺织大学 MC 社区（WTUMC）。

> 本软件并非官方 Minecraft 服务，未获得 Mojang 或 Microsoft 批准或关联。
> NOT AN OFFICIAL MINECRAFT SERVICE. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT.

本项目基于 [GNU 通用公共许可证 v3.0](../LICENSE) 发布。

发布产物对应的源码公开于 <https://github.com/SARICE233/WTUMCL/tree/v1.4.0>，GitHub 亦为每个 Release 附带源码归档。内部标识符（配置文件名、crate 名）沿用上游拼写，用于兼容既有用户数据。

依据 GPLv3 第 7 条款，当您分发本软件的修改版本时，除遵守 GPLv3 外，还须遵守以下[附加条款](../LICENSE.EXTRA)：

1. 必须更换软件名称，禁止使用 SJMCL 或 SJMC Launcher；
2. 在您的仓库 README、分发网站或相关文档、软件的关于页面中，须明确标注您的程序基于 SJMCL，并注明原仓库链接。
3. 当对本软件的修改仅限于**增加**（而非修改或删除）预置认证服务器（`src-tauri/src/account/helpers/authlib_injector/constants.rs`）时，前述第 1 条限制不适用。在该情形下，您可继续使用原始的软件名称进行编译与分发。

另根据我们网站的用户协议，当您分发本软件的修改版本时，请仅向我们的信息统计服务器（`src-tauri/src/utils/sys_info.rs`）发送带前缀（不少于两个字母，如 `XX-0.0.1`）的版本号，除非您的修改满足上述第 3 条限制。

## 联系我们

- WTUMC 社区站点：https://wtumc.top:64053
- 问题反馈：[GitHub Issues](https://github.com/SARICE233/WTUMCL/issues)
