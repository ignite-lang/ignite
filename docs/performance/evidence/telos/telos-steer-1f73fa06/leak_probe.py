import os, socket, subprocess, sys, threading, time

exe = sys.argv[1]
p = subprocess.Popen([exe, "--port", "0"], stdin=subprocess.DEVNULL,
                     stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
first = p.stderr.readline().decode()
port = int(first.strip().rsplit(":", 1)[1])
logged = []


def drain():
    for line in p.stderr:
        logged.append(line)


threading.Thread(target=drain, daemon=True).start()


def session(line):
    s = socket.create_connection(("127.0.0.1", port))
    s.settimeout(10)
    s.sendall(line)
    data = b""
    while True:
        chunk = s.recv(4096)
        if not chunk:
            break
        data += chunk
    s.close()
    return data


def fds():
    return len(os.listdir("/proc/%d/fd" % p.pid))


def rss_kib():
    with open("/proc/%d/status" % p.pid) as f:
        for line in f:
            if line.startswith("VmRSS:"):
                return int(line.split()[1])


assert session(b"index\n") == b""
assert session(b"warm\n") == b"ok warm\n"
f0, r0 = fds(), rss_kib()
t = time.time()
for _ in range(10000):
    assert session(b"index\n") == b""
elapsed = time.time() - t
assert session(b"still\n") == b"ok still\n"
deadline = time.time() + 10
while fds() > f0 and time.time() < deadline:
    time.sleep(0.01)
time.sleep(0.2)
f1, r1 = fds(), rss_kib()
print("sessions=10000 elapsed=%.2fs" % elapsed)
print("fds before=%d after=%d" % (f0, f1))
print("VmRSS before=%d KiB after=%d KiB delta=%d KiB" % (r0, r1, r1 - r0))
p.kill()
p.wait()
print("task-failure lines logged=%d" % sum(1 for l in logged if l.startswith(b"task ended with error: index failed")))
