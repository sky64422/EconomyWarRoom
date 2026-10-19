"""Virtual-clock native geometry updates: zero idle geometry IPC, animation tracking and fold."""
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
    errors = []
    page.on('pageerror', lambda error: errors.append(str(error)))
    page.route('**/src/main.ts', lambda route: route.fulfill(content_type='text/javascript',body=''))
    page.clock.install()
    page.goto(f'http://localhost:{args.port}')
    page.evaluate("async()=>{await import('/src/styles/tokens.css');await import('/src/styles/app.css')}")
    page.evaluate('''async theme=>{
      document.body.innerHTML='<div class="notch-shell"><nav class="notch" data-edge="right" style="position:absolute;left:320px;top:20px;width:64px;height:300px"><div class="notch-finish"></div></nav><section class="notch-detail" style="position:absolute;left:20px;top:20px;width:260px;height:140px;border-radius:16px"></section></div>';
      document.querySelector('.notch-shell').dataset.theme=theme;
      window.calls=[];
      window.__TAURI_INTERNALS__={invoke:async(command,args)=>{
        window.calls.push({command,args});
        if(command==='begin_liquid_backdrop')return 1;
        if(command==='capture_liquid_backdrop')return new ArrayBuffer(0);
        if(command!=='sync_liquid_backdrop'&&command!=='release_liquid_backdrop')throw Error(command);
      }};
      const {mountLiquidGlass}=await import('/src/ui/liquid-glass.ts');
      window.renderer=mountLiquidGlass(document.querySelector('.notch-shell'),document.querySelector('.notch'),document.querySelector('.notch-detail'));
      window.renderer.setEnabled(true);
    }''', args.theme)
    def settle():
        page.evaluate('document.getAnimations().forEach(a=>{if(a.effect?.getTiming().iterations!==Infinity)a.finish();})')
        page.clock.run_for(64)
    settle()
    before = page.evaluate('window.calls.filter(c=>c.command!=="capture_liquid_backdrop").length')
    page.clock.run_for(1000)
    idle = page.evaluate('window.calls.filter(c=>c.command!=="capture_liquid_backdrop").length') - before
    assert idle == 0, {'idle_ipc':idle}
    # Exercise a real CSS keyframe animation, advancing its own timeline explicitly.
    page.evaluate('''()=>{
      const rail=document.querySelector('.notch');
      window.moving=rail.animate([{transform:'translateX(0px)'},{transform:'translateX(-40px)'}],{duration:320,fill:'forwards'});
      rail.dispatchEvent(new Event('animationstart',{bubbles:true}));
    }''')
    positions = []
    for time in [0,80,160,240,320]:
        page.evaluate('(time)=>window.moving.currentTime=time',time)
        page.clock.run_for(32)
        positions.append(page.evaluate('window.calls.filter(c=>c.command==="sync_liquid_backdrop").at(-1).args.surfaces[0].x'))
    assert positions[0] - positions[-1] == 40, positions
    assert len(set(positions)) >= 4, positions
    page.evaluate('window.moving.finish()')
    settle()
    before = page.evaluate('window.calls.filter(c=>c.command!=="capture_liquid_backdrop").length')
    page.clock.run_for(1000)
    assert page.evaluate('window.calls.filter(c=>c.command!=="capture_liquid_backdrop").length') == before
    # DOM style movement is observed; no forever-running geometry loop is required.
    page.evaluate('document.querySelector(".notch").style.left="330px"')
    page.clock.run_for(32)
    assert page.evaluate('window.calls.filter(c=>c.command==="sync_liquid_backdrop").at(-1).args.surfaces[0].x') == 290
    page.evaluate('document.querySelector(".notch-detail").hidden=true;document.querySelector(".notch").classList.add("is-folded")')
    page.clock.run_for(64)
    assert page.evaluate('window.calls.at(-1).command') == 'release_liquid_backdrop'
    before = page.evaluate('window.calls.filter(c=>c.command!=="capture_liquid_backdrop").length')
    page.clock.run_for(1000)
    assert page.evaluate('window.calls.filter(c=>c.command!=="capture_liquid_backdrop").length') == before
    page.evaluate('document.querySelector(".notch").classList.remove("is-folded");document.querySelector(".notch-detail").hidden=false')
    settle()
    assert page.locator('.notch-shell').get_attribute('data-liquid-status') == 'ready'
    assert page.evaluate('window.calls.at(-1).args.surfaces.length') == 2
    page.evaluate('window.renderer.destroy()')
    page.clock.run_for(32)
    assert not errors, errors
    result = {'theme':args.theme,'idle_geometry_ipc_per_second':idle,'animated_positions':positions,
              'animation_stops_polling':True,'style_movement_observed':True,'fold_clears_native':True,'reveal_resumes':True}
    (ROOT/f'tmp/liquid-cadence-{args.theme}-results.json').write_text(json.dumps(result,indent=2))
    print(json.dumps(result))
    browser.close()
