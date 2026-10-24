> **WTUMCL** is a fork of [SJMCL](https://github.com/UNIkeEN/SJMCL), maintained by **WTUMC (Wuhan Textile University Minecraft Community)**.
> The original project is Copyright © 2024-2026 SJMCL Team and is licensed under GPLv3 with additional terms (see [LICENSE.EXTRA](LICENSE.EXTRA)).
> Upstream repository: https://github.com/UNIkeEN/SJMCL · This fork: https://github.com/SARICE233/WTUMCL

<img src="docs/figs/banner.png" alt="WTUMCL" width="640" />

[![Test Build](https://img.shields.io/github/actions/workflow/status/SARICE233/WTUMCL/test.yml?label=test%20build&logo=github&style=for-the-badge)](https://github.com/SARICE233/WTUMCL/blob/main/.github/workflows/test.yml)
![Downloads](https://img.shields.io/github/downloads/SARICE233/WTUMCL/total?logo=github&style=for-the-badge)
![Stars](https://img.shields.io/github/stars/SARICE233/WTUMCL?style=for-the-badge)

**English** · [简体中文](docs/README.zh-Hans.md) · [繁體中文](docs/README.zh-Hant.md)

## Introduction

WTUMCL is the community edition of the SJMCL launcher, maintained for the **Wuhan Textile University Minecraft Community (WTUMC)**. It keeps the upstream launcher's core workflow — multiple game directories and instances, multi-account sign-in, resource downloads and an extension system — and connects it to WTUMC's own services by default: the community news feed, the community authentication server and community skin loading.

The goal is simple: WTUMC members install the launcher, sign in with a community account, and join the community server with their skins working, without editing configuration files.

## Features

- **WTUMC Community Integration**: The community news feed at https://wtumc.top:64053/api/news is the default source, so community announcements and articles appear in the launcher's discovery page without manual setup.
- **Community Accounts and Skins**: The community authentication server https://wtumc.top:64053/api/yggdrasil is built in. The launcher normalizes skin domains returned by the server (stripping URL schemes, ports and paths) so that community skins load correctly in game.
- **Cross Platform**: Supports Windows 10/11, macOS 10.15+ and Linux (webkit2gtk 4.1).
- **Efficient Instance Management**: Supports multiple game directories and instances, allowing the management of all instance resources (such as saves, mods, resource packs, shaders, screenshots, etc.) and settings in one place.
- **Convenient Resource Download**: Supports downloading game clients, mod loaders, various game resources and modpacks from CurseForge and Modrinth.
- **Multi-Account System Support**: Built-in Microsoft login and third-party authentication server support, including authlib-injector compatible servers and the OAuth login flow proposed by the Yggdrasil Connect proposal.
- **External Service Collaboration**: Works with external webpages, applications and Agent services through `wtumcl://` deeplinks and MCP services, delivering convenient features and automation capabilities.
- **Open Extension System**: Keeps the SJMCL extension system and package format, so existing community extensions remain usable while new ones can be developed for WTUMCL.

> Note: some features may be limited by region, platform, or bundle type.

### Built with

[![Tauri](https://img.shields.io/badge/Tauri-v2-FFC131?style=for-the-badge&logo=tauri&logoColor=white&labelColor=24C8DB)](https://tauri.app/)
[![Next JS](https://img.shields.io/badge/next.js-000000?style=for-the-badge&logo=nextdotjs&logoColor=white)](https://nextjs.org/)
[![Chakra UI](https://img.shields.io/badge/chakra_ui-v2-38B2AC?style=for-the-badge&logo=chakraui&logoColor=white&labelColor=319795)](https://v2.chakra-ui.com/)

## WTUMC Community Services

| Service                  | Address                               | Purpose                                                      |
| ------------------------ | ------------------------------------- | ------------------------------------------------------------ |
| Community site           | https://wtumc.top:64053               | Community home page, announcements and user entry            |
| Community news feed      | https://wtumc.top:64053/api/news      | Default news source in the launcher                          |
| Community authentication | https://wtumc.top:64053/api/yggdrasil | Yggdrasil / authlib-injector endpoint for community accounts |

These services are operated by WTUMC and are the recommended defaults for community members. Accounts and skins from other authlib-injector compatible servers can still be added manually.

## Getting Started

Download the latest build from [GitHub Releases](https://github.com/SARICE233/WTUMCL/releases). Builds are produced by this repository's GitHub Actions workflows. WTUMCL does not maintain an official website download page or Homebrew / Snap / AUR / Winget packages, so please do not install an upstream SJMCL package if you want this fork.

WTUMCL currently supports the following platforms:

| Platform | Versions                            | Architectures               | Provided Bundles                  |
| -------- | ----------------------------------- | --------------------------- | --------------------------------- |
| Windows  | 10 and above                        | `aarch64`, `i686`, `x86_64` | installer `.exe`, portable `.exe` |
| macOS    | 10.15 and above                     | `aarch64`, `x86_64`         | `.app`, `.dmg`                    |
| Linux    | webkit2gtk 4.1 (e.g., Ubuntu 22.04) | `aarch64`, `x86_64`         | `.deb`, `.rpm`, portable binary   |

## Build from Source

To build WTUMCL yourself, install the following prerequisites:

- Node.js 22 or newer and pnpm 10 or newer;
- Rust 1.98.1 or newer;
- the [Tauri system dependencies](https://tauri.app/start/prerequisites/) for your operating system.

Then clone the repository and build:

```bash
git clone https://github.com/SARICE233/WTUMCL.git
cd WTUMCL
pnpm install
pnpm tauri dev
```

Create a release build with:

```bash
pnpm tauri build
```

## Development and Contributing

Contributions to this fork are welcome through [Issues](https://github.com/SARICE233/WTUMCL/issues) and [Pull Requests](https://github.com/SARICE233/WTUMCL/pulls) against this repository. Please follow [this fork's Contributing Guide](CONTRIBUTING.md). WTUMCL is maintained independently: changes land here and are not submitted upstream, so the upstream contributing guide does not apply to this fork.

### Developing Extensions

WTUMCL inherits the SJMCL extension system and package format. Existing extensions remain compatible, and new extensions can target WTUMCL in the same way. See the upstream repository for the extension API and scaffold.

### Upstream Contributors

![Contributors](https://contrib.rocks/image?repo=UNIkeEN/SJMCL "Contributors")

## Copyright

Copyright © 2024-2026 SJMCL Team.

Modifications since 2026-10-20 © 2026 WTUMC (Wuhan Textile University Minecraft Community).

WTUMCL is based on [SJMCL](https://github.com/UNIkeEN/SJMCL) and changes the launcher's branding, launch page, download pipeline and community-service integration. Internal identifiers (config file names, crate names) intentionally keep their upstream spelling for compatibility.

> NOT AN OFFICIAL MINECRAFT SERVICE. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT.

## License

WTUMCL is distributed under [GNU General Public License v3.0](LICENSE).

The Corresponding Source of the release binaries is published at <https://github.com/SARICE233/WTUMCL/tree/v1.4.0>; GitHub also attaches a source archive to every release.

By GPLv3 License term 7, we require that when you distribute a modified version of the software, you must obey GPLv3 License as well as the following [additional terms](LICENSE.EXTRA):

1. Use a different software name than SJMCL or SJMC Launcher;
2. Mark clearly in your repository README file, your distribution website or thread, Support documents, About Page in the software that your program is based on SJMCL and give out the url of the origin repository.
3. When your modifications to this software are limited solely to **adding** (without modifying or deleting) preset authentication servers (`src-tauri/src/account/helpers/authlib_injector/constants.rs`), the restrictions set forth in Clauses 1 above shall not apply. In this case, you may continue to compile and distribute the software under its original name.

Besides, per term of use of our website, when distributing a modified version of the software, please send version numbers with prefix (more than two letters, e.g. `XX-0.0.1`) to our statistics server (`src-tauri/src/utils/sys_info.rs`) unless your modifications meets Clauses 3 above.

## Contact

- WTUMC community site: https://wtumc.top:64053
- Issue tracker: https://github.com/SARICE233/WTUMCL/issues
