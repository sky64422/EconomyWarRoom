"""Real renderer lifecycle regression with deterministic raw desktop frames; no native writes."""
from pathlib import Path
import argparse,json
from playwright.sync_api import sync_playwright,expect
ROOT=Path(__file__).resolve().parent.parent
ap=argparse.ArgumentParser();ap.add_argument('--port',type=int,default=1523);args=ap.parse_args()
with sync_playwright() as p:
 b=p.chromium.launch(channel='msedge',headless=True)
 page=b.new_page(viewport={'width':700,'height':600},reduced_motion='reduce')
 errors=[];page.on('pageerror',lambda e:errors.append(str(e)))
 page.route('**/src/main.ts',lambda r:r.fulfill(status=200,content_type='text/javascript',body=''))
 page.add_init_script("""() => {}""")
 page.add_init_script("""
 window.maps=0;const toDataURL=HTMLCanvasElement.prototype.toDataURL;
 HTMLCanvasElement.prototype.toDataURL=function(...args){window.maps++;return toDataURL.apply(this,args);};
 window.calls={capture:0,changed:0,release:0};window.color=30;window.holdNext=false;window.pending=null;
 const cache=new Map();
 window.__TAURI_INTERNALS__={invoke:async(c,a)=>{
  if(c==='release_liquid_backdrop'){window.calls.release++;cache.clear();return;}
  if(c!=='capture_liquid_backdrop')throw Error(c);
  window.calls.capture++;
  const signature=[a.surface,a.generation,a.x,a.y,a.width,a.height,window.color].join(':');
  const unchanged=cache.get(a.surface)===signature;cache.set(a.surface,signature);
  let data=new ArrayBuffer(0);
  if(!unchanged){
   window.calls.changed++;data=new ArrayBuffer(12+a.width*a.height*4);
   const bytes=new Uint8Array(data),view=new DataView(data);bytes.set([76,81,48,49]);
   view.setUint32(4,a.width,true);view.setUint32(8,a.height,true);
   for(let i=12;i<bytes.length;i+=4)bytes.set([window.color,60,100,255],i);
  }
  if(a.surface===1&&window.holdNext){window.holdNext=false;return await new Promise(resolve=>window.pending=()=>{window.pending=null;resolve(data);});}
  return data;
 }};
 """)
 page.goto(f'http://localhost:{args.port}')
 page.evaluate("""async()=>{
  await import('/src/styles/fonts.css');await import('/src/styles/tokens.css');
  await import('/src/styles/notch.css');
  document.body.innerHTML='<div class="notch-shell" data-theme="liquid" style="--notch-scale:1;--panel-opacity:.8"><nav class="notch" data-edge="right" style="left:320px;top:20px;width:64px;height:376px"><svg class="notch-shape"><path id="notch-outline" d="M0 0H64V376H0Z"/></svg><div class="notch-finish"></div></nav><section class="notch-detail" style="left:20px;top:20px;width:260px;height:128px"><div class="detail-body">Material</div></section></div>';
  const {mountLiquidGlass}=await import('/src/ui/liquid-glass.ts');
  const shell=document.querySelector('.notch-shell'),rail=document.querySelector('.notch'),detail=document.querySelector('.notch-detail');
  window.renderer=mountLiquidGlass(shell,rail,detail);window.renderer.setEnabled(true);
 }""")
 def painted():page.wait_for_function('!document.querySelector(".notch-detail .liquid-layer").hidden')
 painted();page.wait_for_function('window.calls.capture>=10')
 assert page.evaluate('window.calls.changed')==2
 assert page.evaluate('window.maps')==0, 'Zero refraction must not build a displacement map'
 # Commit a changed native frame, close the detail before its response reaches paint, then reopen.
 page.evaluate('window.color=80;window.holdNext=true')
 page.wait_for_function('window.pending!==null')
 page.evaluate('document.querySelector(".notch-detail").hidden=true')
 page.evaluate('window.pending()')
 page.wait_for_function('window.pending===null')
 page.evaluate('document.querySelector(".notch-detail").hidden=false')
 page.wait_for_function('document.querySelector(".notch-detail canvas").getContext("2d").getImageData(20,20,1,1).data[0]===80',timeout=2500)
 # Both material names capture; switching an enabled material must replace the lens configuration.
 page.evaluate('document.querySelector(".notch-shell").dataset.theme="glass";window.renderer.setEnabled(true)')
 page.wait_for_function('!document.querySelector(".notch-detail .liquid-layer").hidden')
 glass_scale=page.locator('#liquid-lens-1 feDisplacementMap').nth(1).get_attribute('scale')
 assert page.locator('.notch-detail canvas').evaluate('(c)=>c.style.filter.includes("url(")')
 maps=page.evaluate('window.maps');assert maps>0
 page.evaluate('document.querySelector(".notch-shell").dataset.theme="liquid";window.renderer.setEnabled(true)')
 page.wait_for_function('!document.querySelector(".notch-detail .liquid-layer").hidden')
 assert page.evaluate('window.maps')==maps
 assert page.locator('.liquid-layer canvas').evaluate_all('(els)=>els.every(c=>!c.style.filter.includes("url("))')
 assert page.locator('.liquid-filter-defs feImage').evaluate_all('(els)=>els.every(el=>!el.hasAttribute("href"))')
 liquid_scale='bypassed'
 page.evaluate('window.renderer.setVisible(false)')
 page.wait_for_function('window.calls.release>=2')
 assert page.locator('.liquid-layer canvas').evaluate_all('(els)=>els.every(c=>c.width===0&&c.height===0)')
 captures=page.evaluate('window.calls.capture')
 page.evaluate('window.renderer.setVisible(true)')
 painted();page.wait_for_function('(n)=>window.calls.capture>n',arg=captures)
 page.evaluate('window.renderer.destroy()')
 page.wait_for_function('document.querySelectorAll(".liquid-layer").length===0')
 assert not errors,errors
 result={'page_errors':errors,'unchanged_pixels_skipped':True,'inflight_detail_reentry':True,'material_switch_rebuilds_lens':True,'zero_refraction_bypasses_svg_and_map':True,'glass_scale':glass_scale,'liquid_scale':liquid_scale,'hide_releases_buffers':True,'resume':True,'destroy':True}
 (ROOT/'tmp/liquid-renderer-results.json').write_text(json.dumps(result,indent=2))
 print(json.dumps(result));b.close()
