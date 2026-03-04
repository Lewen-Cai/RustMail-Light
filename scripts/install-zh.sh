#!/usr/bin/env bash
#
# RustMail 一键安装脚本（中文版）
# 支持交互式配置和环境检测
#

set -euo pipefail

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 打印带颜色的消息
print_error() { echo -e "${RED}✗ $1${NC}" >&2; }
print_success() { echo -e "${GREEN}✓ $1${NC}"; }
print_warning() { echo -e "${YELLOW}⚠ $1${NC}"; }
print_info() { echo -e "${BLUE}ℹ $1${NC}"; }

# 欢迎信息
cat << 'EOF'
╔═══════════════════════════════════════════════════════════╗
║                                                           ║
║   RustMail - 高性能邮件服务器                              ║
║   现代、安全、易用的自托管邮件解决方案                      ║
║                                                           ║
╚═══════════════════════════════════════════════════════════╝

EOF

print_info "开始环境检测..."

# 1. 检测操作系统
OS_NAME="$(uname -s)"
case "${OS_NAME}" in
  Linux)
    print_success "操作系统: Linux"
    ;;
  Darwin)
    print_success "操作系统: macOS"
    ;;
  *)
    print_error "不支持的操作系统: ${OS_NAME}"
    exit 1
    ;;
esac

# 2. 检测必要命令
MISSING_CMDS=()
for cmd in curl openssl; do
  if ! command -v "${cmd}" >/dev/null 2>&1; then
    MISSING_CMDS+=("${cmd}")
  fi
done

if [ ${#MISSING_CMDS[@]} -ne 0 ]; then
  print_error "缺少必要工具: ${MISSING_CMDS[*]}"
  print_info "请使用包管理器安装:"
  if [[ "${OS_NAME}" == "Linux" ]]; then
    if command -v apt-get >/dev/null 2>&1; then
      echo "  sudo apt-get install -y curl openssl"
    elif command -v yum >/dev/null 2>&1; then
      echo "  sudo yum install -y curl openssl"
    elif command -v apk >/dev/null 2>&1; then
      echo "  apk add curl openssl"
    fi
  elif [[ "${OS_NAME}" == "Darwin" ]]; then
    echo "  brew install curl openssl"
  fi
  exit 1
fi
print_success "必要工具已安装 (curl, openssl)"

# 3. 检测 Docker
if ! command -v docker >/dev/null 2>&1; then
  print_error "未检测到 Docker"
  echo ""
  print_info "请安装 Docker:"
  echo "  官方安装脚本:"
  echo "    curl -fsSL https://get.docker.com | sh"
  echo ""
  echo "  或使用包管理器:"
  if [[ "${OS_NAME}" == "Linux" ]]; then
    if command -v apt-get >/dev/null 2>&1; then
      echo "    sudo apt-get install -y docker.io"
    elif command -v yum >/dev/null 2>&1; then
      echo "    sudo yum install -y docker"
    fi
  fi
  exit 1
fi
print_success "Docker 已安装"

# 4. 检测 Docker 守护进程
if ! docker info >/dev/null 2>&1; then
  print_error "Docker 守护进程未运行或当前用户无权限"
  print_info "请尝试:"
  echo "  sudo systemctl start docker    # Linux systemd"
  echo "  sudo service docker start        # Linux SysV"
  echo "  或将当前用户加入 docker 组:"
  echo "  sudo usermod -aG docker \$USER && newgrp docker"
  exit 1
fi
print_success "Docker 守护进程运行正常"

# 5. 检测 Docker Compose
COMPOSE_CMD=()
if docker compose version >/dev/null 2>&1; then
  COMPOSE_CMD=(docker compose)
  print_success "Docker Compose 插件已安装"
elif command -v docker-compose >/dev/null 2>&1; then
  COMPOSE_CMD=(docker-compose)
  print_success "Docker Compose (standalone) 已安装"
else
  print_error "未检测到 Docker Compose"
  print_info "请安装 Docker Compose:"
  echo "  官方文档: https://docs.docker.com/compose/install/"
  exit 1
fi

# 6. 检测端口占用
print_info "检测端口占用情况..."
PORTS=(25 465 587 993 995 80 443)
PORT_CONFLICT=false
for port in "${PORTS[@]}"; do
  if command -v ss >/dev/null 2>&1; then
    if ss -tln | grep -q ":${port} "; then
      print_warning "端口 ${port} 已被占用"
      PORT_CONFLICT=true
    fi
  elif command -v netstat >/dev/null 2>&1; then
    if netstat -tln 2>/dev/null | grep -q ":${port} "; then
      print_warning "端口 ${port} 已被占用"
      PORT_CONFLICT=true
    fi
  fi
done

if [ "${PORT_CONFLICT}" = true ]; then
  print_warning "检测到端口冲突，RustMail 可能无法正常启动"
  read -rp "是否继续安装? [y/N]: " continue_install
  if [[ ! "${continue_install}" =~ ^[Yy]$ ]]; then
    print_info "安装已取消"
    exit 0
  fi
fi

echo ""
print_success "环境检测通过!"
echo ""

# ===========================================
# 交互式配置
# ===========================================

print_info "开始配置 RustMail..."
echo ""

# 安装目录
read -rp "安装目录 [/opt/rustmail]: " input_install_dir
INSTALL_DIR="${input_install_dir:-/opt/rustmail}"
if [ ! -w "$(dirname "${INSTALL_DIR}")" ]; then
  INSTALL_DIR="${HOME}/rustmail"
  print_warning "无 /opt 写入权限，使用 ${INSTALL_DIR}"
fi

# 域名配置
echo ""
print_info "域名配置 (用于邮件服务和 HTTPS)"
read -rp "邮件服务器域名 [mail.example.com]: " input_domain
DOMAIN="${input_domain:-mail.example.com}"

# 从域名自动推断根域名
if [[ "${DOMAIN}" == mail.* ]]; then
  DEFAULT_ROOT_DOMAIN="${DOMAIN#mail.}"
else
  DEFAULT_ROOT_DOMAIN="${DOMAIN}"
fi
read -rp "根域名 (用于 MX 记录) [${DEFAULT_ROOT_DOMAIN}]: " input_root_domain
ROOT_DOMAIN="${input_root_domain:-${DEFAULT_ROOT_DOMAIN}}"

# 邮箱配置
echo ""
read -rp "管理员邮箱 [admin@${ROOT_DOMAIN}]: " input_email
ACME_EMAIL="${input_email:-admin@${ROOT_DOMAIN}}"

# 确认配置
echo ""
echo "═══════════════════════════════════════════════════════════"
print_info "配置确认:"
echo "  安装目录: ${INSTALL_DIR}"
echo "  邮件域名: ${DOMAIN}"
echo "  根域名:   ${ROOT_DOMAIN}"
echo "  管理员邮箱: ${ACME_EMAIL}"
echo "═══════════════════════════════════════════════════════════"
echo ""

read -rp "确认以上配置并继续安装? [Y/n]: " confirm
if [[ "${confirm}" =~ ^[Nn]$ ]]; then
  print_info "安装已取消"
  exit 0
fi

# ===========================================
# 开始安装
# ===========================================

echo ""
print_info "开始安装 RustMail..."

# 创建目录
mkdir -p "${INSTALL_DIR}/deploy" "${INSTALL_DIR}/config" "${INSTALL_DIR}/dkim" "${INSTALL_DIR}/data/mail" "${INSTALL_DIR}/web/mail-ui/dist"
print_success "创建目录结构"

# 下载部署文件
REPO_RAW_BASE="${RUSTMAIL_REPO_RAW:-https://raw.githubusercontent.com/Lewen-Cai/RustMail-Light/main}"

print_info "下载配置文件..."
if ! curl -fsSL "${REPO_RAW_BASE}/deploy/docker-compose.yml" -o "${INSTALL_DIR}/deploy/docker-compose.yml"; then
  print_error "下载 docker-compose.yml 失败"
  exit 1
fi
print_success "下载 docker-compose.yml"

if ! curl -fsSL "${REPO_RAW_BASE}/deploy/Caddyfile" -o "${INSTALL_DIR}/deploy/Caddyfile"; then
  print_error "下载 Caddyfile 失败"
  exit 1
fi
print_success "下载 Caddyfile"

# 生成密钥和密码
print_info "生成安全密钥..."
JWT_SECRET="$(openssl rand -hex 32)"
POSTGRES_PASSWORD="$(openssl rand -hex 18)"
REDIS_PASSWORD="$(openssl rand -hex 18)"

# 创建 .env 文件
cat > "${INSTALL_DIR}/.env" <<EOF
DOMAIN=${DOMAIN}
ACME_EMAIL=${ACME_EMAIL}
POSTGRES_DB=rustmail
POSTGRES_USER=rustmail
POSTGRES_PASSWORD=${POSTGRES_PASSWORD}
REDIS_PASSWORD=${REDIS_PASSWORD}
EOF
print_success "创建环境配置文件"

# 创建 rustmaild.toml
cat > "${INSTALL_DIR}/config/rustmaild.toml" <<EOF
[database]
url = "postgres://rustmail:${POSTGRES_PASSWORD}@postgres:5432/rustmail"
max_connections = 20

[smtp]
bind = "0.0.0.0:25"
submission_bind = "0.0.0.0:587"
smtps_bind = "0.0.0.0:465"
starttls = true

[imap]
bind = "0.0.0.0:993"

[pop3]
bind = "0.0.0.0:995"

[api]
bind = "0.0.0.0:8080"

[auth]
jwt_secret = "${JWT_SECRET}"

[logging]
level = "info"
format = "json"
EOF
print_success "创建主配置文件"

# 生成 DKIM 密钥
if [ ! -f "${INSTALL_DIR}/dkim/private.key" ]; then
  print_info "生成 DKIM 密钥对..."
  openssl genrsa -out "${INSTALL_DIR}/dkim/private.key" 2048 >/dev/null 2>&1
  openssl rsa -in "${INSTALL_DIR}/dkim/private.key" -pubout -out "${INSTALL_DIR}/dkim/public.pem" >/dev/null 2>&1
  print_success "DKIM 密钥生成完成"
fi

DKIM_PUBLIC_KEY="$(grep -v '-----' "${INSTALL_DIR}/dkim/public.pem" | tr -d '\n')"

# 创建默认首页
cat > "${INSTALL_DIR}/web/mail-ui/dist/index.html" <<'EOF'
<!doctype html>
<html lang="zh-CN">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>RustMail - 邮件服务器</title>
    <style>
      body {
        font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
        max-width: 800px;
        margin: 50px auto;
        padding: 20px;
        text-align: center;
        color: #333;
      }
      h1 { color: #2563eb; }
      .success { color: #16a34a; }
      .info-box {
        background: #f3f4f6;
        border-radius: 8px;
        padding: 20px;
        margin: 20px 0;
        text-align: left;
      }
      code {
        background: #e5e7eb;
        padding: 2px 6px;
        border-radius: 4px;
        font-family: monospace;
      }
    </style>
  </head>
  <body>
    <h1>🎉 RustMail 安装成功!</h1>
    <p class="success">您的邮件服务器正在运行</p>
    <div class="info-box">
      <h3>下一步操作:</h3>
      <ol>
        <li>配置 DNS 记录 (详见安装输出)</li>
        <li>构建完整的 Web UI (可选)</li>
        <li>创建管理员账户</li>
      </ol>
    </div>
    <p>配置文件位置: <code>config/rustmaild.toml</code></p>
  </body>
</html>
EOF
print_success "创建默认页面"

# 启动服务
echo ""
print_info "启动 RustMail 服务..."
(
  cd "${INSTALL_DIR}/deploy"
  if ! "${COMPOSE_CMD[@]}" --env-file ../.env pull; then
    print_warning "拉取镜像失败，将尝试使用本地镜像"
  fi
  if ! "${COMPOSE_CMD[@]}" --env-file ../.env up -d; then
    print_error "启动服务失败"
    exit 1
  fi
)
print_success "RustMail 服务已启动"

# 输出完成信息
echo ""
cat << EOF
╔═══════════════════════════════════════════════════════════╗
║                    🎉 安装完成! 🎉                         ║
╚═══════════════════════════════════════════════════════════╝

📍 安装目录: ${INSTALL_DIR}

📋 请配置以下 DNS 记录:

  A/AAAA 记录:
    ${DOMAIN} → $(curl -s https://api.ipify.org 2>/dev/null || echo "你的服务器IP")

  MX 记录:
    ${ROOT_DOMAIN}    优先级:10    值:${DOMAIN}

  TXT 记录 (SPF):
    ${ROOT_DOMAIN}    值:"v=spf1 mx -all"

  TXT 记录 (DKIM):
    mail._domainkey.${ROOT_DOMAIN}    值:"v=DKIM1; k=rsa; p=${DKIM_PUBLIC_KEY}"

  TXT 记录 (DMARC):
    _dmarc.${ROOT_DOMAIN}    值:"v=DMARC1; p=quarantine; rua=mailto:postmaster@${ROOT_DOMAIN}"

📁 配置文件位置:
  - 环境变量: ${INSTALL_DIR}/.env
  - 主配置:   ${INSTALL_DIR}/config/rustmaild.toml
  - DKIM私钥: ${INSTALL_DIR}/dkim/private.key

🔧 常用命令:
  cd ${INSTALL_DIR}/deploy && ${COMPOSE_CMD[*]} logs -f  # 查看日志
  cd ${INSTALL_DIR}/deploy && ${COMPOSE_CMD[*]} stop     # 停止服务
  cd ${INSTALL_DIR}/deploy && ${COMPOSE_CMD[*]} up -d    # 启动服务

🌐 Web 界面:
  https://${DOMAIN}

EOF

print_success "RustMail 安装完成!"
