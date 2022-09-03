#!/usr/bin/env bash

. ~/.bashrc
GO_VERSION="1.18"

apt-get update

set -x
apt-get install -o Dpkg::Options::="--force-overwrite" -y \
  ruby-full \
  python3.10-full \
  python3-pip \
  sqlite3 \
  libsqlite3-dev \
  gnuplot-nox \
  linux-tools-$(uname -r) \
  linux-cloud-tools-$(uname -r) \
  build-essential \
  protobuf-compiler \
  automake \
  autoconf \
  cmake \
  gettext \
  ninja-build \
  libtool \
  libtool-bin \
  doxygen \
  bat \
  hexyl \
  fd-find \
  ripgrep \
  universal-ctags \
  gdb \
  clang \
  libssl-dev \
  pkg-config \
  valgrind \
  bpftrace \
  bpfcc-tools \
  ca-certificates \
  gnupg \
  curl \
  lsb-release \
  python-is-python3 \
  tmux \
  nasm \
  msr-tools \
  software-properties-common

# The command-line name has a conflict by default
if ! command -v fd; then
  ln -s $(which fdfind) /usr/local/bin/fd
fi

if ! command -v bat; then
  ln -s $(which batcat) /usr/local/bin/bat
fi

if ! command -v pastel; then
  wget "https://github.com/sharkdp/pastel/releases/download/v0.8.1/pastel_0.8.1_amd64.deb"
  sudo dpkg -i pastel_0.8.1_amd64.deb
  rm pastel*
fi

if ! command -v psql; then
  sudo sh -c "echo 'deb http://apt.postgresql.org/pub/repos/apt $(lsb_release -cs)-pgdg main' > /etc/apt/sources.list.d/pgdg.list"
  wget --quiet -O - https://www.postgresql.org/media/keys/ACCC4CF8.asc | sudo apt-key add -
  apt-get update
  apt-get -y install postgresql
  systemctl start postgresql.service
  sudo -u postgres createuser --superuser root || true
  sudo -u postgres createdb root || true
  echo 'host    all             all             0.0.0.0/0               trust' >> /etc/postgresql/*/main/pg_hba.conf

  # listen on tailscale IP
fi

if ! command -v hyperfine; then
  wget https://github.com/sharkdp/hyperfine/releases/download/v1.13.0/hyperfine_1.13.0_amd64.deb
  sudo dpkg -i hyperfine_1.13.0_amd64.deb
  rm hyperfine*
fi

if ! command -v go; then
  wget -nc https://go.dev/dl/go$GO_VERSION.linux-amd64.tar.gz
  sudo tar -C /usr/local -xzf go$GO_VERSION.linux-amd64.tar.gz

  echo 'export PATH=\$PATH:/usr/local/go/bin' >> ~/.bashrc
  rm *.tar.gz

  go install golang.org/x/tools/cmd/goimports@latest
fi

if ! command -v rustc; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
fi

if ! command -v docker; then
  curl -fsSL https://download.docker.com/linux/ubuntu/gpg | sudo gpg --dearmor -o /usr/share/keyrings/docker-archive-keyring.gpg
  echo \
    "deb [arch=\$(dpkg --print-architecture) signed-by=/usr/share/keyrings/docker-archive-keyring.gpg] https://download.docker.com/linux/ubuntu \
    $(lsb_release -cs) stable" | sudo tee /etc/apt/sources.list.d/docker.list > /dev/null

  apt-get update
  apt-get install -y docker-ce docker-ce-cli containerd.io
fi

if ! command -v docker-compose; then
  # https://docs.docker.com/compose/install/
  curl -L "https://github.com/docker/compose/releases/download/1.29.2/docker-compose-$(uname -s)-\$(uname -m)" -o /usr/local/bin/docker-compose
  chmod +x /usr/local/bin/docker-compose
fi

if ! command -v nvim; then
  (
    cd /tmp
    if [[ ! -d /tmp/neovim ]]; then
      git clone https://github.com/neovim/neovim
    fi
    cd neovim
    git checkout release-0.7
    make clean
    make CMAKE_BUILD_TYPE=Release
    sudo make install
  )
  # run :checkhealth!
fi

if ! command -v delta; then
  wget https://github.com/dandavison/delta/releases/download/0.12.1/git-delta_0.12.1_amd64.deb
  dpkg -i git-delta_0.12.1_amd64.deb
fi

if ! command -v tailscale; then
  curl -fsSL https://pkgs.tailscale.com/stable/ubuntu/jammy.noarmor.gpg | sudo tee /usr/share/keyrings/tailscale-archive-keyring.gpg >/dev/null
  curl -fsSL https://pkgs.tailscale.com/stable/ubuntu/jammy.tailscale-keyring.list | sudo tee /etc/apt/sources.list.d/tailscale.list

  # https://tailscale.com/kb/1103/exit-nodes/
  echo 'net.ipv4.ip_forward = 1' | sudo tee -a /etc/sysctl.conf
  echo 'net.ipv6.conf.all.forwarding = 1' | sudo tee -a /etc/sysctl.conf
  sudo sysctl -p /etc/sysctl.conf

  sudo apt-get update
  sudo apt-get install tailscale

  sudo tailscale up --advertise-exit-node
fi

if ! type nvm; then
  curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.1/install.sh | bash
  nvm install stable
  npm install -g eslint eslint_d neovim
fi

if ! command -v fzf; then
  git clone --depth 1 https://github.com/junegunn/fzf.git ~/.fzf
  ~/.fzf/install
fi

if ! type chruby; then
  echo "installing chruby.."
  (
    wget -O chruby-0.3.9.tar.gz https://github.com/postmodern/chruby/archive/v0.3.9.tar.gz
    tar -xzvf chruby-0.3.9.tar.gz
    cd chruby-0.3.9/
    sudo make install
    cd ..

    wget -O ruby-install-0.8.3.tar.gz https://github.com/postmodern/ruby-install/archive/v0.8.3.tar.gz
    tar -xzvf ruby-install-0.8.3.tar.gz
    cd ruby-install-0.8.3/
    sudo make install

    . ~/.bashrc

    ruby-install --jobs $(nproc) ruby
    gem install bundler

    cd ..
    rm -rf chruby-*
    rm -rf ruby-install-*
    exit 0
  )
fi

if [ ! -d ~/pmu-tools ]; then
  git clone https://github.com/andikleen/pmu-tools ~/pmu-tools
  echo 'export PATH=\$PATH:~/pmu-tools' >> ~/.bashrc
fi

if [ ! -d ~/uarch-bench ]; then
  git clone --recursive https://github.com/travisdowns/uarch-bench ~/uarch-bench
fi

if [ ! -d ~/likwid ]; then
  git clone https://github.com/RRZE-HPC/likwid ~/likwid
fi

if [ ! -d ~/bcc ]; then
  git clone https://github.com/iovisor/bcc ~/bcc
fi

if [ ! -d ~/flamegraph ]; then
  # https://www.brendangregg.com/FlameGraphs/cpuflamegraphs.html#Instructions
  git clone https://github.com/brendangregg/FlameGraph ~/flamegraph
  echo 'export PATH=\$PATH:~/flamegraph' >> ~/.bashrc
fi

# Need master for some coloring issues
if ! command -v mosh; then
  git clone https://github.com/mobile-shell/mosh
  (
    cd mosh
    ./autogen.sh
    ./configure
    make
    make install
  )
  rm -rf mosh
fi

# null-ls: require("null-ls.health").check()
# ========================================================================
#   - ERROR: flake8: the command "flake8" is not executable.
#   - ERROR: hadolint: the command "hadolint" is not executable.
#   - ERROR: jsonlint: the command "jsonlint" is not executable.
#   - ERROR: autopep8: the command "autopep8" is not executable.
#   - ERROR: isort: the command "isort" is not executable.
#   - ERROR: fixjson: the command "fixjson" is not executable.
#   - ERROR: stylua: the command "stylua" is not executable.
#   - INFO: gitsigns: cannot verify if the command is an executable.
#   - ERROR: goimports: the command "goimports" is not executable.
if ! command -v isort; then
  pip install flake8 autopep8 isort
fi

if ! command -v fixjson; then
  npm install -g jsonlint fixjson
fi

if ! command -v hadolint; then
  curl https://github.com/hadolint/hadolint/releases/download/v2.10.0/hadolint-Linux-x86_64 -o /usr/bin/hadolint
  chmod +x /usr/bin/hadolint
fi

if ! command -v stylua; then
  cargo install stylua
fi

if ! command -v tree-sitter-cli; then
  cargo install tree-sitter-cli
fi

if ! command -v goimports; then
  go install golang.org/x/tools/cmd/goimports@latest
fi

apt-get upgrade -y
apt-get autoremove -y
