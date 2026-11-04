#!/usr/bin/env python3
import time
import subprocess
import evdev
from evdev import ecodes

def get_mouse():
    out = subprocess.check_output(['xdotool', 'getmouselocation']).decode()
    import re
    m = re.search(r'x:(\d+)\s+y:(\d+)', out)
    return int(m.group(1)), int(m.group(2))

def set_mouse(x, y):
    subprocess.check_call(['xdotool', 'mousemove', str(x), str(y)])

print("Initial mouse pos:", get_mouse())

# Step 1: Place mouse on DVI-I-2 (x=2200, y=500)
print("Placing mouse on DVI-I-2 (x=2200, y=500)...")
set_mouse(2200, 500)
time.sleep(1.0)
print("Confirmed pos:", get_mouse())

# Step 2: Wiggle on DVI-I-2 using uinput
cap = {
    ecodes.EV_REL: [ecodes.REL_X, ecodes.REL_Y],
    ecodes.EV_KEY: [ecodes.BTN_LEFT, ecodes.BTN_RIGHT]
}
ui = evdev.UInput(cap, name="Boundary Test Mouse")
time.sleep(0.5)

print("Wiggling mouse on DVI-I-2 for 2 seconds...")
t_end = time.time() + 2.0
while time.time() < t_end:
    ui.write(ecodes.EV_REL, ecodes.REL_X, 5)
    ui.syn()
    time.sleep(0.016)
    ui.write(ecodes.EV_REL, ecodes.REL_X, -5)
    ui.syn()
    time.sleep(0.016)

print("Pos before crossing:", get_mouse())

# Step 3: Cross boundary into eDP-1 (x=1850, y=500)
print("CROSSING BOUNDARY TO eDP-1 (x=1850)...")
t0 = time.time()
set_mouse(1850, 500)
t1 = time.time()
print(f"Boundary crossed at t={t1-t0:.4f}s. Current pos: {get_mouse()}")

# Sleep 1s on eDP-1
time.sleep(1.0)

# Step 4: Cross back into DVI-I-2 (x=2000, y=500)
print("CROSSING BACK TO DVI-I-2 (x=2000)...")
t2 = time.time()
set_mouse(2000, 500)
t3 = time.time()
print(f"Boundary crossed back at t={t3-t2:.4f}s. Current pos: {get_mouse()}")

time.sleep(1.0)
ui.close()
print("Test completed.")
