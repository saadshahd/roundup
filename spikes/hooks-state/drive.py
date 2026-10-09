import pty, os, sys, time, select, json, tempfile, re
here=os.path.dirname(os.path.abspath(__file__))
wd=tempfile.mkdtemp(prefix="roundup-spike-")
open(os.path.join(here,"log.jsonl"),"w").close()
screen=open(os.path.join(here,"screen.jsonl"),"w")
pid,fd=pty.fork()
if pid==0:
    os.chdir(wd)
    os.execvp("claude",["claude","--settings",os.path.join(here,"settings.json"),"--setting-sources","local","--permission-mode","default"])
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
send("say hi in 3 words","type1"); pump(1); send("\r","submit1"); pump(25)
send("run the shell command: touch x.txt","type2"); pump(1); send("\r","submit2"); pump(15)
mark("permission pending, waiting"); pump(8)
send("1","answer-yes"); pump(1); send("\r","answer-enter"); pump(20)
mark("idle"); pump(20)
send("/exit","type-exit"); pump(1); send("\r","submit-exit"); pump(6)
try:
    p,st=os.waitpid(pid,os.WNOHANG); mark(f"waitpid {p} {st}")
except Exception as e: mark(str(e))
print(wd, os.path.exists(os.path.join(wd,"x.txt")))
