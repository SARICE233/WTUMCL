# WTUMCL 用户协议

**最后更新：2026 年 10 月 20 日**
**生效日期：2026 年 10 月 20 日**

[English version](#english-version)

本协议是您与 WTUMC（武汉纺织大学 MC 社区，以下简称“我们”）之间就 WTUMCL（也称 WTUMC Launcher，以下简称“本软件”）达成的协议。下载、安装、启动或以其他方式使用本软件，即表示您已阅读、理解并同意本协议。如果您不同意本协议的任何内容，请停止使用并删除本软件。如果您未满 18 周岁，请务必在监护人陪同下阅读，并在取得监护人同意后使用。

## 1. 软件性质与开源许可

1.1 本软件由 WTUMC 免费提供，不收取许可费、订阅费或内购费用，并按“现状”提供。我们未就本软件作出任何付费功能、持续维护或特定可用性承诺。

1.2 本软件是基于 SJMCL 的衍生作品。上游项目 SJMCL（SJMC Launcher）的版权归 SJMCL Team 所有（© 2024-2026），上游仓库为 https://github.com/UNIkeEN/SJMCL 。本分支由 WTUMC 维护，主要修改包括品牌与界面、WTUMC 社区服务接入、认证服务器预设及部分功能调整。

1.3 本软件依据 GNU General Public License v3.0（GPLv3，全文见 [LICENSE](../LICENSE)）以及依 GPLv3 第 7 条作出的附加条款（[LICENSE.EXTRA](../LICENSE.EXTRA)）发布。GPLv3 与附加条款共同构成本软件的许可条件。

1.4 源码获取：本分支的源码仓库为 https://github.com/SARICE233/WTUMCL 。我们分发本软件二进制文件时，将按 GPLv3 的要求提供包含本分支修改的完整对应源码，具体获取地址以对应版本的发布页面为准；如通过发布页面无法获取，可按第 9 节的联系方式向我们索取。获取或使用源码时，请以与二进制版本相对应的版本为准。

1.5 再分发：在遵守 GPLv3 与 LICENSE.EXTRA 的前提下，您可以复制、修改和分发本软件，包括收取费用。再分发时至少应遵守以下要求：保留版权与许可声明；使用不同于 SJMCL 或 SJMC Launcher 的软件名称；在 README、分发页面、支持文档或软件“关于”页中显著说明您的程序基于 SJMCL 并给出上游仓库 URL；分发修改版时，按附加条款向上游统计服务器发送带不少于两个字母前缀的版本号（本项目使用 WTU- 前缀；仅新增预设认证服务器的情形按附加条款第 3 条处理）。其他分发者修改软件或收取费用的行为与我们无关。

1.6 本软件不是 Mojang Studios 或 Microsoft 的官方产品，未获得其背书、认可或授权。“Minecraft”及相关商标归其各自权利人所有。本软件不包含游戏本体，您应通过合法途径自行获取 Minecraft。

## 2. 账号与本地凭证

2.1 微软账号。您可以通过 Microsoft OAuth 设备代码流程登录微软账号。认证在 Microsoft 提供的页面完成，本软件不会要求您在应用内输入微软账号密码；本软件接收并保存 Microsoft 返回的访问令牌、刷新令牌及游戏档案信息，用于登录游戏与刷新会话。

2.2 第三方认证。您可以使用采用 Yggdrasil / authlib-injector 协议的第三方认证服务器，包括 WTUMC 社区认证服务器 https://wtumc.top:64053/api/yggdrasil 。登录时，您提供的用户名和密码会直接发送至您选择的认证服务器以完成验证；认证服务器随后返回访问令牌、角色信息与皮肤数据。

2.3 凭证仅保存在本地。本软件将账户信息（包括角色名、UUID、认证服务器地址、访问令牌、刷新令牌及其过期时间）写入本机应用数据目录中的 sjmcl.account.json。Windows 下该目录通常为 %APPDATA%\cn.wtu.wtumcl。便携模式仅将部分启动器配置文件保存到程序所在目录，账户文件仍位于应用数据目录。密码仅用于当次认证请求，不会写入上述文件，也不会被上传给我们。

2.4 第三方认证服务器由其运营方独立负责，包括账号注册、密码保管、角色与皮肤数据、封禁与申诉。我们不是该等服务器的运营方，不存储其用户密码，也无法代为重置密码、找回账号或解除封禁。您应阅读并遵守所选认证服务器的条款。

2.5 您应妥善保管设备与本地账户文件。在共享设备上使用后，请及时在账户管理中删除账户或清除应用数据。因设备被他人访问、恶意软件或您主动泄露导致的令牌风险，由您自行承担。

## 3. 数据与隐私

### 3.1 匿名统计上报

为统计不同版本与平台的使用情况，本软件在每次启动并完成初始化后，会异步向 SJMCL 上游统计服务发送一次 HTTP POST 请求：

- 接收地址：https://mc.sjtu.cn/api-sjmcl/statistics
- 请求体（JSON）仅包含以下三个字段：

| 字段 | 实际内容 | 说明 |
| --- | --- | --- |
| version | `WTU-` 加上启动器版本号，例如 `WTU-0.1.0` | `WTU-` 前缀用于标识 WTUMC 分发版，同时满足 LICENSE.EXTRA 的统计要求 |
| os | 操作系统平台标识，取自 `tauri_plugin_os::platform()`，常见值为 `windows`、`macos`、`linux` | 仅表示平台类别，不包含系统版本号、主机名或硬件信息 |
| exe_sha256 | 本软件可执行文件的 SHA-256 摘要（十六进制字符串；计算失败时为空字符串） | 用于区分不同的构建产物 |

3.2 上报时机与频率：每次启动最多一次，在应用初始化阶段异步发送；发送失败会被静默忽略，不重试，也不影响启动或功能使用。除上述三个字段外，代码不会收集或发送账号信息、角色 UUID、游戏 ID、服务器列表、存档、文件内容、硬件序列号、MAC 地址等数据；请求体中也没有单独的 IP 字段。与任何 HTTP 请求一样，接收方在网络层可以看到来源 IP，但代码不会将 IP 写入统计请求体。

3.3 用途与接收方：上述数据用于统计版本与平台的分布、识别构建产物，并满足上游附加条款的统计要求。接收方为 SJMCL 上游统计服务（mc.sjtu.cn），其服务端的数据保存与处理受该服务运营方的政策约束，我们无法控制其保存期限或删除流程。

3.4 关闭方式（含设置项现状）：截至本协议更新日，本软件的应用内设置中尚未提供统计上报开关（“设置 → 通用”页面中没有统计或隐私开关），代码中也不存在对应的配置项，因此无法通过图形界面关闭。您可以选择：

1. 在操作系统防火墙中为本软件添加出站规则，阻止其访问 mc.sjtu.cn；
2. 在本机 hosts 文件中将 mc.sjtu.cn 指向 0.0.0.0（Windows 通常为 C:\Windows\System32\drivers\etc\hosts）。该做法可能同时影响依赖该域名的其他功能；
3. 依 GPLv3 获取源码，移除或停用 send_statistics 调用后自行构建。

如果后续版本在设置中增加开关，以应用内实际提供的界面为准，我们会同步更新本协议。

3.5 其他联网功能：登录、新闻、资源搜索与下载、皮肤、更新检查等功能会向相应第三方服务发起请求。这些请求会使接收方获得网络层常规信息（例如来源 IP），并适用第 4 节所述第三方的隐私政策。我们在本地保存的配置、实例、日志与缓存仅用于本机运行；除实现您主动使用的联网功能所必需外，不会主动上传。

3.6 数据删除：卸载本软件不会自动删除应用数据目录中的配置与账户文件；您可手动删除该目录。删除后无法恢复。

## 4. 第三方服务

4.1 本软件会连接以下第三方服务，以实现相应功能：

| 服务 | 用途 | 条款或主页 |
| --- | --- | --- |
| Microsoft 账号与 Xbox / Minecraft 服务 | 微软账号登录、档案与游戏所有权校验 | https://www.microsoft.com/servicesagreement |
| CurseForge | 整合包与模组等资源的搜索、下载 | https://www.curseforge.com |
| Modrinth | 模组等资源的搜索、下载 | https://modrinth.com |
| WTUMC 社区站点与新闻源 | 社区公告、新闻、帮助入口 | https://wtumc.top:64053 |
| WTUMC 社区认证服务器 | 社区账号的 Yggdrasil / authlib-injector 认证与皮肤 | https://wtumc.top:64053/api/yggdrasil |
| SJMCL 上游统计服务 | 第 3.1 节所述统计上报 | https://mc.sjtu.cn |
| GitHub | 源码、发行与更新 | https://github.com/SARICE233/WTUMCL |

4.2 上述服务由各自运营方独立提供，并适用其自身的服务条款与隐私政策。我们不是其运营方，不控制其内容、可用性、资费或数据处理行为，也不对其中断、变更、封禁、内容合法性或数据丢失承担责任。

4.3 第三方服务可能随时调整接口或停止服务，由此导致的部分功能不可用，不视为本软件违约。我们会尽合理努力进行适配，但不作保证。

## 5. 扩展与第三方内容

5.1 本软件提供扩展系统。扩展由第三方作者开发，可能读取本地数据、发起网络请求、修改界面或执行其他操作。扩展的权限、行为与数据处理方式由扩展作者控制。

5.2 您应仅从可信来源安装扩展，并在安装前阅读其说明与源码。扩展造成的数据泄露、账号风险、游戏异常、设备损害或法律纠纷，由扩展作者与您按各自责任承担；我们不认可、不担保任何第三方扩展，也不对其行为负责。

5.3 通过本软件下载的模组、整合包、资源包、光影、地图等第三方内容，其版权与使用条件由作者或发布平台规定。您应自行确认授权范围并遵守相应的最终用户许可协议。我们不托管这些内容，也不保证其合法性、安全性与兼容性。

5.4 我们可基于安全、兼容或法律原因，在不事先通知的情况下停用、屏蔽或移除特定扩展或内容来源。

## 6. 免责声明与责任限制

6.1 本软件按“现状”和“现有”提供，不附带任何明示或默示的保证，包括但不限于适销性、特定用途适用性、不侵权、无中断、无错误、无病毒或安全性保证。

6.2 我们不保证本软件能启动任何特定游戏版本，不保证模组、整合包、皮肤或第三方服务持续可用，不保证您的账号不被封禁、存档不丢失、下载内容合法或安全。您应自行备份存档与配置。

6.3 在适用法律允许的最大范围内，对于因使用或无法使用本软件而产生的任何间接、附带、特殊、惩罚性或后果性损害（包括数据丢失、设备损坏、账号封禁、利润损失、第三方索赔），我们不承担责任。

6.4 本软件免费提供。在适用法律允许的最大范围内，我们就本协议承担的累计责任总额不超过您为使用本软件实际支付的费用（当前为 0 元）。

6.5 本条不排除或限制依法不能排除或限制的责任。若本条任何内容被认定无效，其余条款仍继续有效。

## 7. 协议变更与生效

7.1 本协议自文首载明的生效日期起生效，并通过本软件内的协议入口及本仓库的 docs/TOS.md 公布。

7.2 我们可能因功能、法律或运营变化更新本协议。更新后的协议将在上述位置发布，并更新“最后更新”日期；涉及您重要权利义务的变更，我们会通过社区站点或仓库公告等方式提示。

7.3 变更生效后，您继续使用本软件即视为接受更新后的协议。如不同意，请停止使用并删除本软件。

## 8. 未成年人提示

8.1 若您未满 18 周岁，或未达到您所在地区的法定成年年龄，请在监护人知情、同意和指导下阅读并使用本软件。监护人应对未成年人的使用行为及由此产生的后果负责。

8.2 微软账号、Minecraft 及第三方认证服务器可能设有年龄限制或监护人同意要求，请遵守相应规定。

8.3 若监护人不同意本协议，未成年人应停止使用本软件。

## 9. 联系方式

- WTUMC 社区站点：https://wtumc.top:64053
- 本分支源码与问题反馈：https://github.com/SARICE233/WTUMCL

与本协议、隐私或数据统计有关的问题，可通过上述渠道联系。我们会在合理时间内处理。

## 10. 其他

10.1 本协议构成您与我们之间关于本软件的完整协议，取代此前就同一事项作出的口头或书面说明。

10.2 我们未行使或延迟行使本协议项下任何权利，不构成对该权利的放弃。

10.3 若本协议任何条款被认定无效或不可执行，该条款将在最小范围内调整或删除，其余条款继续有效。

10.4 本协议以简体中文书写，并附英文译文供参考；如中英文版本存在冲突，以简体中文版本为准。

10.5 本协议的订立、效力、解释与争议解决适用中华人民共和国法律（不含冲突法规则）。因本协议产生的争议，双方应先友好协商；协商不成的，提交被告住所地有管辖权的人民法院。

---

# English Version

**Last updated: October 20, 2026**
**Effective date: October 20, 2026**

This User Agreement (the "Agreement") is entered into between you and WTUMC (the Minecraft community of Wuhan Textile University, "we", "us", or "our") regarding WTUMCL (also referred to as WTUMC Launcher, the "Software"). By downloading, installing, launching, or otherwise using the Software, you acknowledge that you have read, understood, and agree to this Agreement. If you do not agree with any part of this Agreement, please stop using the Software and delete it. If you are under 18 years of age, please read this Agreement with your legal guardian and use the Software only with your guardian's consent.

## 1. Nature of the Software and Open-Source License

1.1 The Software is provided by WTUMC free of charge, without license fees, subscription fees, or in-app purchases, and is provided "as is". We make no commitment to paid features, continued maintenance, or any particular level of availability.

1.2 The Software is a derivative work based on SJMCL. The upstream project SJMCL (SJMC Launcher) is Copyright © 2024-2026 SJMCL Team, and its repository is https://github.com/UNIkeEN/SJMCL . This branch is maintained by WTUMC and includes changes to branding and user interface, WTUMC community service integration, preset authentication servers, and other functional adjustments.

1.3 The Software is released under the GNU General Public License v3.0 (GPLv3, see [LICENSE](../LICENSE)) together with additional terms made under Article 7 of GPLv3 (see [LICENSE.EXTRA](../LICENSE.EXTRA)). GPLv3 and the additional terms together constitute the license conditions for the Software.

1.4 Source code: the source repository for this branch is https://github.com/SARICE233/WTUMCL . When we distribute binaries of the Software, we will provide the complete corresponding source code, including modifications made in this branch, as required by GPLv3. The exact location will be stated on the release page for the corresponding version. If it is unavailable there, you may request it from us using the contact details in Section 9. Please use the source code that corresponds to the binary version you received.

1.5 Redistribution: subject to GPLv3 and LICENSE.EXTRA, you may copy, modify, and distribute the Software, including for a fee. At a minimum, you must retain copyright and license notices; use a software name different from SJMCL or SJMC Launcher; clearly state in your README, distribution page, support documentation, or the Software's "About" page that your program is based on SJMCL and give the upstream repository URL; and, when distributing a modified version, send version numbers with a prefix of at least two letters to the upstream statistics server as required by the additional terms (this project uses the WTU- prefix; modifications that only add preset authentication servers are governed by Clause 3 of the additional terms). We are not responsible for other distributors' modifications or fees.

1.6 The Software is not an official product of Mojang Studios or Microsoft and is not endorsed, approved, or authorized by them. "Minecraft" and related trademarks belong to their respective owners. The Software does not include the game itself; you must obtain Minecraft through lawful means.

## 2. Accounts and Local Credentials

2.1 Microsoft accounts. You may sign in with a Microsoft account through the Microsoft OAuth device code flow. Authentication is completed on a Microsoft-hosted page; the Software does not ask you to enter your Microsoft password inside the application. The Software receives and stores the access token, refresh token, and game profile information returned by Microsoft in order to sign in to the game and refresh your session.

2.2 Third-party authentication. You may use third-party authentication servers implementing the Yggdrasil / authlib-injector protocol, including the WTUMC community authentication server at https://wtumc.top:64053/api/yggdrasil . During sign-in, the username and password you provide are sent directly to the authentication server you selected for verification. The authentication server then returns an access token, profile information, and skin data.

2.3 Credentials are stored only locally. The Software writes account information (including profile name, UUID, authentication server URL, access token, refresh token, and their expiration times) to sjmcl.account.json in the local application data directory. On Windows, this directory is usually %APPDATA%\cn.wtu.wtumcl. Portable mode stores only some launcher configuration files in the program directory; the account file remains in the application data directory. The password is used only for the authentication request at that time; it is not written to that file and is not uploaded to us.

2.4 Third-party authentication servers are independently operated by their respective providers, including account registration, password storage, profile and skin data, bans, and appeals. We do not operate those servers, do not store their users' passwords, and cannot reset passwords, recover accounts, or lift bans on their behalf. You should read and comply with the terms of the authentication server you choose.

2.5 You are responsible for safeguarding your device and local account files. After using the Software on a shared device, remove your account or clear the application data promptly. You bear the risks arising from others accessing your device, malware, or your own disclosure of credentials.

## 3. Data and Privacy

### 3.1 Anonymous Statistics

To measure usage across versions and platforms, the Software sends one asynchronous HTTP POST request to the upstream SJMCL statistics service after each startup and initialization:

- Endpoint: https://mc.sjtu.cn/api-sjmcl/statistics
- The JSON request body contains only the following three fields:

| Field | Actual value | Purpose |
| --- | --- | --- |
| version | `WTU-` followed by the launcher version, for example `WTU-0.1.0` | The `WTU-` prefix identifies the WTUMC distribution and satisfies the statistics requirement in LICENSE.EXTRA |
| os | Operating-system platform identifier returned by `tauri_plugin_os::platform()`, commonly `windows`, `macos`, or `linux` | Indicates only the platform category; it does not include the OS version, hostname, or hardware information |
| exe_sha256 | SHA-256 digest of the Software's executable file (hexadecimal string; an empty string if calculation fails) | Distinguishes different build artifacts |

3.2 Timing and frequency: at most once per startup, sent asynchronously during application initialization. Failures are silently ignored, are not retried, and do not affect startup or functionality. Apart from the three fields above, the code does not collect or send account information, profile UUIDs, game IDs, server lists, saves, file contents, hardware serial numbers, MAC addresses, or similar data; the request body also has no separate IP field. As with any HTTP request, the recipient can see the source IP at the network layer, but the code does not write the IP into the statistics request body.

3.3 Purpose and recipient: the data is used to measure the distribution of versions and platforms, identify build artifacts, and satisfy the statistics requirement in the upstream additional terms. The recipient is the upstream SJMCL statistics service (mc.sjtu.cn). Data retention and processing on its server are governed by that service operator's policies; we cannot control its retention period or deletion process.

3.4 How to opt out (including the current state of settings): as of the date of this Agreement, the Software's in-app Settings do not provide a statistics toggle (there is no statistics or privacy switch on the Settings → General page), and no corresponding configuration key exists in the code, so statistics cannot be disabled through the graphical interface. You may:

1. Add an outbound rule in your operating system's firewall to block the Software from accessing mc.sjtu.cn;
2. Map mc.sjtu.cn to 0.0.0.0 in your hosts file (on Windows, usually C:\Windows\System32\drivers\etc\hosts). This may also affect other features that rely on that domain;
3. Obtain the source code under GPLv3, remove or disable the send_statistics call, and build the Software yourself.

If a future version adds a toggle in Settings, the interface actually provided in the application will govern, and we will update this Agreement accordingly.

3.5 Other network features: sign-in, news, resource search and downloads, skins, and update checks send requests to the relevant third-party services. Those requests may expose ordinary network-layer information (such as the source IP) to the recipient and are subject to the privacy policies of the third parties described in Section 4. Configuration, instances, logs, and caches stored locally are used only for local operation and are not actively uploaded except as necessary to provide a network feature you choose to use.

3.6 Data deletion: uninstalling the Software does not automatically delete configuration and account files in the application data directory; you may delete that directory manually. Deleted data cannot be recovered.

## 4. Third-Party Services

4.1 The Software connects to the following third-party services to provide corresponding features:

| Service | Purpose | Terms or homepage |
| --- | --- | --- |
| Microsoft account and Xbox / Minecraft services | Microsoft sign-in, profile and game ownership checks | https://www.microsoft.com/servicesagreement |
| CurseForge | Search and download of modpacks and mods | https://www.curseforge.com |
| Modrinth | Search and download of mods and other resources | https://modrinth.com |
| WTUMC community site and news feed | Community announcements, news, and help links | https://wtumc.top:64053 |
| WTUMC community authentication server | Yggdrasil / authlib-injector authentication and skins for community accounts | https://wtumc.top:64053/api/yggdrasil |
| SJMCL upstream statistics service | Statistics described in Section 3.1 | https://mc.sjtu.cn |
| GitHub | Source code, releases, and updates | https://github.com/SARICE233/WTUMCL |

4.2 These services are independently provided by their respective operators and are governed by their own terms of service and privacy policies. We do not operate them, do not control their content, availability, fees, or data handling, and are not responsible for their interruptions, changes, bans, content legality, or data loss.

4.3 Third-party services may change their APIs or discontinue service at any time. Partial unavailability of features caused by such changes is not a breach by us. We will make reasonable efforts to adapt but provide no guarantee.

## 5. Extensions and Third-Party Content

5.1 The Software provides an extension system. Extensions are developed by third-party authors and may read local data, make network requests, modify the interface, or perform other operations. Extension permissions, behavior, and data handling are controlled by the extension author.

5.2 Install extensions only from trusted sources and review their documentation and source code before installation. Data leaks, account risks, game errors, device damage, or legal disputes caused by extensions are the responsibility of the extension author and you, according to each party's responsibility. We do not endorse or warrant any third-party extension and are not responsible for its behavior.

5.3 Mods, modpacks, resource packs, shaders, maps, and other third-party content downloaded through the Software are subject to the copyright and usage terms set by their authors or distribution platforms. You must verify the scope of authorization and comply with the applicable end-user license agreements. We do not host such content and do not guarantee its legality, security, or compatibility.

5.4 We may disable, block, or remove specific extensions or content sources without prior notice for security, compatibility, or legal reasons.

## 6. Disclaimer and Limitation of Liability

6.1 The Software is provided "as is" and "as available", without any express or implied warranties, including but not limited to merchantability, fitness for a particular purpose, non-infringement, uninterrupted operation, freedom from errors or viruses, or security.

6.2 We do not guarantee that the Software can launch any particular game version, that mods, modpacks, skins, or third-party services will remain available, that your account will not be banned, that your saves will not be lost, or that downloaded content is lawful or safe. You should back up your saves and configuration.

6.3 To the maximum extent permitted by applicable law, we are not liable for any indirect, incidental, special, punitive, or consequential damages arising from the use of or inability to use the Software, including data loss, device damage, account bans, lost profits, or third-party claims.

6.4 The Software is provided free of charge. To the maximum extent permitted by applicable law, our aggregate liability under this Agreement shall not exceed the amount you actually paid to use the Software (currently 0).

6.5 This section does not exclude or limit liability that cannot be excluded or limited by law. If any part of this section is held invalid, the remaining provisions remain in effect.

## 7. Changes and Effectiveness

7.1 This Agreement takes effect on the effective date stated at the top and is published through the in-app agreement link and docs/TOS.md in this repository.

7.2 We may update this Agreement due to functional, legal, or operational changes. Updated versions will be published at those locations with a revised "Last updated" date. For changes that materially affect your rights or obligations, we will provide notice through the community site or repository announcements.

7.3 After a change takes effect, continued use of the Software constitutes acceptance of the updated Agreement. If you do not agree, please stop using the Software and delete it.

## 8. Minors

8.1 If you are under 18 years of age, or below the age of majority in your jurisdiction, please read and use the Software only with your guardian's knowledge, consent, and guidance. Your guardian is responsible for your use and its consequences.

8.2 Microsoft accounts, Minecraft, and third-party authentication servers may impose age limits or guardian-consent requirements. Please comply with them.

8.3 If your guardian does not agree to this Agreement, you should stop using the Software.

## 9. Contact

- WTUMC community site: https://wtumc.top:64053
- Source code and issue tracker for this branch: https://github.com/SARICE233/WTUMCL

Questions about this Agreement, privacy, or statistics may be raised through these channels. We will handle them within a reasonable period.

## 10. Miscellaneous

10.1 This Agreement constitutes the entire agreement between you and us regarding the Software and supersedes prior oral or written statements on the same subject.

10.2 Our failure or delay in exercising any right under this Agreement does not constitute a waiver of that right.

10.3 If any provision of this Agreement is held invalid or unenforceable, that provision will be modified or removed to the minimum extent necessary, and the remaining provisions will continue in effect.

10.4 This Agreement is written in Simplified Chinese and accompanied by an English translation for reference. In case of conflict, the Simplified Chinese version prevails.

10.5 The formation, validity, interpretation, and dispute resolution of this Agreement are governed by the laws of the People's Republic of China (excluding conflict-of-law rules). Disputes arising from this Agreement should first be resolved through friendly negotiation; if negotiation fails, they shall be submitted to the competent people's court at the defendant's domicile.
