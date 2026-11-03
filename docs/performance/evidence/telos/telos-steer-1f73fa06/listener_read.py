import socket, subprocess, sys
s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
s.bind(("127.0.0.1", 0))
s.listen(16)
port = s.getsockname()[1]
exe = sys.argv[1]
p = subprocess.Popen([exe, "--port", str(port)],
                     stdin=subprocess.DEVNULL,
                     stdout=subprocess.PIPE, stderr=subprocess.PIPE)
conn, _ = s.accept()
conn.settimeout(10)
data = b""
try:
    while True:
        chunk = conn.recv(4096)
        if not chunk:
            break
        data += chunk
except socket.timeout:
    pass
conn.close()
try:
    out, err = p.communicate(timeout=10)
except subprocess.TimeoutExpired:
    p.kill()
    out, err = p.communicate()
print("CLIENT_WROTE", repr(data))
print("EXITCODE", p.returncode)
print("STDERR", repr(err))
