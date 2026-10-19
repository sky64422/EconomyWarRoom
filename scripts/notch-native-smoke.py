"""Windows/WebView2 smoke. Requires the isolated com.economywarroom.notch-preview ID.
CDP 9251; optional patterned external window for Liquid. Never targets the installed app.
"""
import ctypes,json,time
from ctypes import wintypes
from pathlib import Path
import pyautogui
from playwright.sync_api import sync_playwright,expect

ROOT=Path(__file__).resolve().parent.parent
u=ctypes.windll.user32
u.GetForegroundWindow.restype=wintypes.HWND
u.FindWindowW.argtypes=[wintypes.LPCWSTR,wintypes.LPCWSTR];u.FindWindowW.restype=wintypes.HWND
u.WindowFromPoint.argtypes=[wintypes.POINT];u.WindowFromPoint.restype=wintypes.HWND
u.GetWindowDisplayAffinity.argtypes=[wintypes.HWND,ctypes.POINTER(wintypes.DWORD)]
u.GetWindowRect.argtypes=[wintypes.HWND,ctypes.POINTER(wintypes.RECT)]

with sync_playwright() as p:
    b=p.chromium.connect_over_cdp('http://localhost:9251');page=b.contexts[0].pages[0]
    assert page.evaluate('async()=>window.__TAURI__.app.getIdentifier()')=='com.economywarroom.notch-preview'
    errors=[];page.on('pageerror',lambda e:errors.append(str(e)))
    def invoke(command,args={}):return page.evaluate('async([c,a])=>window.__TAURI__.core.invoke(c,a)',[command,args])
    def reset():
        if page.locator('#settings-close').is_visible():page.locator('#settings-close').click()
        opened=page.locator('.notch-cell[data-detail-open="true"]')
        if opened.count():
            if opened.get_attribute('aria-pressed')!='true':opened.dispatch_event('click')
            opened.dispatch_event('click')
        page.evaluate('document.activeElement?.blur()')
    def place(edge,offset=.15):
        l=invoke('set_notch_placement',{'placement':{'edge':edge,'offset':offset,'monitor_hint':None}})
        page.wait_for_function('(edge)=>document.querySelector(".notch").dataset.edge===edge',arg=l['edge'])
        return l
    print('native connected',flush=True)
    state=invoke('get_state')
    if not any(i['symbol']=='NVDA' for i in state['watchlist']):invoke('add_symbol',{'symbol':'NVDA','asset_kind':'equity'})
    invoke('set_theme',{'theme':'dark'})
    invoke('set_notch_preferences',{'hoverDetail':True,'alwaysShowNotch':True,'showAnimation':True})
    page.reload();expect(page.locator('.notch-cell')).to_have_count(3)
    ids=[i['id'] for i in invoke('get_state')['watchlist']]
    def pin(index):page.locator('.notch-cell').nth(index).dispatch_event('click')
    matrices=[]
    for edge in ['right','left','top','bottom']:
        print('edge',edge,flush=True)
        reset();l=place(edge);base=l['window'];origin=l['notch']
        for i in [0,2,0,2,0]:
            pin(i);expect(page.locator('.stock-detail')).to_be_visible()
            now=invoke('set_notch_placement',{'placement':{'edge':edge,'offset':.15,'monitor_hint':None}})
            assert now['window']==base,(edge,'canvas moved',base,now['window'])
            assert now['notch']==origin,(edge,'notch moved')
        reset();page.locator('.notch').dispatch_event('contextmenu');expect(page.locator('.settings-root')).to_be_visible()
        heights=[]
        for tab in ['appearance','symbols','general','appearance']:
            page.locator('#tab-btn-'+tab).click()
            page.wait_for_function('document.querySelector(".notch-detail").getBoundingClientRect().height>300')
            heights.append(page.locator('.notch-detail').bounding_box()['height'])
        assert max(heights)-min(heights)<=1,(edge,heights)
        matrices.append({'edge':edge,'settings_heights':heights,'canvas':base})
        reset()
    print('focus and fold',flush=True)
    l=place('right');n=l['notch'];scale=l['scale'];m=l['metrics']
    backdrop=u.FindWindowW(None,'EconomyWarRoom native test backdrop')
    assert backdrop,'Start scripts/native-backdrop.py first'
    u.SetForegroundWindow(backdrop)
    pyautogui.moveTo(n['x']-200,n['y']+100,duration=.15)
    before=u.GetForegroundWindow()
    pyautogui.moveTo(n['x']+n['width']/2,n['y']+(m['inset']+m['cell']/2)*scale,duration=.2)
    expect(page.locator('.stock-detail')).to_be_visible();after=u.GetForegroundWindow()
    assert before==after,('hover focus',before,after)
    inside=u.WindowFromPoint(wintypes.POINT(int(n['x']+n['width']/2),int(n['y']+(m['inset']+m['cell']/2)*scale)))
    pyautogui.moveTo(n['x']+1,n['y']+1,duration=.15)
    deadline=time.monotonic()+2
    while time.monotonic()<deadline:
        outside=u.WindowFromPoint(wintypes.POINT(int(n['x']+1),int(n['y']+1)))
        if outside!=inside:break
    assert outside!=inside,'transparent corner intercepted pointer'
    reset();invoke('set_notch_preferences',{'hoverDetail':False,'alwaysShowNotch':False,'showAnimation':True})
    assert backdrop,'Start scripts/native-backdrop.py first'
    u.SetForegroundWindow(backdrop)
    pyautogui.moveTo(n['x']-200,n['y']+100,duration=.15)
    expect(page.locator('.notch')).to_have_class(__import__('re').compile(r'.*is-folded.*'),timeout=4000)
    # Edge tab is centred along the rail, not along its first stock cell.
    pyautogui.moveTo(n['x']+n['width']-3,n['y']+n['height']/2,duration=.15)
    expect(page.locator('.notch')).not_to_have_class(__import__('re').compile(r'.*is-folded.*'))
    invoke('set_notch_preferences',{'hoverDetail':False,'alwaysShowNotch':True,'showAnimation':True})
    reset();pin(0)
    print('liquid',flush=True)
    page.locator('.notch').dispatch_event('contextmenu')
    page.locator('#tab-btn-appearance').click();page.locator('[data-theme-option="liquid"]').click()
    page.wait_for_function('document.querySelector(".notch-shell").dataset.liquidStatus==="ready"',timeout=15000)
    page.locator('#settings-close').click();pin(2)
    page.screenshot(path=str(ROOT/'tmp/notch-native-liquid.png'),omit_background=True)
    page.wait_for_function('document.querySelector(".notch-shell").dataset.liquidStatus==="ready"')
    assert page.locator('.liquid-layer canvas').count()==0
    raw=page.evaluate("""async()=>{
      const r=document.querySelector('.notch-detail').getBoundingClientRect(),s=devicePixelRatio;
      const data=await window.__TAURI__.core.invoke('capture_liquid_backdrop',{x:Math.round(r.x*s),y:Math.round(r.y*s),width:Math.round(r.width*s),height:Math.round(r.height*s)});
      const a=new Uint8Array(data),colors=new Set();for(let i=54;i+3<a.length;i+=64)colors.add([a[i],a[i+1],a[i+2]].join(','));
      return {bytes:a.length,header:[a[0],a[1]],sample_colors:colors.size};
    }""")
    assert raw['header']==[66,77] and raw['sample_colors']>3,raw
    page.locator('.notch').dispatch_event('contextmenu');page.locator('[data-theme-option="glass"]').click()
    page.wait_for_function('document.querySelector(".notch-shell").dataset.theme==="glass"')
    page.locator('#settings-close').click();pin(0)
    page.screenshot(path=str(ROOT/'tmp/notch-native-glass.png'),omit_background=True)
    page.locator('.notch').dispatch_event('contextmenu');page.locator('[data-theme-option="dark"]').click()
    page.wait_for_function('document.querySelector(".notch-shell").dataset.theme==="dark"')
    rejected=page.evaluate("""async()=>{try{await window.__TAURI__.core.invoke('capture_liquid_backdrop',{x:0,y:0,width:10,height:10});return false;}catch{return true;}}""")
    assert rejected
    assert not errors,errors
    summary={'page_errors':errors,'native_matrix':matrices,'hover_focus_preserved':before==after,'transparent_click_through':inside!=outside,'fold_reveal':True,'liquid':{'compositor_material':True,'raw':raw,'dark_capture_rejected':rejected},'monitors':invoke('get_notch_monitors')}
    (ROOT/'tmp/native-smoke-result.json').write_text(json.dumps(summary,ensure_ascii=False,indent=2),encoding='utf-8')
    print(json.dumps(summary,ensure_ascii=True))
