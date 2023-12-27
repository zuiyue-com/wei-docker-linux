# 安装 rust

- sed -i 's/archive.ubuntu.com/mirrors.ustc.edu.cn/g' /etc/apt/sources.list
- apt update -y
- apt install build-essential pkg-config libssl-dev --fix-missing -y
- curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
- source "$HOME/.cargo/env"
