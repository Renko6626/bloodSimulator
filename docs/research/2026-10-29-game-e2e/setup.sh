#!/usr/bin/env bash
set -euo pipefail
mkdir -p /workspace/blood-game-e2e
cd /workspace/blood-game-e2e
export DEBIAN_FRONTEND=noninteractive
apt-get update > apt-update.log 2>&1
apt-get install -y --no-install-recommends xserver-xorg-core xauth x11-xserver-utils mesa-utils libgl1 libegl1 libopengl0 libvulkan1 libx11-6 libxcursor1 libxinerama1 libxi6 libxrandr2 libasound2 libpulse0 libfontconfig1 libudev1 libdbus-1-3 fonts-noto-cjk curl unzip ca-certificates > apt-install.log 2>&1
tar -xzf /workspace/payload.tar.gz
curl -fL --connect-timeout 20 --max-time 180 --retry 2 https://github.com/godotengine/godot-builds/releases/download/4.7.1-stable/Godot_v4.7.1-stable_linux.x86_64.zip -o godot.zip > download-godot.log 2>&1
unzip -o godot.zip > unzip.log
mv Godot_v4.7.1-stable_linux.x86_64 godot-bin
chmod +x godot-bin
python3 - <<'PY'
import hashlib,json
m=json.load(open('manifest.json'))
assert hashlib.sha256(open('godot-bin','rb').read()).hexdigest()==m['godot_sha256'],'Godot binary mismatch'
PY
mkdir -p display/modules/drivers display/modules/extensions
# Container graphics libraries exist, but Xorg's NVIDIA modules are not mounted.
# Extract the exact host driver version into this experiment; never install kernel modules.
driver_version=$(nvidia-smi --query-gpu=driver_version --format=csv,noheader | head -1)
curl -fL --connect-timeout 20 --max-time 240 --retry 2 "https://download.nvidia.com/XFree86/Linux-x86_64/$driver_version/NVIDIA-Linux-x86_64-$driver_version.run" -o nvidia.run > download-nvidia.log 2>&1
sh nvidia.run --extract-only --target "$PWD/display/driver" > extract-nvidia.log 2>&1
ln -sf "$PWD/display/driver/nvidia_drv.so" display/modules/drivers/nvidia_drv.so
ln -sf "$PWD/display/driver/libglxserver_nvidia.so.$driver_version" display/modules/extensions/libglxserver_nvidia.so
python3 - <<'PY'
import subprocess
bus=subprocess.check_output(['nvidia-smi','--query-gpu=pci.bus_id','--format=csv,noheader'],text=True).strip()
a=bus.split(':');b=int(a[-2],16);d,f=a[-1].split('.')
text='''Section "ServerFlags"
 Option "AutoAddDevices" "False"
 Option "AutoAddGPU" "False"
EndSection
Section "Device"
 Identifier "ExperimentGPU"
 Driver "nvidia"
 BusID "PCI:%d:%d:%d"
 Option "AllowEmptyInitialConfiguration" "True"
 Option "UseDisplayDevice" "None"
EndSection
Section "Screen"
 Identifier "ExperimentScreen"
 Device "ExperimentGPU"
 DefaultDepth 24
 SubSection "Display"
  Depth 24
  Virtual 1280 800
 EndSubSection
EndSection
Section "ServerLayout"
 Identifier "ExperimentLayout"
 Screen "ExperimentScreen"
EndSection
'''%(b,int(d,16),int(f,16))
open('display/xorg.conf','w').write(text)
PY
mkdir -p display/glvnd-debs display/glvnd
(
 cd display/glvnd-debs
 apt-get download libgl1 libglx0 libglvnd0 libegl1 libopengl0 > download.log 2>&1
 for package in *.deb; do dpkg-deb -x "$package" ../glvnd; done
)
export LD_LIBRARY_PATH="$PWD/display/glvnd/usr/lib/x86_64-linux-gnu:$PWD/display/driver:${LD_LIBRARY_PATH:-}"
nohup Xorg :99 -config "$PWD/display/xorg.conf" -modulepath "$PWD/display/modules,/usr/lib/xorg/modules" -logfile "$PWD/display/Xorg.log" -noreset -nolisten tcp -novtswitch -sharevts > display/xorg-stdout.log 2>&1 < /dev/null &
echo $! > display/xorg.pid
export DISPLAY=:99
for attempt in $(seq 1 20); do
    if glxinfo -B > display/glxinfo.txt 2>&1; then break; fi
    sleep 1
done
python3 - <<'PY'
s=open('display/glxinfo.txt').read()
assert 'NVIDIA' in s and 'llvmpipe' not in s and 'direct rendering: Yes' in s,s
PY
export XDG_RUNTIME_DIR=/tmp/blood-e2e-runtime
mkdir -p "$XDG_RUNTIME_DIR"
chmod 700 "$XDG_RUNTIME_DIR"
for project in baseline current world; do
    # Preserve the known first empty-cache import failure, retry cached import once.
    set +e
    ./godot-bin --headless --path "$project/godot" --editor --import --quit > "$project.import-first.log" 2>&1
    code=$?
    set -e
    printf '%s\n' "$code" > "$project.import-first.exit.txt"
    if [[ "$code" != 0 ]]; then
        ./godot-bin --headless --path "$project/godot" --editor --import --quit > "$project.import-cached.log" 2>&1
    fi
done
printf 'SETUP_COMPLETE\n'
