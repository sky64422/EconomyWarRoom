"""Check motion without changing native geometry; run Vite, then pass --port if needed."""
from pathlib import Path
import argparse, ast, json
from playwright.sync_api import sync_playwright, expect

ROOT=Path(__file__).resolve().parent.parent
parser=argparse.ArgumentParser();parser.add_argument('--port',type=int,default=1521)
args=parser.parse_args()
tree=ast.parse((ROOT/'scripts/notch-browser-smoke.py').read_text(encoding='utf-8'))
fixture=next(ast.literal_eval(n.value) for n in tree.body if isinstance(n,ast.Assign) and any(isinstance(t,ast.Name) and t.id=='FIXTURE' for t in n.targets))
results=[]
with sync_playwright() as p:
    browser=p.chromium.launch(channel='msedge',headless=True)
    for edge in ['right','left','top','bottom']:
        page=browser.new_page(viewport={'width':720,'height':800},reduced_motion='no-preference')
        errors=[];page.on('pageerror',lambda e:errors.append(str(e)))
        page.add_init_script('('+fixture+')();'+'''(()=>{
          const edge=EDGE,invoke=window.__TAURI_INTERNALS__.invoke;
          window.__TAURI_INTERNALS__.invoke=async(command,args)=>{
            const value=await invoke(command,args);
            if(command==='set_notch_surface'){
              value.edge=edge;
              if(edge==='left'){value.notch.x=0;if(value.detail)value.detail.x=64;}
              if(edge==='top'||edge==='bottom'){
                value.metrics.cell=96;value.notch={x:172,y:edge==='top'?0:744,width:376,height:56};
                if(value.detail){value.detail.x=230;value.detail.y=edge==='top'?64:736-value.detail.height;}
              }
            }
            return value;
          };
          document.addEventListener('animationstart',event=>{
            if(!event.target.matches('.notch-detail'))return;
            const animation=event.target.getAnimations().find(a=>a.animationName===event.animationName);
            if(animation){animation.pause();window.peekAnimation=animation;}
          },true);
        })();'''.replace('EDGE',json.dumps(edge)))
        page.goto(f'http://localhost:{args.port}')
        expect(page.locator('.notch-cell')).to_have_count(3)
        page.locator('.notch-cell[data-id="item-0"]').click()
        page.wait_for_function('window.peekAnimation && document.querySelector(".notch-detail").style.left!==""')
        samples=page.locator('.notch-detail').evaluate('''el=>{
          const samples=[];
          for(const time of [0,70,140,210,280]){
            window.peekAnimation.currentTime=time;
            const r=el.getBoundingClientRect(),s=getComputedStyle(el);
            samples.push({x:r.x,y:r.y,width:r.width,height:r.height,transform:s.transform,clip:s.clipPath,opacity:s.opacity});
          }
          return samples;
        }''')
        geometry=[(s['x'],s['y'],s['width'],s['height']) for s in samples]
        assert len(set(geometry))==1,(edge,samples)
        assert all(s['transform']=='none' for s in samples),(edge,samples)
        assert len({s['clip'] for s in samples})>1,(edge,samples)
        page.evaluate('window.peekAnimation.finish()')
        page.locator('.notch').click(button='right')
        page.locator('#tab-btn-general').click()
        tab_height=page.locator('#tab-btn-general').evaluate('el=>el.offsetHeight')
        assert tab_height==37,tab_height
        footer=page.locator('#btn-quit').bounding_box();body=page.locator('.settings-scroll').bounding_box()
        assert abs(footer['y']+footer['height']-body['y']-body['height'])<4,(footer,body)
        page.locator('#tab-btn-appearance').click()
        page.locator('label:has([data-pref="show_animation"])').click()
        expect(page.locator('.notch-shell')).to_have_class('notch-shell reduce-animation')
        assert page.locator('.settings-tabs').evaluate('el=>getComputedStyle(el,"::before").transitionDuration')=='0s'
        page.locator('label:has([data-pref="show_animation"])').click()
        expect(page.locator('.notch-shell')).to_have_class('notch-shell')
        page.emulate_media(reduced_motion='reduce')
        assert page.locator('.settings-tabs').evaluate('el=>getComputedStyle(el,"::before").transitionDuration')=='0s'
        assert page.locator('.notch-detail').evaluate('el=>getComputedStyle(el).animationName')=='none'
        assert not errors,errors
        results.append({'edge':edge,'fixed_geometry':geometry[0],'clip_reveal':True,'tab_height':tab_height,'footer_bottom':True,'motion_switch':True,'reduced_motion':True})
        page.close()
    browser.close()
(ROOT/'tmp').mkdir(exist_ok=True)
(ROOT/'tmp/notch-motion-result.json').write_text(json.dumps(results,indent=2),encoding='utf-8')
print(json.dumps(results))
