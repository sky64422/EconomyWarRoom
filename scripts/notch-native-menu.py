"""Regression for mouse settings/add/remove, menu hit coverage and hide/show."""
from pathlib import Path
import json
import pyautogui
from playwright.sync_api import sync_playwright, expect

with sync_playwright() as p:
    browser=p.chromium.connect_over_cdp('http://localhost:9251')
    page=browser.contexts[0].pages[0]
    assert page.evaluate('async()=>window.__TAURI__.app.getIdentifier()')=='com.economywarroom.notch-preview'
    def invoke(command,args={}):
        return page.evaluate('async([command,args])=>window.__TAURI__.core.invoke(command,args)',[command,args])
    added=[]
    try:
        page.locator('.notch').dispatch_event('contextmenu')
        page.locator('#tab-btn-symbols').click()
        expect(page.locator('#add-symbol-input')).to_be_visible()
        page.locator('#add-symbol-input').fill('MSFT')
        page.locator('#add-form button').click()
        expect(page.locator('.management-row')).to_have_count(4)
        new_id=next(item['id'] for item in invoke('get_state')['watchlist'] if item['symbol']=='MSFT')
        added.append(new_id)
        page.locator('#settings-close').click()
        page.locator('.notch-cell[data-id="'+new_id+'"]').click()
        expect(page.locator('.stock-quote .row-symbol strong')).to_have_text('MSFT')
        page.locator('.notch').dispatch_event('contextmenu')
        page.locator('#tab-btn-symbols').click()
        page.locator('.management-row[data-id="'+new_id+'"] .management-remove').click()
        page.locator('#settings-close').click()
        expect(page.locator('.notch-detail')).to_be_hidden()
        assert len(invoke('get_state')['watchlist'])==3
        for symbol in ['MSFT','AMZN','GOOG','META','NFLX','TSLA','AMD']:
            added.append(invoke('add_symbol',{'symbol':symbol,'asset_kind':'equity'})['id'])
        page.locator('.notch-more').dispatch_event('click')
        expect(page.locator('.settings-root')).to_be_visible()
        expect(page.locator('.management-row')).to_have_count(10)
        last=page.locator('.management-row').last
        last.scroll_into_view_if_needed()
        last.click(button='right')
        menu=page.locator('.tint-menu').bounding_box()
        card=page.locator('.notch-detail').bounding_box()
        assert menu['y']+menu['height']<=card['y']+card['height']
        assert menu['x']+menu['width']<=card['x']+card['width']
        button=page.locator('.tint-menu [data-action="remove"]').bounding_box()
        pos=page.evaluate('async()=>window.__TAURI__.window.getCurrentWindow().outerPosition()')
        # Leave the CDP-injected cursor before checking the real Windows pointer.
        pyautogui.moveTo(pos['x']-100,button['y'],duration=.2)
        pyautogui.moveTo(pos['x']+button['x']+button['width']/2,pos['y']+button['y']+button['height']/2,duration=.2)
        page.wait_for_function('document.querySelector(".tint-menu [data-action=remove]")?.matches(":hover")')
        pyautogui.click()
        expect(page.locator('.management-row')).to_have_count(9)
        page.locator('.management-row').last.click(button='right')
        page.locator('#settings-close').click()
        expect(page.locator('.tint-menu')).to_have_count(0)
        expect(page.locator('.settings-root')).to_be_hidden()
        invoke('hide_widget')
        page.wait_for_function('async()=>!(await window.__TAURI__.window.getCurrentWindow().isVisible())')
        invoke('toggle_widget_visibility')
        page.wait_for_function('async()=>await window.__TAURI__.window.getCurrentWindow().isVisible()')
        result={'settings_search_visible':True,'settings_add_remove':True,'menu_inside_detail':True,'physical_menu_remove':True,'mouse_menu_cleanup':True,'hide_show':True}
        Path('tmp/native-menu-result.json').write_text(json.dumps(result,indent=2),encoding='utf-8')
        print(json.dumps(result))
    finally:
        existing={item['id'] for item in invoke('get_state')['watchlist']}
        for item_id in added:
            if item_id in existing:
                invoke('remove_symbol',{'id':item_id})
