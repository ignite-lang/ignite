from datetime import datetime, timezone, timedelta
from zoneinfo import ZoneInfo

B = ZoneInfo("Europe/Berlin")
U = timezone.utc


def s(dt):
    return int(dt.timestamp())


def show(label, dt):
    print(f"{label}: {dt.isoformat()} = {s(dt)} s = {s(dt) * 1_000_000_000} ns")


for y, m in [(2026, 10), (2026, 11), (2026, 3), (2026, 4), (2026, 1), (2027, 1), (2025, 12), (2024, 2), (2024, 3)]:
    local = datetime(y, m, 1, tzinfo=B)
    show(f"Berlin {y}-{m:02d}-01 00:00 local -> UTC", local.astimezone(U))
    show(f"UTC    {y}-{m:02d}-01 00:00", datetime(y, m, 1, tzinfo=U))

show("probe epoch 2026-10-31T22:50:00Z", datetime(2026, 10, 31, 22, 50, tzinfo=U))
show("2026-01-01T00:00:00Z", datetime(2026, 1, 1, tzinfo=U))
show("DST start 2026 (UTC)", datetime(2026, 3, 29, 1, 0, tzinfo=U))
show("DST end 2026 (UTC)", datetime(2026, 10, 25, 1, 0, tzinfo=U))
print("1970-01-01 weekday (Mon=0):", datetime(1970, 1, 1).weekday())
print("days 2026-10-31:", (datetime(2026, 10, 31) - datetime(1970, 1, 1)).days)
print("days 2000-03-01:", (datetime(2000, 3, 1) - datetime(1970, 1, 1)).days)
print("days 2024-02-29:", (datetime(2024, 2, 29) - datetime(1970, 1, 1)).days)
