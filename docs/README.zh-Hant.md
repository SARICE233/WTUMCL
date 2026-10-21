> **WTUMCL** 是基於 [SJMCL](https://github.com/UNIkeEN/SJMCL) 的二次開發分支，由 **武漢紡織大學 MC 社群（WTUMC）** 維護。
> 原專案版權歸 SJMCL 團隊（© 2024-2026）所有，以 GPLv3 及附加條款（見 [LICENSE.EXTRA](../LICENSE.EXTRA)）發佈。
> 源倉庫：https://github.com/UNIkeEN/SJMCL

<img src="figs/banner.png" alt="WTUMCL" width="640" />

[![Test Build](https://img.shields.io/github/actions/workflow/status/SARICE233/WTUMCL/test.yml?label=test%20build&logo=github&style=for-the-badge)](https://github.com/SARICE233/WTUMCL/blob/main/.github/workflows/test.yml)
![Downloads](https://img.shields.io/github/downloads/SARICE233/WTUMCL/total?logo=github&style=for-the-badge)
![Stars](https://img.shields.io/github/stars/SARICE233/WTUMCL?style=for-the-badge)

[English](../README.md) · [简体中文](README.zh-Hans.md) · **繁體中文**

## 簡介

WTUMCL 是面向**武漢紡織大學 MC 社群（WTUMC）**的 SJMCL 社群版啟動器。它保留上游啟動器的核心工作流程——多遊戲目錄與多實例、多帳戶登入、資源下載和擴充系統——並預設接入 WTUMC 自己的服務：社群新聞源、社群認證伺服器與社群外觀載入。

目標很直接：社群成員安裝啟動器後，用社群帳戶登入，即可進入社群伺服器，並讓外觀正常顯示，無需手動修改設定檔。

## 功能特性

* **WTUMC 社群整合**：預設使用社群新聞源 https://wtumc.top:64053/api/news，啟動器的探索頁無需手動設定即可顯示社群公告與文章。
* **社群帳戶與外觀**：內建社群認證伺服器 https://wtumc.top:64053/api/yggdrasil；啟動器會正規化認證伺服器回傳的外觀網域（移除協定標頭、連接埠與路徑），確保社群外觀在遊戲內正常載入。
* **跨平臺支援**：相容 Windows 10/11、macOS 10.15+ 與 Linux（webkit2gtk 4.1）。
* **高效的實例管理**：支援多個遊戲目錄與實例，集中管理所有實例資源（如存檔、模組、資源包、光影包、截圖等）與設定。
* **便捷的資源下載**：支援從 CurseForge 與 Modrinth 等源下載遊戲用戶端、模組載入器、各類遊戲資源與模組包。
* **多帳戶系統支援**：內建 Microsoft 登入與第三方認證伺服器支援，相容 authlib-injector 認證伺服器以及 Yggdrasil Connect 的 OAuth 登入流程規範提案。
* **外部服務協同**：透過 `wtumcl://` 深度連結與 MCP 服務，與外部網頁、程式及 Agent 服務協同運作，提供一系列便捷功能與自動化能力。
* **開放擴充系統**：沿用 SJMCL 的擴充系統與擴充包格式，現有社群擴充仍可使用，也可按同樣方式為 WTUMCL 開發新擴充。

> 注意：部分功能可能受地區、執行平臺或程式發行類型限制。

### 技術堆疊

[![Tauri](https://img.shields.io/badge/Tauri-v2-FFC131?style=for-the-badge&logo=tauri&logoColor=white&labelColor=24C8DB)](https://tauri.app/)
[![Next JS](https://img.shields.io/badge/next.js-000000?style=for-the-badge&logo=nextdotjs&logoColor=white)](https://nextjs.org/)
[![Chakra UI](https://img.shields.io/badge/chakra_ui-v2-38B2AC?style=for-the-badge&logo=chakraui&logoColor=white&labelColor=319795)](https://v2.chakra-ui.com/)

## WTUMC 社群服務

| 服務 | 位址 | 用途 |
|------|------|------|
| 社群網站 | https://wtumc.top:64053 | 社群首頁、公告與使用者入口 |
| 社群新聞源 | https://wtumc.top:64053/api/news | 啟動器預設新聞資料來源 |
| 社群認證伺服器 | https://wtumc.top:64053/api/yggdrasil | 社群帳戶使用的 Yggdrasil / authlib-injector 認證端點 |

以上服務由 WTUMC 營運，是社群成員推薦使用的預設設定。其他相容 authlib-injector 的認證伺服器帳戶與外觀仍可手動加入。

## 開始使用

請從 [GitHub Releases](https://github.com/SARICE233/WTUMCL/releases) 下載最新建置。建置產物由本倉庫的 GitHub Actions 工作流程產生。WTUMCL 不維護官方網站下載頁，也不發佈 Homebrew / Snap / AUR / Winget 套件；如果您需要本分支，請不要安裝上游 SJMCL 的發行套件。

WTUMCL 目前支援以下平臺：

| 平臺    | 系統版本            | 架構               | 提供的發行類型                              |
|---------|---------------------|--------------------|--------------------------------------------|
| Windows | 10 及以上           | `aarch64`, `i686`, `x86_64` | 安裝版 `.exe`，便攜版 `.exe` |
| macOS   | 10.15 及以上        | `aarch64`, `x86_64` | `.app`，`.dmg`                   |
| Linux   | webkit2gtk 4.1（如 Ubuntu 22.04） | `aarch64`, `x86_64` | `.deb`，`.rpm`，便攜版二進位檔 |

## 從原始碼建置

自行建置 WTUMCL 前，請準備：

* Node.js 22 或更新版本，以及 pnpm 10 或更新版本；
* Rust 1.98.1 或更新版本；
* 對應作業系統的 [Tauri 系統相依套件](https://tauri.app/start/prerequisites/)。

然後複製倉庫並建置：

```bash
git clone https://github.com/SARICE233/WTUMCL.git
cd WTUMCL
pnpm install
pnpm tauri dev
```

產生正式建置：

```bash
pnpm tauri build
```

## 開發與貢獻

歡迎透過 [Issues](https://github.com/SARICE233/WTUMCL/issues) 與 [Pull Requests](https://github.com/SARICE233/WTUMCL/pulls) 參與本分支的開發。若變更同時涉及上游專案，請遵循上游的[貢獻指南](https://github.com/UNIkeEN/SJMCL/blob/main/CONTRIBUTING.md)。

### 開發擴充

WTUMCL 沿用 SJMCL 的擴充系統與擴充包格式。現有擴充保持相容，新擴充也可以按相同方式面向 WTUMCL 開發。擴充 API 與腳手架請參閱上游倉庫。

### 上游貢獻者

![貢獻者](https://contrib.rocks/image?repo=UNIkeEN/SJMCL "貢獻者")

## 版權與授權

版權所有 © 2024-2026 SJMCL 團隊。

本分支修改 © 2026 武漢紡織大學 MC 社群（WTUMC）。

> 本軟體並非官方 Minecraft 服務，未獲得 Mojang 或 Microsoft 批准或關聯。
> NOT AN OFFICIAL MINECRAFT SERVICE. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT.

本專案基於 [GNU 通用公眾授權條款 v3.0](../LICENSE) 發佈。

依據 GPLv3 第 7 條款，當您分發本軟體的修改版本時，除遵守 GPLv3 外，還須遵守以下[附加條款](../LICENSE.EXTRA)：

1. 必須更換軟體名稱，禁止使用 SJMCL 或 SJMC Launcher；
2. 在您的專案 README、分發網站或相關文件、軟體的關於頁面中，須明確標註您的程式基於 SJMCL，並註明原專案連結。
3. 當對本軟體的修改僅限於**增加**（而非修改或刪除）預置認證伺服器（`src-tauri/src/account/helpers/authlib_injector/constants.rs`）時，前述第 1 條之限制不適用。在此情形下，您得繼續使用原始軟體名稱進行編譯與分發。

另根據我們網站的使用者協議，當您分發本軟體的修改版本時，請僅向我們的資訊統計伺服器（`src-tauri/src/utils/sys_info.rs`）傳送帶字首（不少於兩個字母，如 `XX-0.0.1`）的版本號，除非您的修改滿足上述第 3 條限制。

## 聯絡我們

* WTUMC 社群網站：https://wtumc.top:64053
* 問題回報：[GitHub Issues](https://github.com/SARICE233/WTUMCL/issues)
