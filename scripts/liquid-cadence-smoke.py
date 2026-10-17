"""Virtual-clock renderer regression for idle capture, motion recovery and fold."""
from pathlib import Path
import argparse, json
from playwright.sync_api import sync_playwright

ROOT = Path(__file__).resolve().parent.parent
parser = argparse.ArgumentParser()
parser.add_argument('--port', type=int, default=1523)
parser.add_argument('--theme', choices=['glass','liquid'], default='liquid')
args = parser.parse_args()
with sync_playwright() as p:
    browser = p.chromium.launch(channel='msedge', headless=True)
    page = browser.new_page(viewport={'width':700,'height':600})
    page.route('**/src/main.ts', lambda r:r.fulfill(content_type='text/javascript',body=''))
    page.clock.install()
    page.goto(f'http://localhost:{args.port}')
    page.evaluate('''async theme=>{
      document.body.innerHTML='<div class="notch-shell"><nav class="notch" style="width:52px;height:300px"><svg><path id="notch-outline" d="M0 0H52V300H0Z"/></svg><div class="notch-finish"></div></nav><section class="notch-detail" style="width:260px;height:140px"></section></div>';
      document.querySelector('.notch-shell').dataset.theme=theme;
      window.captures=0;window.color=30;window.paints=0;const cache=new Map();
      window.__TAURI_INTERNALS__={invoke:async(command,a)=>{
        if(command==='release_liquid_backdrop'){cache.clear();return;}
        if(command!=='capture_liquid_backdrop')throw Error(command);
        window.captures++;
        const key=JSON.stringify([a,window.color]);
        if(cache.get(a.surface)===key)return new ArrayBuffer(0);
        cache.set(a.surface,key);
        const data=new ArrayBuffer(12+a.width*a.height*4),v=new DataView(data),bytes=new Uint8Array(data);
        bytes.set([76,81,48,49]);v.setUint32(4,a.width,true);v.setUint32(8,a.height,true);
        for(let i=12;i<bytes.length;i+=4)bytes.set([window.color,60,100,255],i);
        return data;
      }};
      const draw=CanvasRenderingContext2D.prototype.putImageData;
      CanvasRenderingContext2D.prototype.putImageData=function(...a){window.paints++;return draw.apply(this,a);};
      const {mountLiquidGlass}=await import('/src/ui/liquid-glass.ts');
      window.renderer=mountLiquidGlass(document.querySelector('.notch-shell'),document.querySelector('.notch'),document.querySelector('.notch-detail'));
      window.renderer.setEnabled(true);
    }''', args.theme)
    page.clock.run_for(3200)
    before=page.evaluate('window.captures')
    page.clock.run_for(1000)
    idle=page.evaluate('window.captures')-before
    assert 6<=idle<=10, {'idle_captures_two_surfaces':idle}
    latencies=[]
    for color in [80,90,100]:
        # Start motion immediately after an unchanged read: exercise the slowest phase.
        previous=page.evaluate('window.captures')
        for _ in range(18):
            page.clock.run_for(16)
            if page.evaluate('window.captures')>previous:
                break
        assert page.evaluate('window.captures')>previous
        page.evaluate('(color)=>window.color=color',color)
        latency=0
        while latency<272:
            page.clock.run_for(16);latency+=16
            if page.locator('.notch-detail canvas').evaluate('(c)=>c.getContext("2d").getImageData(1,1,1,1).data[0]')==color:
                break
        assert latency<=272, {'resume_latency_ms':latency}
        assert page.locator('.notch-detail canvas').evaluate('(c)=>c.getContext("2d").getImageData(1,1,1,1).data[0]')==color
        latencies.append(latency)
        page.clock.run_for(3200)
    page.evaluate('window.color=80')
    page.clock.run_for(300)
    before=page.evaluate('window.captures')
    # Keep background changing, so cadence must recover to the normal refresh rate.
    page.evaluate('window.motion=setInterval(()=>window.color=window.color===80?90:80,30)')
    page.clock.run_for(1000)
    moving=page.evaluate('window.captures')-before
    assert 55<=moving<=64, {'moving_captures_two_surfaces':moving}
    page.evaluate('clearInterval(window.motion);document.querySelector(".notch").classList.add("is-dragging")')
    before=page.evaluate('window.captures')
    page.clock.run_for(1000)
    drag=page.evaluate('window.captures')-before
    assert 110<=drag<=126, {'drag_captures_two_surfaces':drag}
    page.evaluate('document.querySelector(".notch").classList.remove("is-dragging")')
    page.clock.run_for(3200)
    page.evaluate('document.querySelector(".notch-detail").hidden=true')
    page.clock.run_for(3200)
    page.evaluate('document.querySelector(".notch-detail").hidden=false')
    page.clock.run_for(50)
    assert page.locator('.notch-detail .liquid-layer').evaluate('(el)=>!el.hidden')
    page.evaluate('document.querySelector(".notch").classList.remove("is-dragging");document.querySelector(".notch-detail").hidden=true;document.querySelector(".notch").classList.add("is-folded")')
    page.clock.run_for(50)
    before=page.evaluate('window.captures')
    page.clock.run_for(1000)
    assert page.evaluate('window.captures')==before
    page.evaluate('document.querySelector(".notch").classList.remove("is-folded");document.querySelector(".notch-detail").hidden=false')
    page.clock.run_for(50)
    assert page.evaluate('window.captures')>before
    page.evaluate('window.renderer.destroy()')
    result={'theme':args.theme,'idle_two_surface_captures_per_second':idle,'moving':moving,'drag':drag,'resume_latencies_ms':latencies,'detail_reentry_within_ms':50,'fold_stops':True,'reveal_resumes':True}
    (ROOT/f'tmp/liquid-cadence-{args.theme}-results.json').write_text(json.dumps(result,indent=2))
    print(json.dumps(result))
    browser.close()
