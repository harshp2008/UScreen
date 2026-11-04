#!/usr/bin/env python3
import time
import subprocess
from Xlib import display, X

d = display.Display()
screen = d.screen()
root = screen.root

# Create a 1x1 input-output window on DVI-I-2 (x=1925, y=200)
win = root.create_window(
    1925, 200, 1, 1, 0,
    screen.root_depth,
    X.InputOutput,
    X.CopyFromParent,
    background_pixel=screen.black_pixel,
    event_mask=X.ExposureMask
)
win.map()
d.sync()

print("Window created on DVI-I-2. Moving mouse to tablet (2200, 500)...")
subprocess.check_call(['xdotool', 'mousemove', '2200', '500'])
time.sleep(1.0)

print("Starting measurement...")
t_before = time.time()
# Move to laptop screen
subprocess.check_call(['xdotool', 'mousemove', '1800', '500'])
t_moved = time.time()

# Immediately trigger redraw
win.clear_area(0, 0, 1, 1, True)
d.sync()
t_triggered = time.time()

print(f"Mouse moved at +{(t_moved - t_before)*1000:.2f}ms")
print(f"Redraw triggered at +{(t_triggered - t_before)*1000:.2f}ms")

time.sleep(1.0)
win.destroy()
d.sync()
print("Done measurement.")
