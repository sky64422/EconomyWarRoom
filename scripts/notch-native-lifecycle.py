from pathlib import Path
import ctypes,json,time,re
from ctypes import wintypes
import pyautogui
from playwright.sync_api import sync_playwright,expect
u=ctypes.windll.user32;u.FindWindowW.restype=wintypes.HWND
u.GetWindowDisplayAffinity.argtypes=[wintypes.HWND,ctypes.POINTER(wintypes.DWORD)]
with sync_playwright() as p:
 b=p.chromium.connect_over_cdp('http://localhost:9251');page=b.contexts[0].pages[0]
 assert page.evaluate('async()=>window.__TAURI__.app.getIdentifier()')=='com.economywarroom.notch-preview'
 def invoke(c,a={}):return page.evaluate('async([c,a])=>window.__TAURI__.core.invoke(c,a)',[c,a])
 def reset():
  if page.locator('#settings-close').is_visible():page.locator('#settings-close').click()
  opened=page.locator('.notch-cell[data-detail-open="true"]')
  if opened.count():
   if opened.get_attribute('aria-pressed')!='true':opened.dispatch_event('click')
   opened.dispatch_event('click')
  page.evaluate('document.activeElement?.blur()')
 reset();invoke('set_notch_preferences',{'hoverDetail':False,'alwaysShowNotch':True,'showAnimation':True})
 monitors=invoke('get_notch_monitors');negative=next(m for m in monitors if m['bounds']['x']<0)
 l=invoke('set_notch_placement',{'placement':{'edge':'left','offset':.2,'monitor_hint':negative['name']}})
 page.locator('.notch-cell').first.dispatch_event('click');expect(page.locator('.stock-detail')).to_be_visible()
 page.locator('.notch').dispatch_event('contextmenu');heights=[]
 for tab in ['appearance','symbols','general','appearance']:
  page.locator('#tab-btn-'+tab).click();heights.append(page.locator('.notch-detail').bounding_box()['height'])
 assert max(heights)-min(heights)<=1,heights
 reset();l=invoke('set_notch_placement',{'placement':{'edge':'right','offset':.2,'monitor_hint':None}})
 n=l['notch'];m=l['metrics'];s=l['scale'];x=n['x']+n['width']/2;y=n['y']+(m['inset']+m['cell']/2)*s
 pyautogui.moveTo(x,y,duration=.2);pyautogui.mouseDown();pyautogui.moveTo(x,y+100,duration=.4);pyautogui.mouseUp()
 page.wait_for_function('''async()=>Math.abs((await window.__TAURI__.core.invoke('get_state')).settings.notch.offset-.2)>.02''')
 moved=invoke('get_state')['settings']['notch'];assert moved['edge']=='right',moved
 reset();l=invoke('set_notch_placement',{'placement':moved});n=l['notch'];x=n['x']+n['width']/2;y=n['y']+(m['inset']+m['cell']/2)*s
 pyautogui.moveTo(x,y,duration=.2);pyautogui.mouseDown();pyautogui.moveTo(x,y+100,duration=.4);pyautogui.mouseUp()
 page.wait_for_function('!document.querySelector(".notch").classList.contains("is-dragging")')
 page.wait_for_function('async(offset)=>Math.abs((await window.__TAURI__.core.invoke("get_state")).settings.notch.offset-offset)>.02',arg=moved['offset'])
 assert invoke('get_state')['settings']['notch']['offset']!=moved['offset']
 reset();invoke('set_notch_preferences',{'hoverDetail':False,'alwaysShowNotch':False,'showAnimation':True})
 backdrop=u.FindWindowW(None,'EconomyWarRoom native test backdrop');assert backdrop,'Start scripts/native-backdrop.py first';u.SetForegroundWindow(backdrop)
 pyautogui.moveTo(800,800,duration=.2)
 expect(page.locator('.notch')).to_have_class(re.compile('.*is-folded.*'),timeout=4000)
 folded=invoke('set_notch_placement',{'placement':invoke('get_state')['settings']['notch']})
 n=folded['notch'];pyautogui.moveTo(n['x']+n['width']-2,n['y']+n['height']/2,duration=.2)
 expect(page.locator('.notch')).not_to_have_class(re.compile('.*is-folded.*'))
 invoke('set_notch_preferences',{'hoverDetail':False,'alwaysShowNotch':True,'showAnimation':True})
 reset();page.locator('.notch').dispatch_event('contextmenu');page.locator('[data-theme-option="liquid"]').click()
 page.wait_for_function('document.querySelector(".notch-shell").dataset.liquidStatus==="ready"')
 hwnd=u.FindWindowW(None,'Economy War Room Preview');aff=wintypes.DWORD();assert hwnd
 assert u.GetWindowDisplayAffinity(hwnd,ctypes.byref(aff));assert aff.value==17,aff.value
 invoke('hide_widget');page.wait_for_function('async()=>!(await window.__TAURI__.window.getCurrentWindow().isVisible())')
 deadline=time.monotonic()+3
 while time.monotonic()<deadline:
  u.GetWindowDisplayAffinity(hwnd,ctypes.byref(aff))
  if aff.value==0:break
 assert aff.value==0,'hide retained capture affinity'
 assert invoke('toggle_widget_visibility') is True
 page.wait_for_function('async()=>await window.__TAURI__.window.getCurrentWindow().isVisible()');page.wait_for_function('document.querySelector(".notch-shell").dataset.liquidStatus==="ready"')
 page.locator('[data-theme-option="dark"]').click()
 deadline=time.monotonic()+3
 while time.monotonic()<deadline:
  u.GetWindowDisplayAffinity(hwnd,ctypes.byref(aff))
  if aff.value==0:break
 assert aff.value==0,'theme exit retained capture affinity'
 result={'negative_monitor':negative['name'],'negative_monitor_settings_heights':heights,'physical_drag_saved':moved,'second_mouse_drag_saved':True,'pointer_reveal':True,'liquid_affinity':17,'hide_and_theme_exit_affinity':aff.value,'hide_show':True}
 Path('tmp/native-extra-result.json').write_text(json.dumps(result,indent=2),encoding='utf-8');print(json.dumps(result))
