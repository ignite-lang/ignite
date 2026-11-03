import socket, subprocess, sys, time

exe = sys.argv[1]
p = subprocess.Popen([exe, "--port", "0"], stdin=subprocess.DEVNULL, stderr=subprocess.PIPE)
try:
    port = int(p.stderr.readline().decode().strip().rsplit(":", 1)[1])
    s = socket.create_connection(("127.0.0.1", port))
    s.settimeout(15)
    start = time.time()
    data = s.recv(100)
    print("closed after", round(time.time() - start, 1), "s, bytes", data)
finally:
    p.kill()
    p.wait()
