#!/bin/bash
set -euxo pipefail
shutdown -h +@MAX_MINUTES@ "ignite quality: max hours reached"
mkdir -p /var/lib/ignite-quality
export DEBIAN_FRONTEND=noninteractive
for unit in unattended-upgrades.service apt-daily.timer apt-daily-upgrade.timer apt-daily.service apt-daily-upgrade.service \
  fwupd-refresh.timer motd-news.timer man-db.timer e2scrub_all.timer fstrim.timer logrotate.timer dpkg-db-backup.timer \
  update-notifier-download.timer update-notifier-motd.timer sysstat.service sysstat-collect.timer sysstat-summary.timer \
  snap.amazon-ssm-agent.amazon-ssm-agent.service snapd.service snapd.socket snapd.seeded.service \
  packagekit.service ModemManager.service udisks2.service multipathd.service multipathd.socket; do
  systemctl disable --now "$unit" || true
done
while fuser /var/lib/dpkg/lock-frontend >/dev/null 2>&1; do sleep 2; done
apt-get update -q
apt-get install -y -q --no-install-recommends build-essential pkg-config curl ca-certificates gnupg lsb-release wget \
  software-properties-common psmisc golang-go
curl -sSfL -o /var/lib/ignite-quality/llvm.sh https://apt.llvm.org/llvm.sh
installed=
for major in 23 22 21 20; do
  if bash /var/lib/ignite-quality/llvm.sh "$major"; then installed=$major; break; fi
done
test -n "$installed"
apt-get install -y -q --no-install-recommends "lld-$installed"
test -x "/usr/lib/llvm-$installed/bin/ld.lld"
ln -sf "/usr/bin/clang-$installed" /usr/local/bin/clang
sudo -u ubuntu -H sh -c 'curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain stable --component rust-src'
echo off > /sys/devices/system/cpu/smt/control || true
if [ -e /sys/devices/system/cpu/intel_pstate/no_turbo ]; then echo 1 > /sys/devices/system/cpu/intel_pstate/no_turbo || true; fi
if [ -e /sys/devices/system/cpu/cpufreq/boost ]; then echo 0 > /sys/devices/system/cpu/cpufreq/boost || true; fi
for governor in /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor; do
  [ -e "$governor" ] && { echo performance > "$governor" || true; }
done
touch /var/lib/ignite-quality/ready
