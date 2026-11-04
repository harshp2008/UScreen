#!/usr/bin/env python3
import time
import subprocess
from Xlib import display, X, Xutil

d = display.Display()
screen = d.screen()
root = screen.root

# Create an unmapped or transparent 1x1 input-only / window on DVI-I-2 (e.g. at x=1921, y=200)
# A window on DVI-I-2 that we can map/unmap or expose
window = root.create_window(
    1925, 200, 10, 10, 0,
    screen.root_depth,
    X.InputOutput,
    X.CopyFromParent,
    background_pixel=screen.black_pixel,
    event_mask=X.ExposureMask
)
window.map()
d.sync()

print("Created window on DVI-I-2 at (1925, 200).")
time.sleep(0.5)

# Now move mouse to DVI-I-2
subprocess.check_call(['xdotool', 'mousemove', '2200', '500'])
time.sleep(1.0)

print("Moving mouse across boundary to eDP-1 (x=1800)...")
t0 = time.time()
subprocess.check_call(['xdotool', 'mousemove', '1800', '500'])

# Immediately trigger redraw on DVI-I-2 by clearing window or moving it
window.clear_area(0, 0, 10, 10, True)
d.sync()
t1 = time.time()
print(f"Triggered redraw at t={t1-t0:.4f}s.")

time.sleep(1.0)
window.destroy()
d.sync()
print("Done.")
