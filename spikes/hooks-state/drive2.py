import pty, os, sys, time, select, json, tempfile, re
here=os.path.dirname(os.path.abspath(__file__))
wd=tempfile.mkdtemp(prefix="roundup-spike-")
open(os.path.join(here,"log2.jsonl"),"w").close()
screen=open(os.path.join(here,"screen2.jsonl"),"w")
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

def prompt(t,m,wait):
    send(t,"type:"+m); pump(1); send("\r","submit:"+m); pump(wait)
prompt("run the shell command: ls /nonexistent_dir_xyz","toolfail",20)
prompt("run the shell command: touch y.txt","deny",14)
send("4","deny-key"); pump(1); send("\r","deny-enter"); pump(15)
prompt("Use the AskUserQuestion tool to ask me whether I prefer A or B","ask",20)
send("\x1b","esc-ask"); pump(10)
prompt("write a 600 word story about a lighthouse","story",4)
send("\x1b","esc-interrupt"); pump(10)
mark("idle70"); pump(70)
import signal
mark("sigkill"); os.kill(pid,signal.SIGKILL); pump(3)
print(wd)
