"""Native material IPC lifecycle in a real browser with deterministic acknowledgements."""
from pathlib import Path
import argparse, json
from playwright.sync_api import sync_playwright

ROOT = Path(__file__).resolve().parent.parent
parser = argparse.ArgumentParser()
parser.add_argument('--port', type=int, default=1523)
args = parser.parse_args()
with sync_playwright() as p:
    browser = p.chromium.launch(channel='msedge', headless=True)
    page = browser.new_page(viewport={'width':700,'height':600}, device_scale_factor=2)
    errors = []
    page.on('pageerror', lambda error: errors.append(str(error)))
    page.route('**/src/main.ts', lambda route: route.fulfill(content_type='text/javascript', body=''))
    page.clock.install()
    page.goto(f'http://localhost:{args.port}')
    page.evaluate("async()=>{await import('/src/styles/tokens.css');await import('/src/styles/app.css')}")
    page.evaluate('''async()=>{
      document.body.innerHTML='<div class="notch-shell" data-theme="liquid"><nav class="notch" data-edge="right" style="position:absolute;left:320px;top:20px;width:64px;height:376px"><svg><path id="notch-outline" d="M0 0H64V376H0Z"/></svg><div class="notch-finish"></div></nav><section class="notch-detail" style="position:absolute;left:20px;top:20px;width:260px;height:128px;border-radius:16px"></section></div>';
      window.calls=[];window.hold=true;window.pending=null;window.rejectNext=false;
      window.__TAURI_INTERNALS__={invoke:async(command,args)=>{
        window.calls.push({command,args});
        if(command==='begin_liquid_backdrop')return 7;
        if(command==='capture_liquid_backdrop')return new ArrayBuffer(0);
        if(command==='release_liquid_backdrop')return;
        if(command!=='sync_liquid_backdrop')return;
        if(window.rejectNext){window.rejectNext=false;throw Error('synthetic material failure');}
        if(window.hold)await new Promise(resolve=>window.pending=()=>{window.pending=null;resolve();});
      }};
      const {mountLiquidGlass}=await import('/src/ui/liquid-glass.ts');
      window.renderer=mountLiquidGlass(document.querySelector('.notch-shell'),document.querySelector('.notch'),document.querySelector('.notch-detail'));
      window.renderer.setEnabled(true);
    }''')
    page.clock.run_for(32)
    first = page.evaluate('window.calls[0].command')
    assert first == 'begin_liquid_backdrop', {'expected':'native begin','actual':first}
    assert page.locator('.notch-shell').get_attribute('data-liquid-status') != 'ready'
    assert page.locator('.liquid-layer canvas').count() == 0
    request = page.evaluate('window.calls.find(c=>c.command==="sync_liquid_backdrop").args')
    assert request['session'] == 7 and len(request['surfaces']) == 2, request
    assert request['surfaces'][0] == {'slot':0,'x':640,'y':40,'width':128,'height':752,'edge':'right','radius':64,'blur':6}, request
    assert request['surfaces'][1]['radius'] == 32 and request['surfaces'][1]['blur'] == 20, request
    # An obsolete acknowledgement cannot reveal a detail closed during native sync.
    page.evaluate('document.querySelector(".notch-detail").hidden=true')
    page.clock.run_for(32)
    page.evaluate('window.pending()')
    page.clock.run_for(32)
    assert page.locator('.notch-detail .liquid-layer').is_hidden()
    assert len(page.evaluate('window.calls.filter(c=>c.command==="sync_liquid_backdrop").at(-1).args.surfaces')) == 1
    page.evaluate('window.hold=false;window.pending()')
    page.clock.run_for(32)
    assert page.locator('.notch-shell').get_attribute('data-liquid-status') == 'ready'
    page.evaluate('document.querySelector(".notch-detail").hidden=false')
    page.clock.run_for(32)
    assert not page.locator('.notch-detail .liquid-layer').is_hidden()
    # A rapid disable/reopen must release behind a pending sync before reopening.
    page.evaluate('window.hold=true;document.querySelector(".notch").style.left="321px"')
    page.clock.run_for(32)
    page.evaluate('window.renderer.setEnabled(false);window.renderer.setEnabled(true);window.hold=false;window.pending()')
    page.clock.run_for(64)
    assert page.evaluate('window.calls.filter(c=>c.command==="release_liquid_backdrop").length') >= 1
    assert page.locator('.notch-shell').get_attribute('data-liquid-status') == 'ready'
    page.evaluate('document.querySelector(".notch-shell").dataset.theme="glass";window.renderer.setEnabled(true)')
    page.clock.run_for(32)
    blur = page.evaluate('window.calls.filter(c=>c.command==="sync_liquid_backdrop").at(-1).args.surfaces.map(s=>s.blur)')
    assert blur == [8,20], blur
    assert page.evaluate('window.calls.filter(c=>c.command==="begin_liquid_backdrop").length') == 1
    page.evaluate('document.getAnimations().forEach(a=>{if(a.effect?.getTiming().iterations!==Infinity)a.finish();})')
    page.clock.run_for(32)
    before = page.evaluate('window.calls.filter(c=>c.command!=="capture_liquid_backdrop").length')
    page.clock.run_for(1000)
    assert page.evaluate('window.calls.filter(c=>c.command!=="capture_liquid_backdrop").length') == before, page.evaluate('(n)=>window.calls.slice(n)', before)
    page.evaluate('window.rejectNext=true;document.querySelector(".notch").style.left="322px"')
    page.clock.run_for(64)
    assert page.locator('.notch-shell').get_attribute('data-liquid-status') == 'unavailable'
    assert page.locator('.liquid-layer').evaluate_all('(layers)=>layers.every(layer=>layer.hidden)')
    page.evaluate('window.renderer.setEnabled(false);window.renderer.setEnabled(true)')
    page.clock.run_for(64)
    assert page.locator('.notch-shell').get_attribute('data-liquid-status') == 'ready'
    page.evaluate('window.renderer.setVisible(false)')
    page.clock.run_for(32)
    assert page.locator('.liquid-layer').evaluate_all('(layers)=>layers.every(layer=>layer.hidden)')
    page.evaluate('window.renderer.setVisible(true)')
    page.clock.run_for(32)
    assert page.locator('.notch-shell').get_attribute('data-liquid-status') == 'ready'
    page.evaluate('window.renderer.destroy()')
    page.clock.run_for(32)
    assert page.locator('.liquid-layer').count() == 0
    assert not errors, errors
    commands = page.evaluate('window.calls')
    revisions = [call['args']['revision'] for call in commands if call['command'] in ['sync_liquid_backdrop', 'release_liquid_backdrop']]
    assert revisions == sorted(set(revisions)), revisions
    assert all(c['args']['width'] == 3 or c['args']['height'] == 3 for c in commands if c['command'] == 'capture_liquid_backdrop')
    result = {'physical_geometry':True,'delayed_ack_hidden_detail':True,'serialized_release_reopen':True,
              'theme_blur_update':True,'idle_geometry_ipc':0,'failure_recovery':True,'hide_resume_destroy':True,'revisions':revisions}
    (ROOT/'tmp/liquid-renderer-results.json').write_text(json.dumps(result,indent=2))
    print(json.dumps(result))
    browser.close()
