"""Item 5 and 6 checks on a model: python3 -I accept_model.py <model.json>

Walks every value and checks it against the encoding table of docs/slice.md 15.4 (tagged
objects carry the keys the table names; no other object shape appears below a property), and
counts the members of assembly.resolution.
"""
import json
import sys

TAGS = {
    "Byte": {"type", "value"},
    "Tuple": {"type", "items"},
    "Map": {"type", "entries"},
    "Duration": {"type", "ns"},
    "ByteSize": {"type", "bytes"},
    "Ratio": {"type", "ppm"},
    "Ref": {"type", "ref"},
    "Signal": {"type", "ref"},
    "SystemRef": {"type", "ref"},
    "Protocol": {"type", "ref"},
    "Module": {"type", "module", "fns"},
    "Fn": {"type", "fn"},
}
problems = []
values = 0


def walk(value, path):
    global values
    values += 1
    if value is None or isinstance(value, (bool, int, str)):
        return
    if isinstance(value, float):
        problems.append(f"{path}: float {value}")
        return
    if isinstance(value, list):
        for i, item in enumerate(value):
            walk(item, f"{path}[{i}]")
        return
    tag = value.get("type")
    if tag is None:
        problems.append(f"{path}: object without type: {sorted(value)}")
        return
    if tag in TAGS:
        if set(value) != TAGS[tag]:
            problems.append(f"{path}: {tag} has keys {sorted(value)}")
        for k in ("items", "entries", "value"):
            if k in value:
                walk(value[k], f"{path}.{k}")
        return
    if "variant" in value:
        if not set(value) <= {"type", "variant", "fields"}:
            problems.append(f"{path}: enum has keys {sorted(value)}")
        for i, item in enumerate(value.get("fields", [])):
            walk(item, f"{path}.fields[{i}]")
        return
    if "fields" in value and set(value) == {"type", "fields"}:
        for k, v in value["fields"].items():
            walk(v, f"{path}.{k}")
        return
    problems.append(f"{path}: unknown object shape {sorted(value)}")


model = json.load(open(sys.argv[1]))
members = []
for key in ("resources", "components", "contracts", "policies"):
    for member in model.get(key, []):
        members.append(member["id"])
        for name, v in member["properties"].items():
            walk(v, f"{member['id']}.{name}")
assembly = model.get("assembly")
if assembly:
    for name, v in assembly["properties"].items():
        walk(v, f"{assembly['id']}.{name}")
    counts = {}
    for entry in assembly.get("resolution") or []:
        counts[entry["member"]] = counts.get(entry["member"], 0) + 1
        walk(entry["binding"], f"resolution[{entry['member']}].binding")
    print(f"resolution entries: {len(assembly.get('resolution') or [])}; per member: {counts}")
    bound = set(counts)
    hosts = {m for m in members if not any(m == c["id"] for k in ("contracts", "policies") for c in model.get(k, []))}
    print(f"resources and components: {sorted(hosts)}")
    print(f"each exactly once: {all(counts.get(m) == 1 for m in hosts) and bound == hosts}")
else:
    print("assembly: null")
print(f"values walked: {values}; encoding problems: {len(problems)}")
for p in problems[:20]:
    print("  " + p)
print(f"keys: {sorted(model)}; formatVersion: {model.get('formatVersion')}")
