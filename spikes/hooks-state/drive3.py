import pty, os, sys, time, select, json, tempfile, re
here=os.path.dirname(os.path.abspath(__file__))
wd=tempfile.mkdtemp(prefix="roundup-spike-")

screen=open(os.path.join(here,"screen3.jsonl"),"w")
pid,fd=pty.fork()
if pid==0:
    os.chdir(wd)
    os.execvp("claude",["claude","--settings",os.path.join(here,"settings.json"),"--setting-sources","local","--model",os.environ.get("M","claude-sonnet-5-5"),"--permission-mode","default"])
import fcntl,termios,struct
fcntl.ioctl(fd,termios.TIOCSWINSZ,struct.pack("HHHH",50,160,0,0))
buf=b""
def mark(m):
    screen.write(json.dumps({"ts":time.time(),"mark":m})+"\n"); screen.flush()
def pump(sec):
    global buf
    end=time.time()+sec
    while time.time()<end:
        r,_,_=select.select([fd],[],[],0.2)
        if r:
            try: d=os.read(fd,65536)
            except OSError: return False
            if not d: return False
            buf+=d
            screen.write(json.dumps({"ts":time.time(),"out":re.sub(r"\x1b\[[0-9;?]*[a-zA-Z]","",d.decode("utf8","replace"))[-300:]})+"\n"); screen.flush()
    return True
def send(s,m):
    mark(m); os.write(fd,s.encode())
pump(6)
send("\x1b[B","down"); pump(1); send("\r","trust-yes"); pump(6)

def prompt(t,m,wait):
    send(t,"type:"+m); pump(1); send("\r","submit:"+m); pump(wait)
if os.environ.get("M"):
    prompt("hi","bogus-model",20)
else:
    prompt("say ok","ok",12)
    mark("idle80"); pump(80)
    prompt("run the shell command: touch z.txt","deny",12)
    send("\x1b[B\x1b[B\x1b[B\r","deny-select4"); pump(15)
    mark("idle-after-deny"); pump(5)
    prompt("run the shell command: touch w.txt","cancelperm",12)
    send("\x1b","esc-perm"); pump(10)
os.kill(pid,9); pump(1)
