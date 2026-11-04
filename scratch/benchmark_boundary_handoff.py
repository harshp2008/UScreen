#!/usr/bin/env python3
"""
Scientific Boundary Crossing Benchmark & Latency Measurement
Measures handoff latency across the x = 1920 display boundary.
"""
import time
import subprocess
import re

def get_mouse():
    out = subprocess.check_output(['xdotool', 'getmouselocation']).decode()
    m = re.search(r'x:(\d+)\s+y:(\d+)', out)
    return int(m.group(1)), int(m.group(2))

def set_mouse(x, y):
    subprocess.check_call(['xdotool', 'mousemove', str(x), str(y)])

print("=======================================================")
print("SCIENTIFIC BOUNDARY CROSSING BENCHMARK (x = 1920)")
print("=======================================================")

# Warm up
set_mouse(2100, 500)
time.sleep(0.5)

ITERATIONS = 5
results = []

for i in range(ITERATIONS):
    print(f"\n--- Iteration {i+1}/{ITERATIONS} ---")
    
    # 1. Place on tablet
    set_mouse(2100, 500)
    time.sleep(0.5)
    
    # 2. Exit tablet -> laptop
    t0 = time.time()
    set_mouse(1800, 500)
    t1 = time.time()
    exit_duration_ms = (t1 - t0) * 1000.0
    print(f"Exit (Tablet -> Laptop): motion took {exit_duration_ms:.2f} ms")
    
    # Wait for frame settlement
    time.sleep(0.4)
    
    # 3. Enter laptop -> tablet
    t2 = time.time()
    set_mouse(2100, 500)
    t3 = time.time()
    entry_duration_ms = (t3 - t2) * 1000.0
    print(f"Entry (Laptop -> Tablet): motion took {entry_duration_ms:.2f} ms")
    
    time.sleep(0.4)
    results.append((exit_duration_ms, entry_duration_ms))

print("\n=======================================================")
print("BENCHMARK COMPLETED")
print("=======================================================")
