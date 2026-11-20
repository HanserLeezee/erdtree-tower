# Cloudflare R2 更新镜像

镜像：`https://erdtree.paper2gal.com/releases`；桶：`erdtree-tower-releases`。
仅保留 2.1.0（v115）及之后的已发布版本清单；更早的数据包仅作为这些版本的增量依赖保留，不能按版本号批量删除。

主镜像发版使用 `upload_r2.py`。凭据使用本机 Wrangler 登录，不放入代码或发布包。
阿里云 `eldenring-mods-hz` 保留为需用户勾选开启的应急备用节点：不参与自动选线，仅在主线路均失败时调用。
正式发布时也需同步阿里云备用节点的增量包和清单（沿用既有 OSS 上传助手，先版本文件、后 latest），避免备用节点缺少新版本数据；仍需保留清单依赖的历史包。

```powershell
python distribution/upload_r2.py dist/mirror-v<N> --phase files
python distribution/upload_r2.py dist/mirror-v<N> --phase files --apply
# 版本文件及历史依赖通过校验后，最后更新 latest：
python distribution/upload_r2.py dist/mirror-v<N> --phase latest --apply
python distribution/check_mirror.py https://erdtree.paper2gal.com/releases --expect <N>
```

未全局安装 Wrangler 时，传入 `--wrangler <bin/wrangler.js>`。上传脚本先验证本地包、拒绝覆盖不同的版本文件，在发布 latest 前验证历史包的 HTTP 206、解压和 SHA-256，并拒绝降级 latest。

旧启动器仍读取自己的 `launcher/sources.json`。迁移存储不会自动改写玩家磁盘上的地址；需要安装换源补丁，或随下一次正式更新下发新配置。GitHub 备用线路不变，阿里云旧文件暂留以兼容旧配置。
