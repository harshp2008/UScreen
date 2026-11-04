#!/usr/bin/env python3
import time
import subprocess
import re

def get_mouse():
    out = subprocess.check_output(['xdotool', 'getmouselocation']).decode()
    m = re.search(r'x:(\d+)\s+y:(\d+)', out)
    return int(m.group(1)), int(m.group(2))

def set_mouse(x, y):
    subprocess.check_call(['xdotool', 'mousemove', str(x), str(y)])

print("Current location:", get_mouse())

# Step 1: Place on DVI-I-2 (Tablet)
print("Moving to DVI-I-2 (x=2100, y=500)...")
set_mouse(2100, 500)
time.sleep(1.0)

# Step 2: Cross boundary to eDP-1 (Laptop)
print("Crossing boundary to eDP-1 (x=1910, y=500)...")
t0 = time.time()
set_mouse(1910, 500)
t1 = time.time()
print(f"Mouse moved across boundary at t={t1-t0:.4f}s")
time.sleep(0.5)

# Step 3: Cross back to DVI-I-2
print("Crossing back to DVI-I-2 (x=1930, y=500)...")
t2 = time.time()
set_mouse(1930, 500)
t3 = time.time()
print(f"Mouse moved back across boundary at t={t3-t2:.4f}s")
time.sleep(0.5)

print("Done probing.")
