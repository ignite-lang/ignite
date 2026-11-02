import socket, subprocess, sys
s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
s.bind(("127.0.0.1", 0))
s.listen(16)
port = s.getsockname()[1]
crash = sys.argv[1]
p = subprocess.Popen([crash, "--port", str(port)],
                     stdin=subprocess.DEVNULL,
                     stdout=subprocess.PIPE, stderr=subprocess.PIPE)
out, err = p.communicate(timeout=10)
print("EXITCODE", p.returncode)
print("STDOUT", repr(out))
print("STDERR", repr(err))
