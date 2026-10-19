"""Local native acceptance probe; synthetic backdrop only, restores preferences."""
import ctypes as c
from ctypes import wintypes as w
import json
from pathlib import Path
from PIL import ImageGrab
from playwright.sync_api import sync_playwright

u=c.WinDLL('user32',use_last_error=True);g=c.WinDLL('gdi32',use_last_error=True)
Proc=c.WINFUNCTYPE(c.c_ssize_t,w.HWND,w.UINT,w.WPARAM,w.LPARAM)
class WC(c.Structure):
 _fields_=[('style',w.UINT),('proc',Proc),('ce',c.c_int),('we',c.c_int),('instance',w.HINSTANCE),('icon',w.HICON),('cursor',w.HANDLE),('brush',w.HBRUSH),('menu',w.LPCWSTR),('name',w.LPCWSTR)]
u.DefWindowProcW.argtypes=[w.HWND,w.UINT,w.WPARAM,w.LPARAM];u.DefWindowProcW.restype=c.c_ssize_t
u.RegisterClassW.argtypes=[c.POINTER(WC)]
u.CreateWindowExW.argtypes=[w.DWORD,w.LPCWSTR,w.LPCWSTR,w.DWORD,c.c_int,c.c_int,c.c_int,c.c_int,w.HWND,w.HMENU,w.HINSTANCE,c.c_void_p];u.CreateWindowExW.restype=w.HWND
u.SetWindowPos.argtypes=[w.HWND,w.HWND,c.c_int,c.c_int,c.c_int,c.c_int,w.UINT]
u.SetClassLongPtrW.argtypes=[w.HWND,c.c_int,c.c_ssize_t];u.SetClassLongPtrW.restype=c.c_ssize_t
u.GetWindowDisplayAffinity.argtypes=[w.HWND,c.POINTER(w.DWORD)]
u.GetWindowRect.argtypes=[w.HWND,c.POINTER(w.RECT)]
u.InvalidateRect.argtypes=[w.HWND,c.POINTER(w.RECT),w.BOOL]
u.UpdateWindow.argtypes=[w.HWND];u.DestroyWindow.argtypes=[w.HWND]
u.UnregisterClassW.argtypes=[w.LPCWSTR,w.HINSTANCE]
g.CreateSolidBrush.argtypes=[w.DWORD];g.CreateSolidBrush.restype=w.HBRUSH
g.DeleteObject.argtypes=[w.HANDLE]
@Proc
def wndproc(hwnd,msg,wp,lp):return u.DefWindowProcW(hwnd,msg,wp,lp)
colors=[(30,110,175),(225,235,245),(30,110,175)]
brushes=[g.CreateSolidBrush(r|(gg<<8)|(b<<16)) for r,gg,b in colors]
name='TokenUsageSyntheticBackdrop'
wc=WC(0,wndproc,0,0,None,None,None,brushes[0],None,name)
assert u.RegisterClassW(c.byref(wc))
plate=u.CreateWindowExW(0x08000080,name,'Synthetic backdrop',0x98000000,0,0,1,1,None,None,None,None)
assert plate
results=[]
with sync_playwright() as p:
 browser=p.chromium.connect_over_cdp('http://127.0.0.1:9251');page=browser.contexts[0].pages[0]
 assert page.evaluate('async()=>await window.__TAURI__.app.getIdentifier()')=='com.economywarroom.notch-preview'
 def invoke(name,args=None):return page.evaluate('async([n,a])=>window.__TAURI__.core.invoke(n,a)',[name,args or {}])
 original=invoke('get_state')['settings']
 def settle(n=50):
  msg=w.MSG()
  for step in range(0,n,10):
   while u.PeekMessageW(c.byref(msg),None,0,0,1):u.TranslateMessage(c.byref(msg));u.DispatchMessageW(c.byref(msg))
   page.evaluate('async n=>{for(let i=0;i<n;i++)await new Promise(requestAnimationFrame)}',min(10,n-step))
 def pin():
  if page.locator('#settings-close:visible').count():page.locator('#settings-close').dispatch_event('click')
  cell=page.locator('.notch-cell').first
  if cell.get_attribute('aria-pressed')!='true':cell.dispatch_event('click')
  page.wait_for_selector('.stock-identity strong')
 def theme(value):
  page.locator('.notch').dispatch_event('contextmenu');page.locator('#tab-btn-appearance').click()
  page.locator('[data-theme-option='+value+']').dispatch_event('click')
  page.wait_for_function('value=>document.querySelector(".notch-shell").dataset.theme===value',arg=value)
  page.locator('#settings-close').dispatch_event('click');pin();settle()
 try:
  for monitor in invoke('get_notch_monitors'):
   invoke('set_notch_placement',{'placement':{'edge':'right','monitor_hint':monitor['name'],'offset':.3}})
   settle();pin()
   for material in ['liquid']:
    theme(material)
    page.wait_for_function('()=>document.querySelector(".notch-shell").dataset.liquidStatus==="ready"')
    bounds=page.evaluate('()=>[screenX,screenY,innerWidth,innerHeight]')
    preview=[]
    @c.WINFUNCTYPE(w.BOOL,w.HWND,w.LPARAM)
    def above(hwnd,_):
     rc=w.RECT();u.GetWindowRect(hwnd,c.byref(rc))
     if int(hwnd)!=int(plate) and u.IsWindowVisible(hwnd) and [rc.left,rc.top,rc.right-rc.left,rc.bottom-rc.top]==bounds:preview.append(hwnd)
     return True
    u.EnumWindows(above,0)
    assert len(preview)==1
    affinity=w.DWORD();assert u.GetWindowDisplayAffinity(preview[0],c.byref(affinity));assert affinity.value==0,affinity.value
    assert u.SetWindowPos(plate,preview[0],*[round(v) for v in bounds],0x50)
    for color,brush in zip(colors,brushes):
     u.SetClassLongPtrW(plate,-10,int(brush));u.InvalidateRect(plate,None,True);u.UpdateWindow(plate)
     c.windll.dwmapi.DwmFlush();settle(160)
     modes=page.evaluate('()=>[document.querySelector(".notch").dataset.contrast,document.querySelector(".notch-detail").dataset.contrast]')
     if color[0]==225:assert modes==['light','light'],modes
     else:assert modes==['dark','dark'],modes
     rect=page.locator('.notch-detail').bounding_box()
     crop=(round(bounds[0]+rect['x']),round(bounds[1]+rect['y']),round(bounds[0]+rect['x']+rect['width']),round(bounds[1]+rect['y']+rect['height']))
     screen=ImageGrab.grab(bbox=crop,all_screens=True)
     mean=tuple(sum(p[i] for p in screen.getdata())/(screen.width*screen.height) for i in range(3))
     screen.save(f'tmp/live-{material}-{color[0]}.png')
     ImageGrab.grab(bbox=tuple(round(v) for v in [bounds[0],bounds[1],bounds[0]+bounds[2],bounds[1]+bounds[3]]),all_screens=True).save(f'tmp/clear-rail-{color[0]}.png')
     if color==colors[0]: previous=mean
     else: assert max(abs(a-b) for a,b in zip(previous,mean))>3,{'no_live_backdrop_change':True,'before':previous,'after':mean,'material':material}
     results.append({'monitor':monitor['name'],'material':material,'color':color,'screen_mean':mean})
    # Screenshot through the OS must include actual foreground text over the plate.
    metrics=page.evaluate('''()=>{const d=devicePixelRatio,r=document.querySelector('.notch-detail').getBoundingClientRect(),t=document.querySelector('.stock-identity strong').getBoundingClientRect();return {rect:[screenX+r.x*d,screenY+r.y*d,screenX+r.right*d,screenY+r.bottom*d],text:[screenX+t.x*d,screenY+t.y*d,screenX+t.right*d,screenY+t.bottom*d]}}''')
    image=ImageGrab.grab(bbox=tuple(round(v) for v in metrics['rect']),all_screens=True)
    suffix='bright-negative' if monitor['bounds']['x']<0 else 'bright-primary'
    image.save(f'tmp/capture-{material}-{suffix}-screen.png')
    label=ImageGrab.grab(bbox=tuple(round(v) for v in metrics['text']),all_screens=True)
    white=sum(min(pixel[:3])>140 for pixel in label.getdata())
    assert white>10,{'foreground_pixels':white,'material':material,'monitor':monitor['name']}
  Path('tmp/liquid-native-results.json').write_text(json.dumps(results,indent=2),'utf-8')
  print(json.dumps({'passed':len(results),'results':results}))
 finally:
  u.DestroyWindow(plate);u.UnregisterClassW(name,None)
  for brush in brushes:g.DeleteObject(brush)
  try:
   invoke('set_notch_placement',{'placement':original['notch']});theme(original['theme'])
  except Exception as error: print('restore:',str(error))
