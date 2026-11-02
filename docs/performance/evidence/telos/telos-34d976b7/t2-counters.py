import socket, subprocess, sys, threading

exe = sys.argv[1]
p = subprocess.Popen([exe, "--port", "0"], stdin=subprocess.DEVNULL, stderr=subprocess.PIPE)
try:
    line = p.stderr.readline().decode()
    port = int(line.strip().rsplit(":", 1)[1])

    def conn():
        s = socket.create_connection(("127.0.0.1", port))
        s.settimeout(10)
        return s, s.makefile("rb")

    def ask(c, text):
        s, f = c
        s.sendall((text + "\n").encode())
        return f.readline().decode().rstrip("\n")

    c = conn()
    for q in ["generic", "new", "new", "inc 1", "inc 1", "inc 9", "inc +1", "get 1", "all",
              "del 2", "del 2", "all", "byte 255", "byte 256", "byte -1",
              "byte 9223372036854775808"]:
        print(q, "->", ask(c, q))
    ids = []
    lock = threading.Lock()

    def worker():
        k = conn()
        mine = [ask(k, "new") for _ in range(100) if ask(k, "inc 1") or True]
        with lock:
            ids.extend(mine)

    ts = [threading.Thread(target=worker) for _ in range(16)]
    for t in ts:
        t.start()
    for t in ts:
        t.join()
    print("distinct", len(set(ids)), "count", ask(c, "get 1"))
finally:
    p.kill()
    p.wait()
