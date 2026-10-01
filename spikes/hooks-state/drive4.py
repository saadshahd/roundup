# usage: drive4.py <run-n> <esc-allowed|esc-dialog|approve3>
import pty, os, sys, time, select, json, tempfile, re, fcntl, termios, struct, copy
here=os.path.dirname(os.path.abspath(__file__))
n, scen = sys.argv[1], sys.argv[2]
wd=tempfile.mkdtemp(prefix="roundup-spike-")
logp=os.path.join(here,f"log.run{n}.jsonl"); scrp=os.path.join(here,f"screen.run{n}.jsonl")
open(logp,"w").close()
s=json.load(open(os.path.join(here,"settings.json")))
for ev,g in s["hooks"].items():
    for e in g:
        for h in e["hooks"]: h["command"]=f"python3 {os.path.join(here,'logger2.py')} {logp}"
if scen=="esc-allowed": s["permissions"]={"allow":["Bash(python3:*)"]}
sp=os.path.join(wd,"..",f"roundup-spike-settings-{n}.json"); sp=os.path.abspath(sp)
json.dump(s,open(sp,"w"))
screen=open(scrp,"w")
pid,fd=pty.fork()
if pid==0:
    os.chdir(wd)
    os.execvp("claude",["claude","--settings",sp,"--setting-sources","local","--model","claude-sonnet-5-5","--permission-mode","default"])
fcntl.ioctl(fd,termios.TIOCSWINSZ,struct.pack("HHHH",50,160,0,0))
tail=b""
def mark(m):
    r={"ts":time.time(),"mark":m}
    screen.write(json.dumps(r)+"\n"); screen.flush()
    open(logp,"a").write(json.dumps({"ts":r["ts"],"payload":{"hook_event_name":"MARK","mark":m}})+"\n")
def pump(sec,until=None):
    global tail
    end=time.time()+sec
    while time.time()<end:
        if until and until(): return True
        r,_,_=select.select([fd],[],[],0.05)
        if r:
            try: d=os.read(fd,65536)
            except OSError: return False
            if not d: return False
            ts=time.time()
            txt=d.decode("utf8","replace")
            screen.write(json.dumps({"ts":ts,"out":re.sub(r"\x1b\[[0-9;?]*[a-zA-Z]","",txt)[-300:]})+"\n")
            for t in re.findall(r"\x1b\]0;([^\x07\x1b]*)",txt):
                screen.write(json.dumps({"ts":ts,"title":t})+"\n")
            screen.flush()
    return False
def hooks(): return [json.loads(l)["payload"].get("hook_event_name") for l in open(logp) if l.strip()]
def seen(ev,k=1): return lambda: hooks().count(ev)>=k
def send(x,m): mark(m); os.write(fd,x.encode())
pump(6); send("\x1b[B","down"); pump(1); send("\r","trust-yes"); pump(6)
def prompt(t,m): send(t,"type:"+m); pump(1); send("\r","submit:"+m)
if scen=="esc-allowed":
    prompt("Use the Bash tool to run exactly `python3 -c \"import time; time.sleep(30)\"` (run_in_background false, timeout 60000), then say done","p")
    pump(30,seen("PreToolUse")); mark("pretooluse-seen"); pump(3)
    send("\x1b","esc"); pump(12); mark("end")
elif scen=="esc-dialog":
    prompt("run the shell command: touch q.txt","p")
    pump(30,seen("PermissionRequest")); mark("permreq-seen"); pump(2.5)
    send("\x1b","esc-on-dialog"); pump(12); mark("end")
else:
    for i,c in enumerate(["touch a1.txt","touch b2.txt","touch c3.txt"],1):
        prompt(f"run the shell command: {c}",f"p{i}")
        pump(40,seen("PermissionRequest",i)); pump(2.5)
        send("\r",f"yes{i}"); pump(40,seen("Stop",i)); pump(3)
    mark("end")
os.kill(pid,9); pump(1)
