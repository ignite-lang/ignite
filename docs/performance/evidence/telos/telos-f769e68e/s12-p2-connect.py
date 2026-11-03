import socket
import sys

port = int(sys.argv[1])
s = socket.create_connection(("127.0.0.1", port), timeout=3)
s.settimeout(3)
try:
    data = s.recv(64)
    print("first connection received:", data)
except Exception as error:
    print("first connection:", error)
