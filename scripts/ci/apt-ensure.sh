#!/usr/bin/env bash
# Make Debian packages available on a CI Linux runner without asking for a
# password.
#
# The owned self-hosted Linux runners (cmux-gcp-lane-build-x64 on the GCP
# agent lanes) run jobs as a user with no sudo; the image provides the build
# packages instead (cmuxterm-hq build-fleet/swift-linux-builder/lane-build-runners).
# A bare `sudo apt-get install` there died on 2026-11-18 with "sudo: a password
# is required" in every musl leg of run 44363923089, though the packages could
# have been preinstalled. So: if every package is installed, do nothing. If one
# is missing, install it with `sudo -n` (GitHub-hosted, Blacksmith and the
# ephemeral fleet VMs have passwordless sudo). If sudo would ask for a password,
# fail at once with an annotation naming the runner and the packages to
# provision, never a prompt.
#
# usage: apt-ensure.sh PACKAGE...
set -uo pipefail

if [ "$#" -eq 0 ]; then
  echo "usage: apt-ensure.sh PACKAGE..." >&2
  exit 2
fi

missing=()
for package in "$@"; do
  status="$(dpkg-query -W -f='${Status}' "$package" 2>/dev/null || true)"
  case "$status" in
    *"ok installed") ;;
    *) missing+=("$package") ;;
  esac
done

if [ "${#missing[@]}" -eq 0 ]; then
  echo "apt-ensure: already installed: $*"
  exit 0
fi

if ! sudo -n true 2>/dev/null; then
  echo "::error::[cmux-ci machine: apt-provision] ${RUNNER_NAME:-this runner} lacks ${missing[*]} and its user has no passwordless sudo; preinstall them in the runner image (cmuxterm-hq build-fleet/swift-linux-builder/lane-build-runners/provision.sh)"
  exit 1
fi

echo "apt-ensure: installing ${missing[*]}"
export DEBIAN_FRONTEND=noninteractive
apt=(sudo -n apt-get -o DPkg::Lock::Timeout=300)
"${apt[@]}" update -qq || "${apt[@]}" update -qq || exit 1
"${apt[@]}" install -y -qq --no-install-recommends "${missing[@]}"
