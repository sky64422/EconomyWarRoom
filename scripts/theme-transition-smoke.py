"""Virtual-clock theme changes: include pending persistence, first paint and rollback."""
from pathlib import Path
import argparse,json
from playwright.sync_api import sync_playwright
ROOT=Path(__file__).resolve().parent.parent
ap=argparse.ArgumentParser();ap.add_argument('--port',type=int,default=1420);ap.add_argument('--token',action='store_true');args=ap.parse_args()
FIXTURE=r'''token=>{
 window.fixture={settings:{theme:'dark',opacity:.92,autostart:false,hover_detail:false,always_show_notch:true,show_animation:true,show_orbit:true,show_icon_glow:true,quote_refresh_secs:10000,notch:{edge:'right',offset:.5,monitor_hint:'mock'},claude:{enabled:true},codex:{enabled:true},grok:{enabled:true},agy:{enabled:true}},watchlist:[{id:'a',symbol:'AAPL',display_name:'Apple',asset_kind:'equity',sort_index:0,card_tint:'none'}]};
 window.snapshots=['claude','codex','grok','agy'].map(provider_id=>({provider_id,display_name:provider_id,windows:[],status:'unavailable',source:'unavailable',message:null,as_of:'fixture',primary_resets_at:null,primary_used_percent:null}));
 window.callbacks={};window.handlers={};window.nativeRequests=[];window.holdTheme=false;window.rejectTheme=false;let next=0;const cache=new Map();
 window.__TAURI_INTERNALS__={metadata:{currentWindow:{label:'main'},currentWebview:{label:'main'}},transformCallback:cb=>{window.callbacks[++next]=cb;return next;},unregisterCallback:id=>delete window.callbacks[id],invoke:async(c,a={})=>{
  if(c==='get_state')return structuredClone(window.fixture);
  if(c==='get_quotes')return [{symbol:'AAPL',price:200,currency:'USD',as_of:'fixture',source:'fixture',display:{primary:{price:200,change:1},secondary:{price:null,change:null}}}];
  if(c==='get_sparklines'||c==='get_provider_activity')return [];
  if(c==='get_snapshots')return structuredClone(window.snapshots);
  if(c==='get_notch_reveal')return true;
  if(c==='plugin:app|version')return '0.4.1';
  if(c==='plugin:event|listen'){(window.handlers[a.event]??=[]).push(window.callbacks[a.handler]);return next;}
  if(c==='set_theme'){
   if(window.holdTheme)await new Promise(resolve=>window.finishTheme=resolve);
   if(window.rejectTheme)throw Error('test theme persistence failure');
   window.fixture.settings.theme=a.theme;return;
  }
  if(c==='set_notch_surface'){
   window.nativeRequests.push(a);const height=Math.round(a.height),count=token?4:1;
   return {edge:'right',anchor_x:'left',anchor_y:'top',scale:1,monitor:'mock',window:{x:0,y:0,width:720,height:800},notch:{x:664,y:180,width:56,height:count*72+88},detail:a.expanded?{x:396,y:Math.round(400-height/2),width:260,height}:null,metrics:{slot_count:count,content_scale:window.contentScale??1,depth:56,cell:72,shoulder:28,inner_radius:28,inset:44,detail_radius:16,drag_threshold:5,rest_depth:10,rest_length:80}};
  }
  if(c==='release_liquid_backdrop'){cache.clear();return;}
  if(c==='capture_liquid_backdrop'){
   const key=JSON.stringify(a);if(cache.get(a.surface)===key)return new ArrayBuffer(0);cache.set(a.surface,key);
   const b=new ArrayBuffer(12+a.width*a.height*4),v=new DataView(b),pixels=new Uint8Array(b);pixels.set([76,81,48,49]);v.setUint32(4,a.width,true);v.setUint32(8,a.height,true);for(let i=12;i<pixels.length;i+=4)pixels.set([60,80,100,255],i);return b;
  }
  return null;
 }};
}'''
with sync_playwright() as p:
 browser=p.chromium.launch(channel='msedge',headless=True);results=[]
 for dpr,scale in [(1,1),(1,.92),(2,1),(2,.92)]:
  page=browser.new_page(viewport={'width':720,'height':800},device_scale_factor=dpr,reduced_motion='no-preference')
  errors=[];page.on('pageerror',lambda e:errors.append(str(e)));page.clock.install()
  page.add_init_script('('+FIXTURE+')('+json.dumps(args.token)+');window.contentScale='+str(scale))
  page.goto(f'http://localhost:{args.port}');page.wait_for_selector('.notch-cell');page.evaluate('document.fonts.ready')
  page.locator('.notch-cell').first.dispatch_event('click');page.clock.run_for(800)
  page.locator('.notch').dispatch_event('contextmenu');page.clock.run_for(800)
  # Clock mocks drive timers/rAF; finish opening animations on the document timeline.
  page.evaluate('document.getAnimations().forEach(a=>{if(a.effect?.getTiming().iterations!==Infinity)a.finish();})')
  page.clock.run_for(32)
  page.evaluate('''()=>{
   window.themeSelectors=['.notch','.notch-cells','.ring-wrap','.notch-detail','.settings-tabs','.settings-scroll'];
   window.themeGeometry=()=>Object.fromEntries(window.themeSelectors.map(s=>{const r=document.querySelector(s).getBoundingClientRect();return [s,[r.x,r.y,r.width,r.height]];}));
  }''')
  baseline=page.evaluate('window.themeGeometry()');failures=[]
  assert page.locator('.settings-save-status').count()==0
  for theme in ['glass','liquid','dark','liquid','glass','dark','glass']:
   page.evaluate('''()=>{window.holdTheme=true;window.themeFrames=[];window.themeRecording=true;function tick(){window.themeFrames.push(window.themeGeometry());if(window.themeRecording)requestAnimationFrame(tick);}tick();}''')
   page.locator(f'[data-theme-option="{theme}"]').dispatch_event('click')
   page.wait_for_function('typeof window.finishTheme==="function"');page.clock.run_for(64)
   page.evaluate('window.finishTheme();delete window.finishTheme');page.clock.run_for(450)
   frames=page.evaluate('window.themeRecording=false;window.themeFrames')
   for frame in frames:
    for selector,values in frame.items():
     if max(abs(a-b) for a,b in zip(values,baseline[selector]))>.1:
      failures.append({'theme':theme,'selector':selector,'before':baseline[selector],'after':values})
   # Failed theme saves must leave the settings geometry and the previous theme intact.
  page.evaluate('window.rejectTheme=true;window.holdTheme=false')
  page.locator('[data-theme-option="liquid"]').dispatch_event('click');page.clock.run_for(450)
  assert page.locator('.notch-shell').get_attribute('data-theme')=='glass'
  assert page.locator('[data-theme-option="glass"]').get_attribute('aria-pressed')=='true'
  assert page.locator('.settings-save-status').count()==0
  # Re-measure all tabs through the real app, not only the already-sized active panel.
  middle='models' if args.token else 'symbols'
  for tab in [middle,'general','appearance']:
   page.locator('#tab-btn-'+tab).dispatch_event('click');page.clock.run_for(450)
   frame=page.evaluate('window.themeGeometry()')
   for selector,values in frame.items():
    if max(abs(a-b) for a,b in zip(values,baseline[selector]))>.1:failures.append({'rollback_tab':tab,'selector':selector,'before':baseline[selector],'after':values})
  assert not errors,errors
  results.append({'dpr':dpr,'scale':scale,'baseline':baseline,'failures':failures[:12],'failure_count':len(failures)})
  print(json.dumps(results[-1]),flush=True)
  page.screenshot(path=str(ROOT/f'tmp/theme-transition-{dpr}-{scale}.png'))
  page.close()
 (ROOT/'tmp/theme-transition-results.json').write_text(json.dumps(results,indent=2))
 browser.close()
 assert all(r['failure_count']==0 for r in results),'Theme switching moved the notch or settings; see results'
