"""Ensure Liquid's blurred captured pixels cover the whole visible surface."""
import io
import os
from PIL import Image
from playwright.sync_api import sync_playwright

URL = os.environ.get('EWR_TEST_URL', 'http://127.0.0.1:1521') + '/liquid-edge-smoke'
POINTS = {'detail-left': (65, 150), 'detail-right': (314, 150), 'detail-top': (190, 65), 'detail-bottom': (190, 234), 'detail-center': (190, 150),
          'rail-left': (343, 135), 'rail-right': (376, 135), 'rail-top': (360, 13), 'rail-bottom': (360, 256), 'rail-center': (360, 135)}

with sync_playwright() as p:
    browser = p.chromium.launch(channel='msedge', headless=True)
    page = browser.new_page(viewport={'width': 400, 'height': 300}, device_scale_factor=1)
    page.route('**/liquid-edge-smoke', lambda r: r.fulfill(content_type='text/html', body='''
      <html><body style="margin:0"><div class="notch-shell" data-theme="liquid" style="--panel-opacity:1;--liquid-opacity:1;--detail-radius:16px">
        <nav class="notch" data-edge="right" style="left:340px;top:10px;width:40px;height:250px"><svg width="0" height="0"><defs><clipPath id="notch-finish-clip"><rect width="40" height="250"/></clipPath></defs></svg><div class="notch-finish"></div></nav>
        <section class="notch-detail" style="left:60px;top:60px;width:260px;height:180px;animation:none"></section>
      </div></body></html>'''))
    page.goto(URL)
    page.evaluate('''async()=>{
      await import('/src/styles/tokens.css');await import('/src/styles/notch.css');
      window.__TAURI_INTERNALS__={invoke:async(command,args)=>{
        if(command!=='capture_liquid_backdrop')return;
        if(args.surface===1 && window.holdDetail){
          await new Promise(resolve=>window.releaseDetail=resolve);
        }
        const {width,height}=args, bytes=new Uint8Array(12+width*height*4), view=new DataView(bytes.buffer);
        view.setUint32(0,0x4c513031,false);view.setUint32(4,width,true);view.setUint32(8,height,true);
        for(let i=12;i<bytes.length;i+=4){bytes[i]=100;bytes[i+1]=110;bytes[i+2]=120;bytes[i+3]=255;}
        return bytes.buffer;
      }};
      const {mountLiquidGlass}=await import('/src/ui/liquid-glass.ts');
      window.renderer=mountLiquidGlass(document.querySelector('.notch-shell'),document.querySelector('.notch'),document.querySelector('.notch-detail'));
      window.renderer.setEnabled(true);
    }''')
    page.wait_for_function('document.querySelector(".notch-shell").dataset.liquidStatus === "ready"')
    for opacity in [0, .5, 1]:
        page.evaluate('opacity=>document.querySelector(".notch-shell").style.setProperty("--liquid-opacity",opacity)',opacity)
        samples = []
        for color in ['rgb(240,20,20)', 'rgb(20,40,240)']:
            page.evaluate('color=>document.body.style.background=color',color)
            image=Image.open(io.BytesIO(page.screenshot(animations='disabled'))).convert('RGB')
            samples.append({name:image.getpixel(point) for name,point in POINTS.items()})
        differences={name:sum(abs(a-b) for a,b in zip(samples[0][name],samples[1][name])) for name in POINTS}
        assert all(value <= 3 for value in differences.values()), (opacity,differences)
        if opacity == 1:
            for name in ['detail-center', 'rail-center']:
                assert samples[0][name][0] >= 98, ('Liquid should not darken the captured surface',name,samples[0][name])
    # A new detail size must keep the last painted image until its capture arrives.
    page.evaluate('''()=>{window.holdDetail=true;document.querySelector('.notch-detail').style.height='250px';window.renderer.relayout()}''')
    page.wait_for_function('typeof window.releaseDetail === "function"')
    pending = page.evaluate('''()=>{
      const c=document.querySelector('.notch-detail canvas');
      return {size:[c.width,c.height],pixel:[...c.getContext('2d').getImageData(Math.floor(c.width/2),Math.floor(c.height/2),1,1).data]};
    }''')
    assert pending['pixel'][3] == 255, ('Detail image disappeared while capture was pending', pending)
    page.evaluate('''()=>{window.holdDetail=false;window.releaseDetail()}''')
    page.wait_for_function('document.querySelector(".notch-detail canvas").height > 360')
    page.evaluate('window.renderer.destroy()')
    print('PASS: Liquid captured pixels cover the center and four edges:',differences)
    browser.close()
