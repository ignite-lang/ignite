import json
import sys

path = sys.argv[1]
with open(path) as f:
    for line in f:
        try:
            rec = json.loads(line)
        except Exception:
            continue
        msg = rec.get("message")
        if not isinstance(msg, dict):
            continue
        content = msg.get("content")
        if not isinstance(content, list):
            continue
        for block in content:
            if not isinstance(block, dict):
                continue
            if block.get("type") == "tool_use" and block.get("name") == "SendMessage":
                inp = block.get("input", {})
                print("=== SendMessage", rec.get("timestamp"), json.dumps({k: v for k, v in inp.items() if k != "message"}))
                print(inp.get("message", inp))
                print()
