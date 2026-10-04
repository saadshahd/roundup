import subprocess,json,pathlib,hashlib
ROOT=pathlib.Path('/tmp/u86-final-proof-69108b8')
MEASURE=r'''JSON.stringify((()=>{const s=getComputedStyle(document.documentElement); const visible=e=>!!(e.getBoundingClientRect().width&&e.getBoundingClientRect().height);const box=e=>{const b=e.getBoundingClientRect();return {x:b.x,y:b.y,width:b.width,height:b.height}};return {url:location.href,viewport:[innerWidth,innerHeight],dark:matchMedia('(prefers-color-scheme: dark)').matches,colorScheme:s.colorScheme,tokens:Object.fromEntries(['--ground','--sunken','--text','--grey','--accent','--red','--amber'].map(k=>[k,s.getPropertyValue(k).trim()])),bodyText:document.body.innerText,alerts:[...document.querySelectorAll('[role=alert]')].map(e=>({text:e.textContent,...box(e)})),fixtureNodes:window.__fake.app.handlers['rail.tree']().length,rpcFailure:window.__qaFailure??null,regions:[...document.querySelectorAll('section[aria-label], [role=region], main[aria-label], aside[aria-label]')].map(e=>({name:e.getAttribute('aria-label'),background:getComputedStyle(e).backgroundColor,...box(e)})),icons:[...document.querySelectorAll('svg')].filter(visible).map(e=>({class:e.getAttribute('class'),label:e.getAttribute('aria-label'),...box(e)})),buttons:[...document.querySelectorAll('button')].filter(visible).map(e=>({name:e.getAttribute('aria-label')||e.textContent,color:getComputedStyle(e).color,...box(e)}))}})())'''
def ab(rev,*args):
    p=subprocess.run(['agent-browser','--session','u86-delivery-'+rev,'--profile',str(ROOT/('profile-'+rev)),*args],capture_output=True,text=True,check=True)
    with (ROOT/'logs'/'browser.log').open('a') as f:f.write(json.dumps([rev,*args])+'\n'+p.stdout+p.stderr)
    return p.stdout
results=[]
for rev,port in [('base',5152),('head',5151)]:
  for width in [1280,700]:
    ab(rev,'set','viewport',str(width),'800')
    for theme in ['light','dark']:
      ab(rev,'set','media',theme)
      for seed in ['tree-40','agents-10','first-run','conflict']:
        existing=list((ROOT/'evidence').glob(f'{rev}-{seed}-{width}x800-{theme}*.json'))
        if existing:
          results.append(json.loads(existing[0].read_text()))
          continue
        ab(rev,'open',f'http://127.0.0.1:{port}/harness.html?seed={seed}')
        try:
          ab(rev,'wait','--text','open a folder to start' if seed=='first-run' else 'agent-1')
        except subprocess.CalledProcessError:
          ab(rev,'reload')
          ab(rev,'wait','--text','open a folder to start' if seed=='first-run' else 'agent-1')
        if seed=='conflict':
          ab(rev,'eval',"window.__qaFailure=null; const rpc=window.__fake.app.rpc; window.__fake.app.rpc=async (...args)=>{try{return await rpc(...args)}catch(e){window.__qaFailure={method:args[0],params:args[1],code:e.code,message:e.message};throw e}}")
          ab(rev,'find','role','button','click','--name','+ group' if rev=='base' else 'group','--exact')
          ab(rev,'wait','--text','name already taken')
        data=json.loads(json.loads(ab(rev,'eval',MEASURE)))
        assert data['viewport']==[width,800]
        assert data['dark']==(theme=='dark')
        assert data['fixtureNodes']==({'first-run':0,'tree-40':40}.get(seed,10))
        if seed=='conflict':
          assert data['rpcFailure']['code']==-32003, data
          assert data['rpcFailure']['method']=='rail.createGroup',data
          assert 'name already taken' in data['bodyText'],data
          if rev=='head': assert any('name already taken' in a['text'] for a in data['alerts']),data
        validDark=data['tokens']['--ground'].lower()=='#1c1c1e' and data['tokens']['--sunken'].lower()=='#141416'
        suffix='dark-preference-BLOCKED-light-rendering' if theme=='dark' and not validDark else theme
        stem=f'{rev}-{seed}-{width}x800-{suffix}'
        path=ROOT/'png'/(stem+'.png')
        ab(rev,'screenshot',str(path))
        data.update(commit={'base':'a1f4d5eae322470951bb4906b31412af564bed15','head':'69108b822a5e21d8f94906585ab491f668ef0778'}[rev],revision=rev,seed=seed,requestedTheme=theme,darkPaletteValid=validDark,png=str(path),sha256=hashlib.sha256(path.read_bytes()).hexdigest())
        (ROOT/'evidence'/(stem+'.json')).write_text(json.dumps(data,indent=2)+'\n')
        results.append(data)
        print(stem,flush=True)
(ROOT/'manifest.json').write_text(json.dumps(results,indent=2)+'\n')
