# RustMail - 高性能邮件服务器

[![CI](https://github.com/Lewen-Cai/RustMail-Light/actions/workflows/ci.yml/badge.svg)](https://github.com/Lewen-Cai/RustMail-Light/actions/workflows/ci.yml)

RustMail 是一个使用 Rust 编写的现代邮件服务器，专注于性能、内存安全和易用性。

## 特性

- 🚀 **高性能** - Rust 异步运行时，资源占用低
- 🔒 **安全可靠** - 内存安全保证，内置 DKIM/SPF/DMARC 支持
- 🐳 **容器化部署** - Docker Compose 一键启动
- 🌐 **现代 Web UI** - React + TypeScript 构建的管理界面
- 📧 **完整协议支持** - SMTP、IMAP、POP3 全支持
- ⚡ **自动 HTTPS** - Caddy 自动申请和续期 SSL 证书

## 快速开始

### 一键安装（推荐）

在您的服务器上执行：

```bash
curl -fsSL https://raw.githubusercontent.com/Lewen-Cai/RustMail-Light/main/scripts/install-zh.sh | bash
```

脚本会自动：
- ✅ 检测系统环境（Docker、端口等）
- ✅ 交互式配置域名和邮箱
- ✅ 自动生成安全密钥和 DKIM
- ✅ 下载并启动所有服务
- ✅ 输出 DNS 配置指南

### 手动安装

1. 克隆仓库

```bash
git clone https://github.com/Lewen-Cai/RustMail-Light.git
cd RustMail-Light
```

2. 构建 Web UI

```bash
cd web/mail-ui
npm install
npm run build
cd ../..
```

3. 准备配置文件

```bash
mkdir -p config
mkdir -p data/mail
mkdir -p dkim

# 复制示例配置
cp config.example.toml config/rustmaild.toml

# 编辑配置
vim config/rustmaild.toml
```

4. 创建环境文件 `.env`

```dotenv
DOMAIN=mail.example.com
ACME_EMAIL=admin@example.com
POSTGRES_DB=rustmail
POSTGRES_USER=rustmail
POSTGRES_PASSWORD=your_secure_password
REDIS_PASSWORD=your_secure_password
```

5. 启动服务

```bash
cd deploy
docker-compose --env-file ../.env up -d
```

## 配置指南

### 主配置文件 (`config/rustmaild.toml`)

```toml
[database]
url = "postgres://rustmail:password@postgres:5432/rustmail"
max_connections = 20

[smtp]
bind = "0.0.0.0:25"           # SMTP 端口
submission_bind = "0.0.0.0:587"  # Submission 端口
smtps_bind = "0.0.0.0:465"      # SMTPS 端口
starttls = true

[imap]
bind = "0.0.0.0:993"          # IMAPS 端口

[pop3]
bind = "0.0.0.0:995"          # POP3S 端口

[api]
bind = "0.0.0.0:8080"         # HTTP API 端口

[auth]
jwt_secret = "your-secret-key"

[logging]
level = "info"
format = "json"
```

## DNS 配置

假设您的域名是 `example.com`，邮件服务器域名是 `mail.example.com`：

### A/AAAA 记录
```
mail.example.com → 你的服务器 IP 地址
```

### MX 记录
```
example.com    优先级: 10    值: mail.example.com
```

### SPF 记录 (TXT)
```
example.com    TXT    "v=spf1 mx -all"
```

### DKIM 记录 (TXT)
```
mail._domainkey.example.com    TXT    "v=DKIM1; k=rsa; p=<公钥内容>"
```

安装脚本会自动生成 DKIM 密钥对，并显示公钥内容。

### DMARC 记录 (TXT)
```
_dmarc.example.com    TXT    "v=DMARC1; p=quarantine; rua=mailto:postmaster@example.com"
```

## 服务架构

```
┌─────────────────────────────────────────────┐
│                   Caddy                      │
│         (反向代理 + HTTPS + 静态文件)          │
│                  :80, :443                   │
└───────────────────┬─────────────────────────┘
                    │
        ┌───────────┼───────────┐
        ▼           ▼           ▼
   ┌─────────┐ ┌─────────┐ ┌──────────┐
   │   API   │ │   UI    │ │  Webmail │
   │  :8080  │ │(静态文件)│ │          │
   └────┬────┘ └─────────┘ └──────────┘
        │
        ▼
┌─────────────────────────────────────────────┐
│                rustmaild                     │
│        (SMTP + IMAP + POP3 + API)            │
│        :25, :465, :587, :993, :995          │
└───────────────────┬─────────────────────────┘
        │           │
        ▼           ▼
   ┌─────────┐ ┌─────────┐
   │PostgreSQL│ │  Redis  │
   │  :5432   │ │  :6379  │
   └─────────┘ └─────────┘
```

## 常用命令

```bash
# 查看日志
cd /opt/rustmail/deploy
docker-compose logs -f

# 停止服务
docker-compose stop

# 启动服务
docker-compose up -d

# 重启服务
docker-compose restart

# 查看状态
docker-compose ps
```

## 开发

### 后端开发

```bash
# 检查代码
cargo check --workspace

# 运行测试
cargo test --workspace

# 运行 clippy
cargo clippy --workspace --all-targets -- -D warnings

# 本地运行
cargo run -p rustmaild -- --config config/rustmaild.toml
```

### 前端开发

```bash
cd web/mail-ui
npm install
npm run dev
```

## 项目结构

```
RustMail-Light/
├── crates/
│   ├── core-domain/      # 领域模型
│   ├── core-auth/        # 认证模块
│   ├── core-storage/     # 存储层
│   ├── core-policy/      # 策略引擎
│   ├── proto-smtp/       # SMTP 协议
│   ├── proto-imap/       # IMAP 协议
│   ├── proto-pop3/       # POP3 协议
│   ├── service-api/      # HTTP API
│   ├── service-worker/   # 后台任务
│   └── app-rustmaild/    # 主程序
├── web/
│   └── mail-ui/          # Web 前端
├── deploy/
│   ├── Dockerfile        # Docker 构建
│   ├── docker-compose.yml # 编排配置
│   └── Caddyfile         # Caddy 配置
├── scripts/
│   ├── install.sh        # 英文安装脚本
│   └── install-zh.sh     # 中文安装脚本
├── migrations/           # 数据库迁移
└── config.example.toml   # 配置示例
```

## 系统要求

- **操作系统**: Linux (推荐 Ubuntu 20.04+) 或 macOS
- **Docker**: 20.10+
- **Docker Compose**: 2.0+
- **内存**: 至少 2GB RAM
- **磁盘**: 至少 10GB 可用空间
- **端口**: 25, 465, 587, 993, 995, 80, 443 需要可用

## 安全建议

1. **防火墙配置**: 只开放必要的端口
2. **定期备份**: 备份 `data/` 和 `config/` 目录
3. **密钥管理**: 妥善保管 `.env` 和 DKIM 私钥
4. **更新维护**: 定期拉取最新镜像更新

## 许可证

本项目采用 MIT 或 Apache-2.0 双许可证。

## 社区与支持

- 📖 [英文文档](README.md)
- 🐛 [问题反馈](https://github.com/Lewen-Cai/RustMail-Light/issues)
- 💬 [Discussions](https://github.com/Lewen-Cai/RustMail-Light/discussions)

---

Made with ❤️ by RustMail Team
