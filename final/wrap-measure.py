import json,subprocess,pathlib
root=pathlib.Path('/tmp/u86-wrap')
def ab(rev,*args):
 return subprocess.check_output(['agent-browser','--session','u86-wrap-'+rev,'--profile',str(root/('profile-'+rev)),*args],text=True,timeout=60)
rows=[]
for rev,port in [('base',5152),('head',5151)]:
 for width in [1280,700]:
  ab(rev,'set','viewport',str(width),'800');ab(rev,'open',f'http://127.0.0.1:{port}/harness.html?seed=tree-40');ab(rev,'wait','--text','todo 1')
  measured=json.loads(json.loads(ab(rev,'eval',r'''JSON.stringify((()=>{const row=document.querySelector('[aria-label=todos] [data-id="3"] .row-head button');const walker=document.createTreeWalker(row,NodeFilter.SHOW_TEXT);const chars=[];while(walker.nextNode()){const t=walker.currentNode;for(let i=0;i<t.length;i++){if(!t.data[i].trim())continue;const r=document.createRange();r.setStart(t,i);r.setEnd(t,i+1);const b=r.getBoundingClientRect();chars.push({char:t.data[i],x:b.x,y:b.y})}}const hash=chars.find(c=>c.char==='#');const next=chars.find(c=>c.y>hash.y+1);const s=getComputedStyle(row);return {hash,wrapped:next,delta:next.x-hash.x,rowHeight:row.getBoundingClientRect().height,padding:s.paddingLeft,indent:s.textIndent,computedIndent:s.getPropertyValue('--todo-row-indent'),viewport:[innerWidth,innerHeight]}})())''')))
  assert measured['wrapped'],measured
  if rev=='base': assert abs(measured['delta'])>1,measured
  else: assert abs(measured['delta'])<.5,measured
  measured.update(revision=rev,width=width)
  ab(rev,'screenshot',str(root/'png'/f'{rev}-wrap-{width}x800.png'))
  rows.append(measured)
(root/'wrap-checks.json').write_text(json.dumps(rows,indent=2)+'\n')
print(json.dumps(rows))
