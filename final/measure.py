import subprocess,json,pathlib
root=pathlib.Path('/tmp/u86-final-proof-69108b8')
def ab(rev,*args):
 p=subprocess.run(['agent-browser','--session','u86-delivery-'+rev,'--profile',str(root/('profile-'+rev)),*args],capture_output=True,text=True,check=True,timeout=60)
 return p.stdout
rows=[]
for rev,port in [('base',5152),('head',5151)]:
 for width in [1280,700]:
  ab(rev,'set','viewport',str(width),'800'); ab(rev,'set','media','light')
  ab(rev,'open',f'http://127.0.0.1:{port}/harness.html?seed=tree-40'); ab(rev,'wait','--text','todo 1')
  ab(rev,'eval',"document.querySelector('[aria-label=todos] .row-head button').focus()")
  ab(rev,'press','Shift+Tab'); ab(rev,'press','Tab')
  measured=json.loads(json.loads(ab(rev,'eval',r'''JSON.stringify((()=>{const todo=document.querySelector('[aria-label=todos] .row-head button');const s=getComputedStyle(todo);const b=e=>{const r=e.getBoundingClientRect();return {height:r.height,width:r.width}};return {focused:document.activeElement===todo,focusVisible:todo.matches(':focus-visible'),outline:s.outline,offset:s.outlineOffset,todoBox:b(todo),whiteSpace:s.whiteSpace,paddingLeft:s.paddingLeft,textIndent:s.textIndent,icons:[...document.querySelectorAll('svg.icon')].map(e=>b(e)),glyphs:[...document.querySelectorAll('.glyph')].map(e=>({kind:e.getAttribute('aria-label'),tone:e.getAttribute('data-tone'),color:getComputedStyle(e).color,...b(e)})),buttons:[...document.querySelectorAll('button')].filter(e=>e.querySelector('svg')).map(e=>({name:e.getAttribute('aria-label')||e.textContent,...b(e)})),rows:[...document.querySelectorAll('.rail-row')].map(e=>({id:e.dataset.id,...b(e)}))}})())''')))
  assert measured['focused'] and measured['focusVisible'],measured
  if rev=='head':
   assert measured['todoBox']['height']>=24 and measured['outline']=='rgb(10, 96, 216) solid 2px',measured
   assert all(i=={'height':16,'width':16} for i in measured['icons'])
   assert all(b['height']>=24 for b in measured['buttons']), measured['buttons']
  measured.update(revision=rev,width=width)
  ab(rev,'screenshot',str(root/'png'/f'{rev}-todo-focus-{width}x800-light.png'))
  rows.append(measured)
(root/'checks.json').write_text(json.dumps(rows,indent=2))
print('4 focus captures; head icon16px, vector button24px, Todo keyboard focus2px checks passed')
