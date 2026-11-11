#!/usr/bin/env bash
# Installs Ubuntu packages on a CI runner: `ci_apt_install.sh PACKAGE...`.
#
# On 11 November 2026 GitHub's hosted Ubuntu runners could not resolve the Ubuntu archives for hours
# ("Temporary failure resolving 'security.ubuntu.com'") while public resolvers answered, and every job
# that installs a package failed (checklist G144). So before apt runs, the runner's resolver is asked
# for the archives' hosts; when it cannot resolve one, public resolvers are added beside the runner's
# own, for the rest of the job, which also serves a later step that installs through apt itself
# (Playwright's `--with-deps`). apt retries each download, and the whole install is tried three times.
set -euo pipefail

if [ "$#" -eq 0 ]; then
  echo "usage: ci_apt_install.sh PACKAGE..." >&2
  exit 2
fi

archive_hosts=(archive.ubuntu.com security.ubuntu.com azure.archive.ubuntu.com)
public_resolvers=(1.1.1.1 8.8.8.8 9.9.9.9)

archives_resolve() {
  local host
  for host in "${archive_hosts[@]}"; do
    if ! getent hosts "$host" >/dev/null; then
      echo "ci_apt_install: the runner cannot resolve $host" >&2
      return 1
    fi
  done
}

add_public_resolvers() {
  if command -v resolvectl >/dev/null 2>&1 && systemctl is-active --quiet systemd-resolved; then
    # systemd-resolved asks its global servers in parallel with the runner's own (resolved.conf DNS=).
    sudo mkdir -p /etc/systemd/resolved.conf.d
    printf '[Resolve]\nDNS=%s\n' "${public_resolvers[*]}" |
      sudo tee /etc/systemd/resolved.conf.d/ci-public-resolvers.conf >/dev/null
    sudo systemctl restart systemd-resolved
  else
    local entries=""
    local resolver
    for resolver in "${public_resolvers[@]}"; do
      entries+="nameserver ${resolver}"$'\n'
    done
    { printf '%s' "$entries"; cat /etc/resolv.conf; } | sudo tee /etc/resolv.conf.ci >/dev/null
    sudo mv /etc/resolv.conf.ci /etc/resolv.conf
  fi
  echo "ci_apt_install: added the public resolvers ${public_resolvers[*]}" >&2
}

if ! archives_resolve; then
  add_public_resolvers
  archives_resolve || echo "ci_apt_install: the archives still do not resolve; trying apt anyway" >&2
fi

for attempt in 1 2 3; do
  if sudo apt-get -o Acquire::Retries=3 update &&
    sudo apt-get -o Acquire::Retries=3 install -y "$@"; then
    exit 0
  fi
  echo "ci_apt_install: attempt ${attempt} of 3 failed" >&2
  if [ "$attempt" -lt 3 ]; then
    sleep $((attempt * 15))
  fi
done
exit 1
