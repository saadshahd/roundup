"""U142 real-pointer observer. Serve `just harness tree-40 5143`, then run:
python3 apps/desktop/src/rail/u142_browser.py http://localhost:5143 /tmp/u142-before
Requires agent-browser; exits nonzero if any U142 observation fails.
"""
import json
import os
from pathlib import Path
import subprocess
import sys

URL, OUTPUT = sys.argv[1:]
OUT = Path(OUTPUT)
OUT.mkdir(parents=True, exist_ok=True)


def browser(*args):
    result = subprocess.run(
        ["agent-browser", "--session", "u142-fix", "--json", *map(str, args)],
        check=True, capture_output=True, text=True, timeout=60,
        env={**os.environ, "AGENT_BROWSER_SESSION": "u142-fix"},
    )
    reply = json.loads(result.stdout)
    if not reply["success"]:
        raise RuntimeError(reply)
    return reply["data"]


def evaluate(code):
    return browser("eval", code)["result"]


def reset():
    evaluate("localStorage.clear()")
    browser("open", "about:blank")
    browser("open", URL + "/harness.html?seed=tree-40")
    browser("wait", '[data-id="migrate"]')
    evaluate("getSelection().removeAllRanges()")


def rect(selector):
    return evaluate(f"document.querySelector({json.dumps(selector)}).getBoundingClientRect().toJSON()")


def drag(start, end, release=True):
    browser("mouse", "move", *start)
    browser("mouse", "down")
    for step in range(1, 9):
        browser("mouse", "move", *[round(a + (b-a)*step/8) for a, b in zip(start, end)])
    if release:
        browser("mouse", "up")


def point(box, right=False):
    return [round(box["right"] - 2 if right else box["left"] + 2), round(box["top"] + box["height"]/2)]


browser("open", URL + "/harness.html?seed=tree-40")
browser("wait", '[data-id="migrate"]')

observations = []
failures = []
selectors = {
    "group": '[data-id="backend"] .name',
    "agent": '[data-id="migrate"] .name',
    "terminal": '[data-id="shell"] .name',
    "done": '.rail-fold button',
    "add-agent": '.rail-actions button:nth-child(1)',
    "add-terminal": '.rail-actions button:nth-child(2)',
    "add-group": '.rail-actions button:nth-child(3)',
}
for width in (1280, 700):
    browser("set", "viewport", width, 800)
    for target in (*selectors, "done-gap", "add-gap", "adjacent-rows"):
        for reverse in (False, True):
            reset()
            styles = evaluate("""Object.fromEntries(['.rail-row','.rail-row .name',
                '.rail-fold','.rail-fold button','.rail-actions',
                ...[1,2,3].map(n=>`.rail-actions button:nth-child(${n})`)]
                .map(s=>[s,getComputedStyle(document.querySelector(s)).userSelect]))""")
            if target == "done-gap":
                fold, button = rect('.rail-fold'), rect('.rail-fold button')
                start, end = point(fold), point(button, True)
            elif target == "add-gap":
                first, second, last = [rect(f'.rail-actions button:nth-child({i})') for i in (1, 2, 3)]
                start = [round((first["right"]+second["left"])/2), point(first)[1]]
                end = point(last, True)
            elif target == "adjacent-rows":
                start, end = point(rect(selectors["agent"])), point(rect(selectors["terminal"]), True)
            else:
                box = rect(selectors[target])
                start, end = point(box), point(box, True)
            if reverse:
                start, end = end, start
            drag(start, end)
            measured = evaluate("({selection:getSelection().toString(),rangeCount:getSelection().rangeCount,viewport:[innerWidth,innerHeight]})")
            name = f'{width}-{target}-{"reverse" if reverse else "forward"}'
            if target in ("done-gap", "add-gap"):
                browser("screenshot", str(OUT / f"{name}.png"))
            observation = dict(name=name, start=start, end=end, styles=styles, **measured)
            observations.append(observation)
            print(json.dumps(observation), flush=True)
            if measured["selection"] or measured["viewport"] != [width, 800]:
                failures.append(name)

def record(name, actual, expected):
    observations.append(dict(name=name, actual=actual, expected=expected))
    if actual != expected:
        failures.append(name)
    print(json.dumps(observations[-1]), flush=True)


def calls(method):
    return evaluate(f"__fake.app.calls.filter(c=>c.method=={json.dumps(method)}).map(c=>c.params)")


def field_state():
    return evaluate("""(()=>{const e=document.querySelector('input[aria-label=name]');
        return {start:e.selectionStart,end:e.selectionEnd,value:e.value}})()""")


for width in (1280, 700):
    browser("set", "viewport", width, 800)
    reset()
    (OUT / f"{width}-accessibility.txt").write_text(browser("snapshot")["snapshot"])
    for cancel in (False, True):
        reset()
        browser("dblclick", selectors["group"])
        browser("wait", 'input[aria-label="name"]')
        record(f"{width}-rename-open", field_state(), dict(start=0, end=7, value="backend"))
        box = rect('input[aria-label="name"]')
        start = [round(box["left"]+4), round(box["top"]+box["height"]/2)]
        end = [round(box["right"]-4), start[1]]
        drag(start, end)
        record(f"{width}-rename-pointer", field_state(), dict(start=0, end=7, value="backend"))
        browser("screenshot", str(OUT / f"{width}-rename-selected.png"))
        browser("press", "End")
        browser("press", "Shift+ArrowLeft")
        browser("press", "Shift+ArrowLeft")
        record(f"{width}-rename-keyboard", field_state(), dict(start=5, end=7, value="backend"))
        browser("keyboard", "type", "XY")
        record(f"{width}-rename-replacement", field_state()["value"], "backeXY")
        browser("press", "Escape" if cancel else "Enter")
        record(f"{width}-rename-{'escape' if cancel else 'enter'}", calls("rail.rename"),
               [] if cancel else [dict(id="backend", name="backeXY")])
        record(f"{width}-rename-no-move", calls("rail.move"), [])
        record(f"{width}-rename-row", evaluate("document.querySelector('[data-id=\"backend\"] .name').textContent"),
               "backend" if cancel else "backeXY")
    for ending in ("drop", "escape", "outside"):
        reset()
        source = rect(selectors["agent"])
        home = rect('[data-id="payments"]')
        start = point(source)
        end = [round(home["left"]+20), round(home["bottom"]-1)]
        drag(start, end, release=False)
        record(f"{width}-{ending}-lift", evaluate("document.querySelector('[data-id=\"migrate\"]').dataset.lifted"), "true")
        record(f"{width}-{ending}-line", evaluate('document.querySelector(".drop-line") !== null'), True)
        browser("screenshot", str(OUT / f"{width}-{ending}-lift.png"))
        if ending == "escape":
            browser("press", "Escape")
        elif ending == "outside":
            browser("mouse", "move", width-20, end[1])
        browser("mouse", "up")
        expected = [] if ending == "escape" else [dict(id="migrate", parent="payments", index=0)]
        record(f"{width}-{ending}-move", calls("rail.move"), expected)
        record(f"{width}-{ending}-selection", evaluate("getSelection().toString()"), "")
        record(f"{width}-{ending}-line-cleared", evaluate('document.querySelector(".drop-line") === null'), True)
        if ending == "drop":
            record(f"{width}-new-home", evaluate('__fake.app.rpc("rail.tree",{}).then(t=>t.find(n=>n.id==="migrate").parent)'), "payments")
            browser("screenshot", str(OUT / f"{width}-changed-home.png"))
    reset()
    browser("click", selectors["group"])
    record(f"{width}-row-click", evaluate("document.querySelector('[data-id=\"backend\"]').getAttribute('aria-selected')"), "true")
    browser("focus", '[data-id="backend"]')
    browser("press", "ArrowDown")
    record(f"{width}-row-focus", evaluate("document.activeElement.dataset.id"), "auth")
    browser("click", '[data-id="backend"] button[aria-label="collapse"]')
    record(f"{width}-group-fold", evaluate("document.querySelector('[data-id=\"backend\"]').getAttribute('aria-expanded')"), "false")
    reset()
    browser("click", '.rail-fold button')
    record(f"{width}-done-unfold", evaluate("document.querySelector('[data-id=\"docs\"]') !== null"), True)
    for name, method, params in (
        ("agent", "agent.spawn", dict(cwd="/Users/you/harness", prompt=None, parent=None)),
        ("terminal", "rail.spawnTerminal", dict(cwd="/Users/you/harness", parent=None)),
        ("group", "rail.createGroup", dict(name="group", parent=None)),
    ):
        reset()
        browser("find", "role", "button", "hover", "--name", name, "--exact")
        browser("focus", selectors[f"add-{name}"])
        record(f"{width}-{name}-focus", evaluate("document.activeElement.textContent.trim()"), name)
        browser("press", "Enter")
        record(f"{width}-{name}-action", calls(method), [params])

report = dict(
    revision=subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip(),
    diff=subprocess.check_output(["git", "diff", "--", "apps/desktop/src/rail/styles.css"], text=True),
    browser=evaluate("navigator.userAgent"), observations=observations, failures=failures,
)
(OUT / "selection.json").write_text(json.dumps(report, indent=2) + "\n")
print(f"U142: {len(observations)} observations; {len(failures)} failures", flush=True)
sys.exit(bool(failures))
