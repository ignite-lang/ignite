import socket
import subprocess
import sys

listener = socket.socket()
listener.bind(("127.0.0.1", 0))
listener.listen(1)
port = listener.getsockname()[1]
child = subprocess.Popen([sys.argv[1], "--port", str(port)], stdin=subprocess.DEVNULL,
                         stdout=subprocess.PIPE, stderr=subprocess.PIPE)
conn, _ = listener.accept()
try:
    out, err = child.communicate(timeout=20)
except subprocess.TimeoutExpired:
    child.kill()
    out, err = child.communicate()
    print("TIMEOUT")
conn.close()
sys.stdout.write(out.decode("utf-8", "replace"))
sys.stdout.write("stderr: " + err.decode("utf-8", "replace") + "\n")
sys.stdout.write("exit: %s\n" % child.returncode)
