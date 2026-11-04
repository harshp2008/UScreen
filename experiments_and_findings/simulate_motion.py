#!/usr/bin/env python3
import time
import subprocess
import sys
import math

duration = int(sys.argv[1]) if len(sys.argv) > 1 else 30
start_time = time.time()
center_x = 2600
center_y = 450
radius = 350
freq = 1.0 # 1 cycle per second

print(f"Generating 60 FPS motion on virtual display for {duration} seconds...")
frame_period = 1.0 / 60.0

while time.time() - start_time < duration:
    t = time.time() - start_time
    x = int(center_x + radius * math.cos(2 * math.pi * freq * t))
    y = int(center_y + radius * math.sin(2 * math.pi * freq * t))
    subprocess.run(["xdotool", "mousemove", str(x), str(y)], check=False)
    time.sleep(frame_period)

print("Motion generation complete.")
