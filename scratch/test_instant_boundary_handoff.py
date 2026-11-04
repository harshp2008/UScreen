#!/usr/bin/env python3
import time
import subprocess
from Xlib import display, X

d = display.Display()
screen = d.screen()
root = screen.root

# Create a 1x1 window positioned on DVI-I-2 (e.g. at x=1921, y=166)
# Completely invisible / black, border_width=0
win = root.create_window(
    1921, 166, 1, 1, 0,
    screen.root_depth,
    X.InputOutput,
    X.CopyFromParent,
    background_pixel=screen.black_pixel
)
win.map()
d.sync()
time.sleep(0.5)

print("Setup complete. Moving mouse to tablet (x=2100, y=500)...")
subprocess.check_call(['xdotool', 'mousemove', '2100', '500'])
time.sleep(1.0)

toggle = 1
def nudge():
    global toggle
    toggle = 2 if toggle == 1 else 1
    win.configure(width=toggle, height=1)
    d.sync()

print("Testing boundary crossing from DVI-I-2 to eDP-1...")
t0 = time.time()
subprocess.check_call(['xdotool', 'mousemove', '1850', '500'])
t_move = time.time()

# Immediately nudge DVI-I-2
nudge()
t_nudge = time.time()

print(f"Move took {(t_move - t0)*1000:.2f} ms")
print(f"Nudge executed at +{(t_nudge - t_move)*1000:.2f} ms after move")

time.sleep(1.0)

print("Testing crossing back from eDP-1 to DVI-I-2...")
t2 = time.time()
# Nudge DVI-I-2 to wake frame clock
nudge()
subprocess.check_call(['xdotool', 'mousemove', '2100', '500'])
t3 = time.time()
print(f"Entry back to DVI-I-2 took {(t3 - t2)*1000:.2f} ms")

time.sleep(1.0)
win.destroy()
d.sync()
print("Test completed successfully.")
