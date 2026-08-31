#!/usr/bin/env bash
set -euo pipefail

state_dir=${OPENOT_DATABASE_RUNNER_STATE_DIR:-${RUNNER_TEMP:-/tmp}/trust-openot-databases}
prefix=${OPENOT_DATABASE_RUNNER_PREFIX:-trust-openot-${GITHUB_RUN_ID:-local}}
github_env=${GITHUB_ENV:?GITHUB_ENV must identify the GitHub Actions environment file}
postgres_image='postgres:18.6@sha256:4ef4dbc939d61acea57712655ddb4b4ab27419c913f94cca0cd57cb3ea3c2280'
timescale_image='timescale/timescaledb:2.29.2-pg18@sha256:9508616d5b941ed931198504c5db3fb47e8f53f790732ea1e889591f1062057c'
mysql_image='mysql:8.4.11@sha256:b3b90af2a6552ae30c266fdb7d5dd55f3afb72404bb78d37fe8a23eb857fd3fb'
mariadb_image='mariadb:11.8.8@sha256:24e76fcec8c003a0362d0dd53f4806e7e79458d7fdeaf47437760e19496f5a9c'
sqlserver_image='mcr.microsoft.com/mssql/server:2025-CU8-ubuntu-22.04@sha256:2f9da673779dc5556d385164f6b1541d169ff1eeed97b9833ca0308e8628e683'
influx_image='influxdb:3.11.2-core@sha256:f4a6d4a76f0ed0a196cc997da472cd0b7ae52a766430493a1bead807ab8c1217'
nginx_image='nginx:1.29.3-alpine@sha256:b3c656d55d7ad751196f21b7fd2e8d4da9cb430e32f646adcf92441b72f82b14'

export OPENOT_DATABASE_RUNNER_STATE_DIR=$state_dir
export OPENOT_DATABASE_RUNNER_PREFIX=$prefix
scripts/openot_database_runner_teardown.sh
if [[ -e $state_dir ]]; then
  echo "refusing unowned or incompletely cleaned runner state: $state_dir" >&2
  exit 2
fi
mkdir -p "$state_dir/tls" "$state_dir/nginx" "$state_dir/nginx-tls"
chmod 700 "$state_dir" "$state_dir/tls"
printf '%s\n' "$prefix" >"$state_dir/.trust-openot-runner-state"

password=$(openssl rand -hex 24)
sqlserver_password="A1!$(openssl rand -hex 20)z"

openssl genrsa -out "$state_dir/tls/ca.key" 2048 >/dev/null 2>&1
openssl req -x509 -new -sha256 -days 2 \
  -key "$state_dir/tls/ca.key" -out "$state_dir/tls/ca.pem" \
  -subj "/CN=truST OpenOT ephemeral CA" >/dev/null 2>&1
openssl genrsa -out "$state_dir/tls/server.key" 2048 >/dev/null 2>&1
openssl req -new -key "$state_dir/tls/server.key" \
  -out "$state_dir/tls/server.csr" -subj "/CN=localhost" >/dev/null 2>&1
cat >"$state_dir/tls/server.ext" <<'EOF'
subjectAltName=DNS:localhost,IP:127.0.0.1
extendedKeyUsage=serverAuth
EOF
openssl x509 -req -sha256 -days 2 -in "$state_dir/tls/server.csr" \
  -CA "$state_dir/tls/ca.pem" -CAkey "$state_dir/tls/ca.key" -CAcreateserial \
  -extfile "$state_dir/tls/server.ext" -out "$state_dir/tls/server.crt" >/dev/null 2>&1
chmod 640 "$state_dir/tls/server.key"
chmod 644 "$state_dir/tls/ca.pem" "$state_dir/tls/server.crt"

docker network create "$prefix-network" >/dev/null

prepare_postgres_tls() {
  local image=$1 directory=$2 uid gid
  uid=$(docker run --rm --entrypoint sh "$image" -c 'id -u postgres')
  gid=$(docker run --rm --entrypoint sh "$image" -c 'id -g postgres')
  mkdir -p "$directory"
  cp "$state_dir/tls/server.crt" "$directory/server.crt"
  cp "$state_dir/tls/server.key" "$directory/server.key"
  chmod 644 "$directory/server.crt"
  chmod 600 "$directory/server.key"
  sudo chown "$uid:$gid" "$directory/server.crt" "$directory/server.key"
}
prepare_postgres_tls "$postgres_image" "$state_dir/postgres-tls"
prepare_postgres_tls "$timescale_image" "$state_dir/timescale-tls"

docker run -d --name "$prefix-postgres" --network "$prefix-network" \
  -p 127.0.0.1:55432:5432 \
  -e POSTGRES_PASSWORD="$password" -e POSTGRES_DB=openot \
  -v "$state_dir/postgres-tls:/tls:ro" "$postgres_image" \
  -c ssl=on -c ssl_cert_file=/tls/server.crt -c ssl_key_file=/tls/server.key >/dev/null

docker run -d --name "$prefix-timescale" --network "$prefix-network" \
  -p 127.0.0.1:55433:5432 \
  -e POSTGRES_PASSWORD="$password" -e POSTGRES_DB=openot \
  -v "$state_dir/timescale-tls:/tls:ro" "$timescale_image" \
  -c ssl=on -c ssl_cert_file=/tls/server.crt -c ssl_key_file=/tls/server.key >/dev/null

prepare_mysql_tls() {
  local image=$1 directory=$2 uid gid
  uid=$(docker run --rm --entrypoint sh "$image" -c 'id -u mysql')
  gid=$(docker run --rm --entrypoint sh "$image" -c 'id -g mysql')
  mkdir -p "$directory"
  cp "$state_dir/tls/ca.pem" "$directory/ca.pem"
  cp "$state_dir/tls/server.crt" "$directory/server.crt"
  cp "$state_dir/tls/server.key" "$directory/server.key"
  chmod 755 "$directory"
  chmod 644 "$directory/ca.pem" "$directory/server.crt"
  chmod 600 "$directory/server.key"
  sudo chown "$uid:$gid" "$directory/ca.pem" "$directory/server.crt" "$directory/server.key"
}
prepare_mysql_tls "$mysql_image" "$state_dir/mysql-tls"
prepare_mysql_tls "$mariadb_image" "$state_dir/mariadb-tls"
docker run -d --name "$prefix-mysql" --network "$prefix-network" \
  -p 127.0.0.1:53306:3306 -e MYSQL_ROOT_PASSWORD="$password" \
  -e MYSQL_DATABASE=openot -v "$state_dir/mysql-tls:/tls:ro" "$mysql_image" \
  --ssl-ca=/tls/ca.pem --ssl-cert=/tls/server.crt --ssl-key=/tls/server.key \
  --require-secure-transport=ON >/dev/null

docker run -d --name "$prefix-mariadb" --network "$prefix-network" \
  -p 127.0.0.1:53307:3306 -e MARIADB_ROOT_PASSWORD="$password" \
  -e MARIADB_DATABASE=openot -v "$state_dir/mariadb-tls:/tls:ro" "$mariadb_image" \
  --ssl-ca=/tls/ca.pem --ssl-cert=/tls/server.crt --ssl-key=/tls/server.key \
  --require-secure-transport=ON >/dev/null

mkdir -p "$state_dir/sqlserver"
cp "$state_dir/tls/server.crt" "$state_dir/sqlserver/server.crt"
cp "$state_dir/tls/server.key" "$state_dir/sqlserver/server.key"
cat >"$state_dir/sqlserver/mssql.conf" <<'EOF'
[network]
forceencryption = 1
tlscert = /var/opt/mssql/secrets/server.crt
tlskey = /var/opt/mssql/secrets/server.key
tlsprotocols = 1.2
EOF
chmod 644 "$state_dir/sqlserver/server.crt" "$state_dir/sqlserver/mssql.conf"
chmod 600 "$state_dir/sqlserver/server.key"
sudo chown -R 10001:0 "$state_dir/sqlserver"
docker run -d --name "$prefix-sqlserver" --network "$prefix-network" \
  -p 127.0.0.1:51433:1433 -e ACCEPT_EULA=Y -e MSSQL_PID=Developer \
  -e MSSQL_SA_PASSWORD="$sqlserver_password" \
  -v "$state_dir/sqlserver/mssql.conf:/var/opt/mssql/mssql.conf:ro" \
  -v "$state_dir/sqlserver:/var/opt/mssql/secrets:ro" \
  "$sqlserver_image" >/dev/null

mkdir -p "$state_dir/influx"
docker run --rm --user "$(id -u):$(id -g)" -v "$state_dir/influx:/state" \
  "$influx_image" influxdb3 create token --admin --name trust-openot-release \
  --expiry 1d --offline --output-file /state/admin-token.json >/dev/null
influx_uid=$(docker run --rm --entrypoint sh "$influx_image" -c 'id -u')
influx_gid=$(docker run --rm --entrypoint sh "$influx_image" -c 'id -g')
influx_token=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1], encoding="utf-8"))["token"])' \
  "$state_dir/influx/admin-token.json")
chmod 600 "$state_dir/influx/admin-token.json"
sudo chown "$influx_uid:$influx_gid" "$state_dir/influx/admin-token.json"
docker run -d --name "$prefix-influx" --network "$prefix-network" \
  -v "$state_dir/influx/admin-token.json:/run/secrets/influx-admin-token.json:ro" \
  "$influx_image" influxdb3 serve --node-id openot --object-store memory \
  --admin-token-file /run/secrets/influx-admin-token.json >/dev/null
cat >"$state_dir/nginx/default.conf" <<EOF
server {
  listen 443 ssl;
  client_max_body_size 16m;
  ssl_certificate /tls/server.crt;
  ssl_certificate_key /tls/server.key;
  location / { proxy_pass http://$prefix-influx:8181; }
}
EOF
cp "$state_dir/tls/server.crt" "$state_dir/nginx-tls/server.crt"
cp "$state_dir/tls/server.key" "$state_dir/nginx-tls/server.key"
chmod 644 "$state_dir/nginx-tls/server.crt"
chmod 600 "$state_dir/nginx-tls/server.key"
docker run -d --name "$prefix-influx-tls" --network "$prefix-network" \
  -p 127.0.0.1:58181:443 -v "$state_dir/nginx-tls:/tls:ro" \
  -v "$state_dir/nginx/default.conf:/etc/nginx/conf.d/default.conf:ro" "$nginx_image" >/dev/null

wait_for() {
  local description=$1
  shift
  for _ in $(seq 1 120); do
    if "$@" >/dev/null 2>&1; then return 0; fi
    sleep 1
  done
  echo "timed out waiting for $description" >&2
  docker ps -a >&2
  return 1
}

wait_for PostgreSQL docker exec "$prefix-postgres" pg_isready -U postgres -d openot
wait_for TimescaleDB docker exec "$prefix-timescale" pg_isready -U postgres -d openot
wait_for MySQL docker exec "$prefix-mysql" mysql -h127.0.0.1 -uroot "-p$password" -e 'SELECT 1'
wait_for MariaDB docker exec "$prefix-mariadb" mariadb -h127.0.0.1 -uroot "-p$password" -e 'SELECT 1'
wait_for SQLServer docker exec "$prefix-sqlserver" /opt/mssql-tools18/bin/sqlcmd \
  -S localhost -U sa -P "$sqlserver_password" -C -Q "SELECT 1"
influx_authenticated_health() {
  printf 'header = "Authorization: Bearer %s"\n' "$influx_token" | \
    curl --fail --silent --config - --cacert "$state_dir/tls/ca.pem" \
      https://localhost:58181/health
}
wait_for InfluxDB influx_authenticated_health
printf 'header = "Authorization: Bearer %s"\n' "$influx_token" | \
  curl --fail --silent --show-error --config - --cacert "$state_dir/tls/ca.pem" \
    -H 'Content-Type: text/plain' --data-binary 'runner_auth_check value=1i' \
    'https://localhost:58181/api/v3/write_lp?db=openot'
unauthenticated_status=$(curl --silent --output /dev/null --write-out '%{http_code}' \
  --cacert "$state_dir/tls/ca.pem" -H 'Content-Type: text/plain' \
  --data-binary 'runner_unauthenticated_check value=1i' \
  'https://localhost:58181/api/v3/write_lp?db=openot')
if [[ $unauthenticated_status =~ ^2 ]]; then
  echo "unauthenticated InfluxDB write was accepted" >&2
  exit 1
fi

ca_pem=$(cat "$state_dir/tls/ca.pem")
{
  printf 'OPENOT_DATABASE_RUNNER_STATE_DIR=%s\n' "$state_dir"
  printf 'OPENOT_DATABASE_RUNNER_PREFIX=%s\n' "$prefix"
  printf 'TRUST_TEST_OPENOT_POSTGRES_URL=postgresql://postgres:%s@localhost:55432/openot?sslmode=require\n' "$password"
  printf 'TRUST_TEST_OPENOT_TIMESCALE_URL=postgresql://postgres:%s@localhost:55433/openot?sslmode=require\n' "$password"
  printf 'TRUST_TEST_OPENOT_MYSQL_URL=mysql://root:%s@127.0.0.1:53306/openot\n' "$password"
  printf 'TRUST_TEST_OPENOT_MARIADB_URL=mysql://root:%s@127.0.0.1:53307/openot\n' "$password"
  printf 'TRUST_TEST_OPENOT_SQLSERVER_URL=server=tcp:localhost,51433;user=sa;password=%s;database=master\n' "$sqlserver_password"
  printf 'TRUST_TEST_OPENOT_INFLUX_HOST=https://localhost:58181\n'
  printf 'TRUST_TEST_OPENOT_INFLUX_TOKEN=%s\n' "$influx_token"
  for product in POSTGRES TIMESCALE MYSQL MARIADB SQLSERVER INFLUX; do
    printf 'TRUST_TEST_OPENOT_%s_CA_PEM<<TRUST_OPENOT_CA\n%s\nTRUST_OPENOT_CA\n' "$product" "$ca_pem"
  done
  printf 'TRUST_TEST_OPENOT_POSTGRES_CONTAINER=%s-postgres\n' "$prefix"
  printf 'TRUST_TEST_OPENOT_TIMESCALE_CONTAINER=%s-timescale\n' "$prefix"
  printf 'TRUST_TEST_OPENOT_MYSQL_CONTAINER=%s-mysql\n' "$prefix"
  printf 'TRUST_TEST_OPENOT_MARIADB_CONTAINER=%s-mariadb\n' "$prefix"
  printf 'TRUST_TEST_OPENOT_SQLSERVER_CONTAINER=%s-sqlserver\n' "$prefix"
  printf 'TRUST_TEST_OPENOT_INFLUX_CONTAINER=%s-influx\n' "$prefix"
} >>"$github_env"
