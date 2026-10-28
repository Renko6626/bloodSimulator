# 本轮离屏显示环境

实例58133143，公网125.240.239.50，容器SSH22映射30782；已在启动/停机状态JSON核验。只有本实例参与测量，没有暴露新的远程桌面服务。Xorg只监听本地Unix socket。

容器内实际准备步骤：

```bash
apt-get update -qq
DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends xserver-xorg-core xvfb mesa-utils libxcursor1 libxi6 libxrandr2 libxinerama1 libasound2t64
install-display-drivers
```

`install-display-drivers` 是本Vast镜像提供的工具；本轮先阅读实现，再仅提取与宿主580.173.02匹配的显示用户态库到 `/opt/nvidia-drivers`。未运行NVIDIA内核驱动安装器，未改宿主模块、GPU频率或功耗。实际使用NVIDIA Xorg，安装的Xvfb未用于测量。

```bash
mkdir -p /workspace/lbm-gameplay-20261029/display/modules/drivers /workspace/lbm-gameplay-20261029/display/modules/extensions
ln -sf /opt/nvidia-drivers/lib64/nvidia_drv.so /workspace/lbm-gameplay-20261029/display/modules/drivers/nvidia_drv.so
ln -sf /opt/nvidia-drivers/lib64/libglxserver_nvidia.so.580.173.02 /workspace/lbm-gameplay-20261029/display/modules/extensions/libglxserver_nvidia.so
Xorg :99 -noreset -novtswitch -sharevts -nolisten tcp -config /workspace/lbm-gameplay-20261029/display/xorg.conf -modulepath /workspace/lbm-gameplay-20261029/display/modules,/usr/lib/xorg/modules -logfile /workspace/lbm-gameplay-20261029/display/Xorg.99.log
DISPLAY=:99 glxinfo -B
```

Xorg配置见 [xorg.conf](xorg.conf)，`UseDisplayDevice=None`，virtual1280×800，GPU BusID由nvidia-smi确认PCI:3:0:0。缺少物理键鼠输入驱动的Xorg警告没有影响本轮MoveInput回放；未用鼠标/键盘自动操作UI。

第一次截图发现最小容器缺少中文字体。在Rust正式probe结束、Godot正式测量开始前安装 `fonts-noto-cjk`；正式Godot与全部性能控制采用相同字体环境，首个缺字smoke截图保留，报告显示后续正确字体截图。没有在计时期间安装软件。

Godot来自本机已有 `Godot_v4.7.1-stable_linux.x86_64`，复制到 `/workspace/godot-4.7.1`；实际哈希见 [runtime-binary.sha256](results/runtime-binary.sha256)。所有Godot测量均使用Dummy音频、固定60Hz模拟步；正常绘制与no-draw控制均使用原gl_compatibility和NVIDIA GPU。headless控制只代表无画面控制。

安装和Xorg日志保留在 [display/](display/)。本轮结束后整个容器已stop，不持续运行显示服务。
