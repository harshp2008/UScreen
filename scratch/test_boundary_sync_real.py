#!/usr/bin/env python3
import time
import subprocess

def get_mouse():
    out = subprocess.check_output(['xdotool', 'getmouselocation']).decode()
    import re
    m = re.search(r'x:(\d+)\s+y:(\d+)', out)
    return int(m.group(1)), int(m.group(2))

def set_mouse(x, y):
    subprocess.check_call(['xdotool', 'mousemove', str(x), str(y)])

print("1. Placing mouse on Tablet (2100, 500)...")
set_mouse(2100, 500)
time.sleep(1.0)
print("Confirmed at:", get_mouse())

print("2. Crossing from Tablet -> Laptop (1800, 500)...")
t0 = time.time()
set_mouse(1800, 500)
t1 = time.time()
print(f"Crossed to laptop in {(t1-t0)*1000:.2f}ms. Current: {get_mouse()}")

time.sleep(1.0)

print("3. Crossing from Laptop -> Tablet (2100, 500)...")
t2 = time.time()
set_mouse(2100, 500)
t3 = time.time()
print(f"Crossed to tablet in {(t3-t2)*1000:.2f}ms. Current: {get_mouse()}")

time.sleep(1.0)
print("Test complete.")
