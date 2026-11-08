#!/bin/bash
# Proxy Request and Database Query rows on Ubuntu 24.04: installs nginx,
# HAProxy, Redis, Memcached, Postgres and MySQL, loads the same 100k keys into
# each, and times one connection's requests over loopback with
# `examples/services.rs`. Reconfigures system services, so run it on a
# throwaway VM.
#
#   examples/services.sh          # SECS=10 per target
set -euo pipefail
cd "$(dirname "$0")/.."
SECS=${SECS:-10}
export DEBIAN_FRONTEND=noninteractive
sudo -E apt-get install -y -q nginx haproxy redis-server memcached postgresql mysql-server >/dev/null

# nginx answers on 8080 and proxies to it on 8081; HAProxy proxies to it on 8082.
sudo tee /etc/nginx/sites-enabled/default >/dev/null <<'EOF'
upstream origin { server 127.0.0.1:8080; keepalive 16; keepalive_requests 100000000; }
# nginx closes a keep-alive connection after 1000 requests by default.
keepalive_requests 100000000;
server { listen 8080; access_log off; location / { return 200 "ok"; } }
server {
  listen 8081; access_log off;
  location / { proxy_pass http://origin; proxy_http_version 1.1; proxy_set_header Connection ""; }
}
EOF
sudo systemctl restart nginx
sudo tee /etc/haproxy/haproxy.cfg >/dev/null <<'EOF'
defaults
  mode http
  timeout connect 5s
  timeout client 60s
  timeout server 60s
frontend proxy
  bind 127.0.0.1:8082
  default_backend origin
backend origin
  http-reuse always
  server origin 127.0.0.1:8080
EOF
sudo systemctl restart haproxy

seq 0 99999 | awk '{print "SET k:" $1 " value-" $1}' | redis-cli --pipe >/dev/null
seq 0 99999 | awk '{printf "set k:%d 0 0 %d noreply\r\nvalue-%d\r\n", $1, length("value-" $1), $1}' |
  nc -N 127.0.0.1 11211

sudo -u postgres psql -q -c 'CREATE USER bench' -c 'CREATE DATABASE bench OWNER bench'
sudo -u postgres psql -q -d bench <<'EOF'
CREATE TABLE kv (k int PRIMARY KEY, v text);
INSERT INTO kv SELECT g, 'value-' || g FROM generate_series(0, 99999) g;
ALTER TABLE kv OWNER TO bench;
ANALYZE kv;
EOF
sudo sed -i -E 's/^(host .*)(scram-sha-256|md5)$/\1trust/' /etc/postgresql/*/main/pg_hba.conf
sudo systemctl reload postgresql

sudo mysql <<'EOF'
CREATE DATABASE bench;
CREATE USER 'bench'@'127.0.0.1';
GRANT ALL ON bench.* TO 'bench'@'127.0.0.1';
CREATE TABLE bench.kv (k int PRIMARY KEY, v text);
SET SESSION cte_max_recursion_depth = 100000;
INSERT INTO bench.kv
  WITH RECURSIVE s(n) AS (SELECT 0 UNION ALL SELECT n + 1 FROM s WHERE n < 99999)
  SELECT n, CONCAT('value-', n) FROM s;
EOF

cargo build --release --example services -q
BIN=target/release/examples/services
echo "8080 is nginx directly, 8081 nginx proxying to it, 8082 HAProxy proxying to it"
"$BIN" http=127.0.0.1:8080 http=127.0.0.1:8081 http=127.0.0.1:8082 \
  redis=127.0.0.1:6379 memcached=127.0.0.1:11211 \
  postgres=127.0.0.1:5432 mysql=127.0.0.1:3306 --secs "$SECS"
