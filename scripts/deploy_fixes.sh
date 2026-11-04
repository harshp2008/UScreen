#!/bin/bash
set -euo pipefail

echo "== Applying UScreen System Configurations =="

# 1. Modprobe configuration
mkdir -p /etc/modprobe.d
cat << 'EOF' > /etc/modprobe.d/uscreen-evdi.conf
options evdi initial_device_count=2
softdep evdi pre: drm_kms_helper i915
EOF

cat << 'EOF' > /etc/modprobe.d/evdi.conf
options evdi initial_device_count=2
softdep evdi pre: drm_kms_helper i915
EOF

# 2. Udev rule for EVDI seat access
mkdir -p /etc/udev/rules.d
cat << 'EOF' > /etc/udev/rules.d/99-uscreen-evdi.rules
ACTION=="add", SUBSYSTEM=="drm", KERNEL=="card[0-9]*", DRIVERS=="evdi", TAG+="uaccess"
EOF

# 3. Reload udev
udevadm control --reload || true
udevadm trigger --subsystem-match=drm || true

# 4. Install binaries to system paths
mkdir -p /usr/lib/uscreen
cp -f "/home/harsh/Documents/GITHUB PROJECTS/UScreen/target/release/uscreen" /usr/bin/uscreen
cp -f "/home/harsh/Documents/GITHUB PROJECTS/UScreen/target/release/uscreen-gui" /usr/bin/uscreen-gui
cp -f "/home/harsh/Documents/GITHUB PROJECTS/UScreen/target/release/evdi_helper" /usr/lib/uscreen/evdi_helper

echo "== System fixes deployed successfully =="
