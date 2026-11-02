import sys
import time

start = time.monotonic()
last = start
for line in sys.stdin:
    now = time.monotonic()
    sys.stdout.write(f"{now - start:8.3f} +{now - last:7.3f} {line}")
    sys.stdout.flush()
    last = now
