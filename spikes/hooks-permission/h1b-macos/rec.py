"""usage: rec.py <script -r file> [pattern ...]
Prints a timeline from a macOS `script -r` recording: every input record (keys),
every OSC 0 title, and every output record holding a pattern."""
import re, struct, sys

HDR = struct.Struct("<QQII")  # len, sec, usec, direction
TITLE = re.compile(rb"\x1b\]0;(.*?)\x07", re.S)

data = open(sys.argv[1], "rb").read()
patterns = [p.encode() for p in sys.argv[2:]]
pos, last_title = 0, None
while pos + HDR.size <= len(data):
    n, sec, usec, d = HDR.unpack_from(data, pos)
    body = data[pos + HDR.size : pos + HDR.size + n]
    pos += HDR.size + n
    t = f"{sec}.{usec // 1000:03d}"
    if d == ord("i"):
        print(t, "IN ", repr(body))
    elif d == ord("o"):
        for m in TITLE.finditer(body):
            title = m.group(1).decode(errors="replace")
            if title != last_title:
                print(t, "TITLE", title)
                last_title = title
        for p in patterns:
            if p in body:
                print(t, "OUT", p.decode())
