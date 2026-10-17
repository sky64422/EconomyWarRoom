"""Keep unchanged display nodes; still render corrected data and activity changes."""
from pathlib import Path
import argparse, json
from playwright.sync_api import sync_playwright

ROOT = Path(__file__).resolve().parent.parent
parser = argparse.ArgumentParser()
parser.add_argument('--port', type=int, default=1523)
parser.add_argument('--token', action='store_true')
args = parser.parse_args()
with sync_playwright() as p:
    browser = p.chromium.launch(channel='msedge', headless=True)
    page = browser.new_page()
    page.clock.install()
    page.route('**/src/main.ts', lambda r: r.fulfill(content_type='text/javascript', body=''))
    page.goto(f'http://localhost:{args.port}')
    result = page.evaluate('''async token => {
      document.body.innerHTML='<nav id="rail"></nav><section id="detail"></section>';
      const rail=document.querySelector('#rail'),detail=document.querySelector('#detail');
      const {mountNotch}=await import('/src/ui/notch.ts');
      const notch=mountNotch(rail,{hover:()=>{},pin:()=>{},more:()=>{}});
      let mutations=0;
      const observe=new MutationObserver(records=>mutations+=records.length);
      let stable,updated;
      if(token){
        const {mountProviders}=await import('/src/ui/providers.ts');
        const providers=mountProviders(detail);
        const snap={provider_id:'claude',display_name:'Claude',windows:[{kind:'rolling_5h',used:25,limit:100,unit:'percent',resets_at:null,used_percent:25,label:'5h'}],status:'ok',source:'vendor',as_of:'first',message:null,primary_resets_at:null,primary_used_percent:25};
        const update=()=>{notch.update([snap],['claude']);providers.setSnapshots([structuredClone(snap)]);providers.setActivities([{provider_id:'claude',state:'idle'}]);};
        update();const original=detail.firstElementChild;
        observe.observe(rail,{subtree:true,attributes:true,childList:true,characterData:true});
        snap.as_of='new timestamp';update();await Promise.resolve();
        stable=detail.firstElementChild===original&&mutations===0;
        snap.primary_used_percent=40;snap.windows[0].used_percent=40;update();
        updated=rail.textContent.includes('40%')&&detail.textContent.includes('40%');
        const idle=detail.innerHTML;providers.setActivities([{provider_id:'claude',state:'running'}]);
        updated=updated&&detail.innerHTML!==idle;
      }else{
        const {mountStockDetail}=await import('/src/ui/stock-detail.ts');
        const stock=mountStockDetail(detail);
        const item={id:'a',symbol:'AAPL',asset_kind:'equity',display_name:'Apple',sort_index:0,card_tint:'none'};
        const quote={symbol:'AAPL',price:202,change_percent:1,previous_close:200,as_of:'first',currency:'USD',source:'fixture',display:{primary:{price:202,change:1},secondary:{price:null,change:null}}};
        const spark={symbol:'AAPL',as_of:'fixed',points:[{t:10,close:200},{t:20,close:201},{t:30,close:202}]};
        const update=()=>{notch.update([item],[structuredClone(quote)],6);stock.update(item,structuredClone(quote),structuredClone(spark));};
        update();const original=detail.querySelector('.row-sparkline').firstElementChild;
        observe.observe(rail,{subtree:true,attributes:true,childList:true,characterData:true});
        quote.as_of='new timestamp';update();await Promise.resolve();
        stable=detail.querySelector('.row-sparkline').firstElementChild===original&&mutations===0;
        const before=detail.querySelector('.row-sparkline').innerHTML;
        spark.points[1].close=199;update();
        updated=detail.querySelector('.row-sparkline').innerHTML!==before;
        quote.price=204;quote.change_percent=2;quote.display.primary={price:204,change:2};update();
        updated=updated&&rail.textContent.includes('2.00%')&&detail.textContent.includes('204');
      }
      observe.disconnect();return {stable,updated};
    }''', args.token)
    assert result['stable'], result
    assert result['updated'], result
    if args.token:
        page.evaluate('''async()=>{
          const {mountProviders}=await import('/src/ui/providers.ts');
          const root=document.querySelector('#detail');
          window.countdownProvider=mountProviders(root);
          window.countdownSnapshot={provider_id:'claude',display_name:'Claude',windows:[{kind:'rolling_5h',used:25,limit:100,unit:'percent',resets_at:new Date(Date.now()+75000).toISOString(),used_percent:25,label:'5h'}],status:'ok',source:'vendor',as_of:'first',message:null,primary_resets_at:null,primary_used_percent:25};
          window.countdownProvider.setSnapshots([window.countdownSnapshot]);
          window.originalCountdown=root.firstElementChild;
        }''')
        assert not page.locator('.window-reset').evaluate('(el)=>el.classList.contains("urgent")')
        page.clock.run_for(30000)
        page.evaluate('window.countdownProvider.setSnapshots([{...window.countdownSnapshot,as_of:"later"}])')
        assert page.evaluate('document.querySelector("#detail").firstElementChild===window.originalCountdown')
        assert page.locator('.window-reset').evaluate('(el)=>el.classList.contains("urgent")&&el.textContent.includes("soon")')
        result['countdown_survives_unchanged_snapshots']=True
    print(json.dumps(result))
    browser.close()
