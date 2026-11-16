# 发布者与代码签名

文件属性的 Company / Authors 使用 TamikiP；这不是经过认证的 Windows 发布者身份。当前没有可用证书，构建仍为未签名状态。

`ProductInfo.props` 保存默认产品版本。`package.py --display-version` 在本次构建中通过 `TOWER_PRODUCT_VERSION` 传给 .NET 工具和 Rust DLL；启动器继续使用自身项目里的独立版本号。Windows Rust 构建需要 MSVC 与 Windows SDK 的 rc.exe，可用 `TOWER_RC` 指定工具路径。

取得可信代码签名证书后，将证书交由 Windows 证书库、硬件密钥或提供商配置的密钥提供程序管理。不要把 PFX、私钥、密码或 Token PIN 提交到仓库。不要为了消除提示安装自签名根证书。

检查待签文件（只读，不需要证书）：

```powershell
& .\distribution\Sign-Tower.ps1 -PackageRoot 'F:\path\to\stage' -CheckOnly
```

正式打包时添加 `--signing-thumbprint <证书指纹>`，可选 `--signing-store LocalMachine`，默认 CurrentUser。打包流程会先对暂存目录里的爬塔组件签名、验证签名及时间戳，再生成 SHA256 清单、ZIP 和增量数据。证书无效、时间戳失败或验签失败都会停止打包；不传指纹则仍输出未签名构建。

独立暂存目录签名命令：

```powershell
& .\distribution\Sign-Tower.ps1 -PackageRoot 'F:\path\to\stage' -CertificateThumbprint '<证书指纹>'
```

不要对已生成哈希清单或已经发布的文件直接补签；应重新打包。PE 文件使用 SHA-256 和 RFC 3161 时间戳；PowerShell 脚本使用 Set-AuthenticodeSignature，并检查签名和时间戳。默认时间戳服务为 DigiCert，可按证书提供商要求传 `-TimestampUrl`。

签名白名单仅覆盖爬塔自有或自行构建的组件及启动脚本，不修改无缝联机、Steam、WebView2 等第三方原始二进制文件。签名成功后生成 `signing-report.json`，记录文件、证书发布者、指纹和签名后的 SHA256。

Windows 显示的验证发布者来自证书主体，不由 CompanyName 决定。签名不能保证免于安全软件拦截，也不能证明游戏注入行为已被所有安全产品认可。

当前验证范围：已验证元数据构建、待签文件筛选和无证书拒绝路径；由于尚无可信证书，实际签名、硬件密钥交互与在线时间戳仍需取得证书后验证。
