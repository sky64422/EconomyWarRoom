"""Full frontend regression with synthetic quotes and mocked native geometry.
Run Vite on 1521, then python scripts/notch-browser-smoke.py.
Native focus, capture and click-through require separate Windows smoke.
"""
from pathlib import Path
import json
from playwright.sync_api import sync_playwright, expect

ROOT=Path(__file__).resolve().parent.parent
FIXTURE=r"""() => {
  const symbols=['AAPL','BTC-USD','NVDA'];
  window.fixture={watchlist:symbols.map((symbol,i)=>({id:'item-'+i,symbol,display_name:['Apple','Bitcoin','NVIDIA'][i],asset_kind:i===1?'crypto':'equity',sort_index:i,card_tint:'none'})),settings:{theme:'dark',opacity:.92,autostart:false,hover_detail:false,always_show_notch:true,show_animation:true,quote_refresh_secs:1000,window:{x:80,y:80,width:320,height:640},notch:{edge:'right',offset:.5,monitor_hint:'mock'},column_ratios:{symbol:1.15,spark:1.25,metrics:2}}};
  window.fixtureQuotes=symbols.map((symbol,i)=>({symbol,price:[212.35,87432.56,145.78][i],currency:'USD',change_percent:[1.24,3.12,-2.36][i],regular_change_percent:-.9,as_of:'2026-10-16T18:00:00Z',source:'smoke-fixture',market_state:i===0?'pre':'regular',display:{primary:{price:[212.35,87432.56,145.78][i],change:[1.24,3.12,-2.36][i]},secondary:{price:[209.75,84787,149.3][i],change:[-.36,-.23,.83][i]}},previous_close:200,sparkline_change_percent:-.9}));
  window.fixtureSparks=symbols.map(symbol=>({symbol,as_of:'fixture',previous_close:200,session_start:1760621400,session_end:1760644800,points:[{t:1760621400,close:201},{t:1760625000,close:204},{t:1760628600,close:200},{t:1760632200,close:202}]}));
  window.callbacks={};window.events={};window.calls=[];let nextId=0;
  window.emitFixture=(event,payload)=>{for(const callback of window.events[event]??[])callback({event,payload,id:1});};
  window.__TAURI_INTERNALS__={metadata:{currentWindow:{label:'main'},currentWebview:{label:'main'}},
    transformCallback:callback=>{const id=++nextId;window.callbacks[id]=callback;return id;},unregisterCallback:id=>delete window.callbacks[id],
    invoke:async(command,args={})=>{
      window.calls.push([command,args]);
      if(command==='get_state')return structuredClone(window.fixture);
      if(command==='get_quotes'){
        if(window.delayInitialQuotes)await new Promise(resolve=>window.releaseInitialQuotes=resolve);
        return structuredClone(window.fixtureQuotes);
      }
      if(command==='get_sparklines')return structuredClone(window.fixtureSparks);
      if(command==='get_notch_reveal')return window.initialReveal??true;
      if(command==='set_notch_focus')return true;
      if(command==='plugin:app|version')return '0.1.54';
      if(command==='plugin:event|listen'){(window.events[args.event]??=[]).push(window.callbacks[args.handler]);return nextId;}
      if(command==='set_notch_surface'){
        if(window.delaySurface)await new Promise(resolve=>{window.releaseSurface=resolve;});
        const n=Math.max(1,Math.min(6,window.fixture.watchlist.length));
        return {edge:'right',anchor_x:'left',anchor_y:'top',scale:1,monitor:'mock',window:{x:0,y:0,width:720,height:800},notch:{x:664,y:130,width:56,height:n*72+88},detail:args.expanded?{x:396,y:80,width:260,height:Math.min(560,args.height)}:null,metrics:{slot_count:n,content_scale:1,depth:56,cell:72,shoulder:28,inner_radius:28,inset:44,detail_radius:16,drag_threshold:5,rest_depth:10,rest_length:80}};
      }
      if(command==='set_theme'){if(window.rejectTheme)throw Error('test save failed');window.fixture.settings.theme=args.theme;return;}
      if(command==='finish_notch_drag'){window.emitFixture('notch-drag',{id:args.id,active:false,finished:true,placement:window.fixture.settings.notch,error:null});return;}
      if(command==='set_column_ratios')return args.ratios;
      if(command==='add_symbol'){
        if(window.rejectAdd)throw Error('test add failed');
        if(window.delayAdd)await new Promise(resolve=>{window.releaseAdd=resolve;});
        const item={id:'added',symbol:args.symbol,display_name:args.symbol,asset_kind:args.asset_kind,sort_index:window.fixture.watchlist.length,card_tint:'none'};
        window.fixture.watchlist.push(item);window.emitFixture('watchlist-updated',window.fixture.watchlist);return item;
      }
      if(command==='reorder_symbols'){
        const byId=new Map(window.fixture.watchlist.map(item=>[item.id,item]));
        window.fixture.watchlist=args.ordered_ids.map((id,sort_index)=>({...byId.get(id),sort_index}));
        window.emitFixture('watchlist-updated',window.fixture.watchlist);return;
      }
      if(command==='remove_symbol'){
        if(window.rejectRemove)throw Error('test remove failed');
        if(window.delayRemove)await new Promise(resolve=>{window.releaseRemove=resolve;});
        window.fixture.watchlist=window.fixture.watchlist.filter(item=>item.id!==args.id);
        window.emitFixture('watchlist-updated',window.fixture.watchlist);return;
      }
      if(command==='set_quote_refresh_ms'){if(window.rejectRefresh)throw Error('test refresh failed');return args.ms;}
      if(command==='set_autostart'){if(window.rejectAutostart)throw Error('test autostart failed');return;}
      if(command==='check_for_updates'){
        if(window.rejectUpdate)throw Error('test update failed');
        if(window.delayUpdate)await new Promise(resolve=>{window.releaseUpdate=resolve;});
        return false;
      }
      if(command==='get_diagnostics')return 'test diagnostics';
      if(command==='capture_liquid_backdrop')throw Error('browser has no native capture');
      return null;
    }
  };
}"""

with sync_playwright() as p:
    browser=p.chromium.launch(channel='msedge',headless=True)
    page=browser.new_page(viewport={'width':720,'height':800},reduced_motion='reduce')
    errors=[]
    page.on('pageerror',lambda e:errors.append(str(e)))
    page.add_init_script('('+FIXTURE+')()')
    page.on('console',lambda msg: print('CONSOLE',msg.text) if msg.type=='error' else None)
    page.goto('http://localhost:1521')
    page.wait_for_function("document.querySelector('.notch-cell') || document.body.innerText.includes('Failed to load')")
    print('MOUNT',errors,page.locator('body').inner_text()[:300])
    expect(page.locator('.notch-cell')).to_have_count(3)
    expect(page.locator('.notch-add')).to_have_count(0)
    expect(page.locator('.notch-remove')).to_have_count(0)
    icon_offsets=page.locator('.notch-cell').evaluate_all('els=>els.map(el=>el.querySelector(".ring-wrap").getBoundingClientRect().top-el.getBoundingClientRect().top)')
    assert max(icon_offsets)-min(icon_offsets)<.1,icon_offsets
    ring_colors=[page.locator('.notch-cell[data-id="'+item_id+'"]').evaluate('el=>getComputedStyle(el.querySelector(".ring-track")).stroke') for item_id in ['item-0','item-2']]
    assert ring_colors[0]==ring_colors[1],ring_colors
    page.locator('.notch').click(button='right')
    page.locator('#tab-btn-symbols').click()
    expect(page.locator('#tab-panel-symbols input').first).to_be_visible()
    expect(page.locator('.management-mark')).to_have_count(3)
    expect(page.locator('.management-count, #add-cancel, .management-open')).to_have_count(0)
    expect(page.locator('.management-remove')).to_have_count(3)
    input_box=page.locator('#add-symbol-input').bounding_box()
    assert input_box['width']>=150,input_box
    assert input_box['y']<page.locator('.management-row').first.bounding_box()['y']
    card_box=page.locator('.notch-detail').bounding_box()
    search_box=page.locator('#add-form').bounding_box()
    assert abs(search_box['x']+search_box['width']/2-card_box['x']-card_box['width']/2)<1,(search_box,card_box)
    page.locator('#add-symbol-input').fill('TSLA')
    expect(page.locator('.add-suggest-item[data-symbol="TSLA"]')).to_have_count(1)
    page.keyboard.press('Escape')
    expect(page.locator('.settings-root')).to_be_visible()
    expect(page.locator('#add-symbol-input')).to_have_value('TSLA')
    page.keyboard.press('ArrowDown')
    page.keyboard.press('Enter')
    assert not page.evaluate('window.calls.some(([command])=>command==="add_symbol")')
    page.locator('#add-symbol-input').fill('MSFT')
    page.evaluate('window.rejectAdd=true')
    page.locator('#add-form button').click()
    expect(page.locator('.add-error')).to_contain_text('test add failed')
    expect(page.locator('#add-symbol-input')).to_have_value('MSFT')
    expect(page.locator('#add-form button')).to_be_enabled()
    page.evaluate('window.rejectAdd=false;window.delayAdd=true')
    page.locator('#add-form button').click()
    expect(page.locator('#add-form')).to_have_attribute('aria-busy','true')
    expect(page.locator('#add-form button')).to_be_disabled()
    page.locator('#add-form button').dispatch_event('click')
    assert page.evaluate('window.calls.filter(([command])=>command==="add_symbol").length')==2
    page.evaluate('window.delayAdd=false;window.releaseAdd()')
    expect(page.locator('.management-row')).to_have_count(4)
    # Cross several rows with real transitions. Exclude the cloned body-level ghost.
    page.emulate_media(reduced_motion='no-preference')
    page.wait_for_function('document.querySelector(".notch-detail").getAnimations().every(a=>a.playState!=="running")')
    rows=page.locator('.watchlist-rows .management-row')
    boxes=rows.evaluate_all('els=>els.map(el=>el.getBoundingClientRect().toJSON())')
    x=boxes[0]['x']+14
    before_reorder=page.evaluate('window.calls.filter(([command])=>command==="reorder_symbols").length')
    page.mouse.move(x,boxes[0]['y']+boxes[0]['height']/2);page.mouse.down()
    for box in boxes[1:]:
        page.mouse.move(x,box['y']+box['height']-2,steps=5)
        page.wait_for_function('Array.from(document.querySelectorAll(".watchlist-rows .management-row")).every(el=>el.getAnimations().every(a=>a.playState!=="running"))')
    page.mouse.up()
    expect(page.locator('.drag-ghost')).to_have_count(0)
    expect(rows.last).to_have_attribute('data-id','item-0')
    assert page.evaluate('window.calls.filter(([command])=>command==="reorder_symbols").length')==before_reorder+1
    boxes=rows.evaluate_all('els=>els.map(el=>el.getBoundingClientRect().toJSON())')
    page.mouse.move(x,boxes[-1]['y']+boxes[-1]['height']/2);page.mouse.down()
    for box in reversed(boxes[:-1]):
        page.mouse.move(x,box['y']+2,steps=5)
        page.wait_for_function('Array.from(document.querySelectorAll(".watchlist-rows .management-row")).every(el=>el.getAnimations().every(a=>a.playState!=="running"))')
    page.mouse.up()
    expect(page.locator('.drag-ghost')).to_have_count(0)
    expect(rows.first).to_have_attribute('data-id','item-0')
    assert page.evaluate('window.calls.filter(([command])=>command==="reorder_symbols").length')==before_reorder+2
    page.emulate_media(reduced_motion='reduce')
    page.locator('.management-row[data-id="item-1"]').click(button='right')
    page.locator('#tab-btn-appearance').click()
    page.keyboard.press('Delete')
    assert page.evaluate('window.fixture.watchlist.length')==4
    page.locator('#tab-btn-symbols').click()
    page.locator('.management-row[data-id="item-1"]').click(button='right')
    page.keyboard.press('Delete')
    assert page.evaluate('window.fixture.watchlist.length')==4
    page.keyboard.press('Escape')
    expect(page.locator('.settings-root')).to_be_visible()
    page.locator('#settings-close').click()
    page.locator('.notch-cell[data-id="added"]').click()
    expect(page.locator('.stock-quote .row-symbol strong')).to_have_text('MSFT')
    expect(page.locator('.stock-heading, .stock-meta, .stock-detail .watchlist-card')).to_have_count(0)
    expect(page.locator('.stock-quote .row-symbol .stock-symbol-mark')).to_have_count(1)
    expect(page.locator('.stock-remove')).to_have_count(0)
    page.locator('.notch').click(button='right')
    page.locator('#tab-btn-symbols').click()
    expect(page.locator('.management-row[data-id="added"] .management-remove')).to_have_attribute('aria-label','MSFT 관심종목에서 제거')
    drag_count=page.evaluate('window.calls.filter(([command])=>command==="begin_notch_drag").length')
    page.evaluate('window.delayRemove=true')
    page.locator('.management-row[data-id="added"] .management-remove').click()
    expect(page.locator('.management-row[data-id="added"] .management-remove')).to_be_disabled()
    remove_count=page.evaluate('window.calls.filter(([command])=>command==="remove_symbol").length')
    page.locator('.management-row[data-id="added"] .management-remove').dispatch_event('click')
    assert page.evaluate('window.calls.filter(([command])=>command==="remove_symbol").length')==remove_count
    page.evaluate('window.delayRemove=false;window.releaseRemove()')
    assert page.evaluate('window.calls.filter(([command])=>command==="begin_notch_drag").length')==drag_count
    expect(page.locator('.settings-root')).to_be_visible()
    expect(page.locator('.management-row')).to_have_count(3)
    page.locator('#settings-close').click()
    expect(page.locator('.notch-detail')).to_be_hidden()
    expect(page.locator('.notch-cell')).to_have_count(3)
    expect(page.locator('[data-id="item-0"] .ring-pct')).to_have_text('+1.24%')
    page.locator('.notch-cell[data-id="item-0"]').click()
    expect(page.locator('.stock-detail')).to_be_visible()
    expect(page.locator('.stock-detail [data-price-primary]')).to_have_text('212.35')
    expect(page.locator('.stock-detail [data-change-primary]')).to_have_text('(+1.24%)')
    expect(page.locator('.stock-detail [data-change-secondary]')).to_have_text('(-0.36%)')
    assert page.locator('.stock-detail .row-sparkline path').count()>0
    page.wait_for_function('document.querySelector(".stock-quote .row-sparkline").getBoundingClientRect().width>170')
    assert page.locator('.stock-detail .row-col-resize').count()==0
    chart=page.locator('.stock-quote .row-sparkline').bounding_box()
    quote=page.locator('.stock-quote .row-metrics').bounding_box()
    assert chart['y']>=quote['y']+quote['height']
    assert page.locator('.notch-detail').bounding_box()['width']==260
    before=page.locator('.notch').bounding_box()
    page.locator('.notch-cell[data-id="item-2"]').click()
    expect(page.locator('.stock-quote .row-symbol strong')).to_have_text('NVDA')
    page.wait_for_function('window.calls.some(([command,args])=>command==="set_notch_surface" && args.target===224)')
    assert page.locator('.notch').bounding_box()==before
    page.evaluate('window.delaySurface=true')
    page.locator('.notch').click(button='right')
    page.wait_for_function('typeof window.releaseSurface === "function"')
    page.evaluate('new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)))')
    expect(page.locator('.settings-root')).to_be_hidden()
    page.evaluate('window.delaySurface=false;window.releaseSurface()')
    expect(page.locator('.settings-root')).to_be_visible()
    page.wait_for_function("document.querySelector('.notch-detail').getBoundingClientRect().height>300")
    heights=[]
    for tab in ['appearance','symbols','general','appearance']:
        page.locator('#tab-btn-'+tab).click()
        page.wait_for_function('document.querySelector(".settings-root").getBoundingClientRect().height>0')
        heights.append(page.locator('.notch-detail').bounding_box()['height'])
        scroll=page.locator('.settings-scroll').evaluate('el=>({height:el.clientHeight,content:el.scrollHeight,width:el.clientWidth,outer:el.offsetWidth,bar:getComputedStyle(el).scrollbarWidth})')
        assert scroll['content']<=scroll['height']+1,scroll
        assert scroll['width']==scroll['outer'] and scroll['bar']=='none',scroll
    assert max(heights)-min(heights)<=1,heights
    expect(page.locator('.settings-tabs .tab-icon')).to_have_count(3)
    page.locator('#tab-btn-appearance').focus()
    page.keyboard.press('ArrowRight')
    expect(page.locator('#tab-panel-appearance')).to_be_visible()
    page.locator('#tab-btn-general').click()
    assert page.locator('[data-refresh]').all_text_contents()==['1s','10s','60s']
    page.evaluate('window.rejectRefresh=true')
    page.locator('[data-refresh="10000"]').click()
    expect(page.locator('[data-refresh="1000"]')).to_have_attribute('aria-pressed','true')
    expect(page.locator('#settings-live')).to_be_empty()
    page.evaluate('window.rejectRefresh=false')
    page.locator('[data-refresh="60000"]').click()
    expect(page.locator('[data-refresh="60000"]')).to_have_attribute('aria-pressed','true')
    assert page.evaluate('window.calls.filter(([command])=>command==="set_quote_refresh_ms").at(-1)[1].ms')==60000
    page.locator('[data-refresh="1000"]').click()
    footer=page.locator('#btn-quit').bounding_box()
    body=page.locator('.settings-scroll').bounding_box()
    assert abs(body['y']+body['height']-footer['y']-footer['height'])<4,(body,footer)
    page.evaluate('window.rejectAutostart=true')
    page.locator('label[for="autostart-toggle"]').click()
    expect(page.locator('#autostart-toggle')).not_to_be_checked()
    expect(page.locator('#settings-live')).to_be_empty()
    page.evaluate('window.rejectAutostart=false;window.delayUpdate=true')
    page.locator('#btn-check-update').click()
    expect(page.locator('#btn-check-update')).to_be_disabled()
    expect(page.locator('#btn-check-update')).to_have_attribute('data-feedback','loading')
    page.evaluate('window.delayUpdate=false;window.releaseUpdate()')
    expect(page.locator('#btn-check-update')).to_have_attribute('data-feedback','done')
    expect(page.locator('#btn-check-update .update-icon-reload')).to_have_css('opacity','0')
    expect(page.locator('#update-status')).to_have_text('최신 버전입니다')
    page.evaluate('window.rejectUpdate=true')
    page.locator('#btn-check-update').click()
    expect(page.locator('#update-status')).to_contain_text('test update failed')
    expect(page.locator('#btn-check-update')).to_be_enabled()
    page.evaluate('window.rejectUpdate=false')
    page.locator('#btn-check-update').click()
    expect(page.locator('#btn-check-update')).to_have_attribute('data-feedback','done')
    page.evaluate('Object.defineProperty(navigator,"clipboard",{configurable:true,value:{writeText:async text=>{window.copiedText=text;}}})')
    page.locator('#btn-diag').click()
    expect(page.locator('#btn-diag')).to_have_attribute('data-feedback','done')
    expect(page.locator('#btn-diag .diag-copy-icon')).to_have_css('opacity','0')
    assert page.evaluate('window.copiedText')=='test diagnostics'
    assert not page.evaluate('window.calls.some(([command])=>command==="quit_app")')
    page.locator('#tab-btn-appearance').click()
    page.evaluate('window.rejectTheme=true')
    page.locator('[data-theme-option="glass"]').click()
    expect(page.locator('[data-theme-option="dark"]')).to_have_attribute('aria-pressed','true')
    expect(page.locator('.settings-save-status')).to_have_count(0)
    page.evaluate('window.rejectTheme=false')
    page.locator('[data-theme-option="glass"]').click()
    expect(page.locator('.notch-shell')).to_have_attribute('data-theme','glass')
    page.locator('#settings-close').click()
    page.locator('.notch-cell[data-id="item-0"]').click()
    (ROOT/'tmp').mkdir(exist_ok=True)
    page.screenshot(path=str(ROOT/'tmp/notch-browser-glass.png'))
    page.keyboard.press('Escape')
    expect(page.locator('.stock-detail')).to_be_visible()
    page.locator('.notch-cell[data-id="item-0"]').click()
    page.evaluate("""() => {
      const items=Array.from({length:10},(_,i)=>({id:'many-'+i,symbol:'T'+i,display_name:'Ticker '+i,asset_kind:'equity',sort_index:i,card_tint:'none'}));
      window.fixture.watchlist=items;window.emitFixture('watchlist-updated',items);
    }""")
    expect(page.locator('.notch-cell')).to_have_count(6)
    expect(page.locator('.notch-more .ring-pct')).to_have_text('+5')
    page.locator('.notch-more').click()
    expect(page.locator('#tab-panel-symbols')).to_be_visible()
    expect(page.locator('.management-row')).to_have_count(10)
    page.locator('.management-row').last.scroll_into_view_if_needed()
    page.locator('.management-row').last.click(button='right')
    menu=page.locator('.tint-menu').bounding_box()
    card=page.locator('.notch-detail').bounding_box()
    assert menu['y']+menu['height']<=card['y']+card['height'],(menu,card)
    page.keyboard.press('Escape')
    expect(page.locator('.tint-menu')).to_have_count(1)
    page.locator('#tab-btn-appearance').click()
    expect(page.locator('.tint-menu')).to_have_count(0)
    page.locator('#tab-btn-symbols').click()
    page.locator('.management-row').last.click(position={'x':70,'y':20})
    expect(page.locator('.stock-quote .row-symbol strong')).to_have_text('T9')
    page.locator('.notch-cell[data-id="many-0"]').click()
    page.evaluate('window.rejectRemove=true')
    page.locator('.notch').click(button='right')
    page.locator('#tab-btn-symbols').click()
    page.locator('.management-row[data-id="many-0"] .management-remove').click()
    expect(page.locator('.settings-root')).to_be_visible()
    expect(page.locator('.management-row[data-id="many-0"] .management-remove')).to_be_enabled()
    expect(page.locator('.management-row[data-id="many-0"] .management-remove-error')).to_contain_text('제거 실패')
    page.evaluate('window.rejectRemove=false')
    page.locator('.management-row[data-id="many-0"] .management-remove').click()
    expect(page.locator('.management-row')).to_have_count(9)
    page.locator('#settings-close').click()
    expect(page.locator('.notch-detail')).to_be_hidden()
    assert page.evaluate('window.fixture.watchlist.some(item=>item.id==="many-0")') is False
    page.evaluate("window.fixture.watchlist=[];window.emitFixture('watchlist-updated',[])")
    expect(page.locator('.notch-detail')).to_be_hidden()
    expect(page.locator('.notch-cell')).to_have_count(0)
    page.locator('.notch').click(button='right')
    page.locator('#tab-btn-symbols').click()
    expect(page.locator('.management-empty')).to_be_visible()
    expect(page.locator('#add-symbol-input')).to_be_visible()
    assert not errors,errors
    legacy=browser.new_page(viewport={'width':720,'height':800},reduced_motion='reduce')
    legacy.add_init_script('('+FIXTURE+')();window.fixture.settings.quote_refresh_secs=250;')
    legacy.goto('http://localhost:1521')
    legacy.locator('.notch').click(button='right')
    legacy.locator('#tab-btn-general').click()
    expect(legacy.locator('[data-refresh][aria-pressed="true"]')).to_have_count(0)
    assert not legacy.evaluate('window.calls.some(([command])=>command==="set_quote_refresh_ms")')
    legacy.locator('[data-refresh="1000"]').click()
    expect(legacy.locator('[data-refresh="1000"]')).to_have_attribute('aria-pressed','true')
    legacy.close()
    # A slow initial quote read must not overwrite a newer native fold/reveal event.
    for initial, latest in [(True, False), (False, True)]:
        startup=browser.new_page(viewport={'width':720,'height':800},reduced_motion='reduce')
        startup.add_init_script('('+FIXTURE+')();window.delayInitialQuotes=true;window.initialReveal='+json.dumps(initial)+';')
        startup.goto('http://localhost:1521')
        startup.wait_for_function('typeof window.releaseInitialQuotes==="function" && window.calls.some(([c])=>c==="get_notch_reveal")')
        startup.evaluate('value=>window.emitFixture("notch-reveal",value)',latest)
        expect(startup.locator('.notch')).to_have_attribute('aria-expanded',str(latest).lower())
        startup.evaluate('window.releaseInitialQuotes()')
        expect(startup.locator('.notch-cell .ring-pct').first).to_have_text('+1.24%')
        expect(startup.locator('.notch')).to_have_attribute('aria-expanded',str(latest).lower())
        assert startup.locator('.notch-cells').evaluate('(el)=>el.inert') is not latest
        startup.close()
    print(json.dumps({'page_errors':errors,'settings_heights':heights,'checks':['primary equality','secondary preserved','sparkline','fixed origin','settings shared height/no scrollbars/centered body','add failure/retry and duplicate prevention','mouse reorder','keyboard shortcuts inactive','settings rollback','update/copy feedback','no notch add/remove','overflow access','settings removal failure/retry','empty list settings access','native fold/reveal wins delayed startup snapshot']},ensure_ascii=False))
    browser.close()
