import { invoke } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { mountNotch, position } from "./notch";
import { createNotchSurface } from "./notch-surface";
import { mountNotchDrag } from "./notch-drag";
import { mountSettingsPanel } from "./settings-panel";
import { mountStockDetail } from "./stock-detail";
import { mountLiquidGlass } from "./liquid-glass";
import { applyPanelOpacity } from "./opacity";
import { initialNotchState, reduceNotchState, CLOSE_DELAY_MS, MAX_CELLS } from "./notch-state";
import { logCaptureEvent } from "./capture-log";
import type { NotchEvent } from "./notch-state";
import type { DragNotice } from "./notch-drag";
import type { AppSettings, PersistedState, Quote, Sparkline, NotchLayout } from "./types";

const DETAIL_PADDING = 34;
const SETTINGS_FALLBACK = 440;
export async function mountApp(root: HTMLElement): Promise<void> {
  root.innerHTML=`<div class="notch-shell"><nav class="notch" aria-label="관심종목"></nav><section class="notch-detail" aria-label="종목 상세" hidden><div class="detail-body"><div class="stock-detail"></div><div class="detail-settings" hidden><div class="settings-root"></div></div><p class="notch-error" role="status" hidden></p></div></section></div>`;
  const shell=root.querySelector<HTMLElement>('.notch-shell')!;
  const rail=root.querySelector<HTMLElement>('.notch')!;
  const detail=root.querySelector<HTMLElement>('.notch-detail')!;
  const stockRoot=root.querySelector<HTMLElement>('.stock-detail')!;
  const settingsArea=root.querySelector<HTMLElement>('.detail-settings')!;
  const errorEl=root.querySelector<HTMLElement>('.notch-error')!;
  const persisted=await invoke<PersistedState>('get_state');
  let quotes:Quote[]=[],sparks:Sparkline[]=[];
  let interaction=initialNotchState(),beforeDrag=initialNotchState();
  let layout:NotchLayout|undefined;
  let revealRevision=0;
  let settings:ReturnType<typeof mountSettingsPanel>|undefined;
  let closeTimer:ReturnType<typeof setTimeout>|undefined;
  let measureFrame=0,destroyed=false;
  let pendingSettingsHeight:number|null=null, settingsOpening=false;
  const events=new AbortController(), unlisteners:(()=>void)[]=[];
  const cancelClose=()=>{clearTimeout(closeTimer);};
  const fail=(error:unknown)=>{console.error(error);errorEl.textContent=String(error);errorEl.hidden=false;};
  const surface=createNotchSurface({send:request=>invoke<NotchLayout>('set_notch_surface',{...request}),paint:(next,request)=>{
    paintLayout(next);
    if(settingsOpening && interaction.settings && request.expanded && request.target===null && request.height===pendingSettingsHeight){
      settingsOpening=false;detail.style.visibility='';
    }
  },fail});
  const drag=mountNotchDrag(rail,()=>layout,{
    begin:id=>invoke('begin_notch_drag',{id}),finish:(id,cancel)=>invoke('finish_notch_drag',{id,cancel}),
    onStart:()=>{beforeDrag={...interaction};cancelClose();},
    onNotice:notice=>{
      if(notice.active)cancelClose();
      if(notice.finished && notice.active)interaction=beforeDrag.settings||beforeDrag.pinned ? beforeDrag : initialNotchState();
      if(notice.error)fail(notice.error);
      render();
    },fail,
  });
  const notch=mountNotch(rail,{
    hover:id=>{if(!drag.dragging && persisted.settings.hover_detail){cancelClose();dispatch({type:'hover',id});}},
    pin:id=>dispatch({type:'pin',id}),
    more:()=>openSettings('symbols'),
  });
  const stock=mountStockDetail(stockRoot);
  const liquid=mountLiquidGlass(shell,rail,detail);
  function appearance(st:AppSettings){
    errorEl.hidden=true;
    shell.dataset.theme=st.theme ?? 'dark';
    shell.classList.toggle('reduce-animation',st.show_animation===false);
    applyPanelOpacity(shell,st.opacity);
    liquid.setEnabled(st.theme==='glass'||st.theme==='liquid');
  }
  appearance(persisted.settings);
  let version='unknown';try{version=await getVersion();}catch(error){fail(error);}
  settings=mountSettingsPanel(root.querySelector<HTMLElement>('.settings-root')!,persisted,{
    appearance,openDetail:id=>{interaction={itemId:id,pinned:true,settings:false};render();},
    close:closeSettings,resize:requestSurface,fail,
  },version);
  function dispatch(event:NotchEvent){
    if((event.type==='hover'||event.type==='pin')&&!persisted.watchlist.some(item=>item.id===event.id))return;
    interaction=reduceNotchState(interaction,event);render();
  }
  function openSettings(tab?:'symbols'){
    settingsOpening=true;pendingSettingsHeight=null;
    detail.style.visibility='hidden';
    cancelClose();interaction={...interaction,settings:true};render();settings?.show(tab);requestSurface();
    void getCurrentWindow().setFocus().catch(fail);
  }
  function closeSettings(){errorEl.hidden=true;interaction={...interaction,settings:false};render();notch.focus(interaction.itemId);}
  function paintLayout(next:NotchLayout){
    if(destroyed)return;
    layout=next;shell.dataset.edge=next.edge;
    shell.style.setProperty('--notch-scale',String(next.metrics.content_scale));
    shell.style.setProperty('--detail-radius',`${next.metrics.detail_radius}px`);
    notch.layout(next);
    notch.update(persisted.watchlist,quotes,next.metrics.slot_count);
    notch.select(interaction.itemId,interaction.pinned);
    if(next.detail)position(detail,next.detail,next);
    requestSurface();
  }
  function requestSurface(){
    if(destroyed || measureFrame)return;
    measureFrame=requestAnimationFrame(()=>{
      measureFrame=0;
      const expanded=interaction.settings||interaction.itemId!==null;
      const scale=layout?.metrics.content_scale ?? 1;
      const height=interaction.settings ? settings?.getContentHeight(scale) ?? SETTINGS_FALLBACK : Math.ceil(stockRoot.scrollHeight+DETAIL_PADDING*scale+(errorEl.hidden?0:errorEl.scrollHeight));
      if(settingsOpening)pendingSettingsHeight=height;
      surface.request({expanded,height,target:interaction.settings ? null : notch.target(interaction.itemId),providers:persisted.watchlist.map(item=>item.id)});
    });
  }
  function render(){
    if(destroyed)return;
    const selected=persisted.watchlist.find(item=>item.id===interaction.itemId);
    if(interaction.itemId && !selected)interaction={...interaction,itemId:null,pinned:false};
    detail.hidden=!interaction.settings&&!selected;
    if(!interaction.settings){settingsOpening=false;pendingSettingsHeight=null;detail.style.visibility='';}
    detail.classList.toggle('is-settings',interaction.settings);
    detail.setAttribute('aria-label',interaction.settings?'설정':`${selected?.symbol ?? ''} 상세`);
    stockRoot.hidden=interaction.settings||!selected;
    settingsArea.hidden=!interaction.settings;
    rail.inert=interaction.settings;
    if(interaction.settings)settings?.show();else settings?.hide();
    if(selected&&!interaction.settings)stock.update(selected,quotes.find(q=>q.symbol===selected.symbol),sparks.find(s=>s.symbol===selected.symbol));
    notch.update(persisted.watchlist,quotes,layout?.metrics.slot_count ?? MAX_CELLS);
    notch.select(interaction.itemId,interaction.pinned);
    requestSurface();
  }
  rail.addEventListener('contextmenu',event=>{event.preventDefault();openSettings();},{signal:events.signal});
  detail.addEventListener('contextmenu',event=>event.preventDefault(),{signal:events.signal});
  const leave=()=>{
    cancelClose();closeTimer=setTimeout(()=>{if(drag.sessionId===null)dispatch({type:'leave'});},CLOSE_DELAY_MS);
  };
  for(const el of [rail,detail]){
    el.addEventListener('pointerenter',cancelClose,{signal:events.signal});el.addEventListener('pointerleave',leave,{signal:events.signal});
  }
  async function subscribe<T>(name:string,callback:(data:T)=>void){
    const unlisten=await listen<T>(name,event=>callback(event.payload));
    if(destroyed)unlisten();else unlisteners.push(unlisten);
  }
  function merge<T extends {symbol:string}>(old:T[],next:T[]){const map=new Map(old.map(item=>[item.symbol,item]));for(const item of next)map.set(item.symbol,item);return Array.from(map.values());}
  await Promise.all([
    subscribe<Quote[]>('quotes-updated',next=>{quotes=merge(quotes,next);render();}),
    subscribe<Sparkline[]>('sparklines-updated',next=>{sparks=merge(sparks,next);render();}),
    subscribe<PersistedState['watchlist']>('watchlist-updated',next=>{
      const selected=interaction.itemId;persisted.watchlist=next.slice().sort((a,b)=>a.sort_index-b.sort_index);
      interaction=reduceNotchState(interaction,{type:'items',ids:next.map(item=>item.id)});settings?.setItems(persisted.watchlist);render();if(selected&&!interaction.itemId)notch.focus(null);
    }),
    subscribe<NotchLayout>('notch-layout',paintLayout),
    subscribe<boolean>('notch-reveal',open=>{revealRevision++;notch.reveal(open);}),
    subscribe<boolean>('widget-visibility',visible=>liquid.setVisible(visible)),
    subscribe<DragNotice>('notch-drag',notice=>drag.notice(notice)),
    subscribe<number>('opacity-updated',opacity=>{persisted.settings.opacity=opacity;appearance(persisted.settings);}),
  ]);
  unlisteners.push(await getCurrentWindow().onFocusChanged(({payload})=>{
    logCaptureEvent('FOCUS', `onFocusChanged payload=${payload}`);
    if(!payload){
      drag.cancel();
      (document.activeElement as HTMLElement|null)?.blur();
      leave();
    }
  }));
  const initialRevealRevision=revealRevision;
  const [initialQuotes,initialSparks,revealed]=await Promise.all([invoke<Quote[]>('get_quotes'),invoke<Sparkline[]>('get_sparklines'),invoke<boolean>('get_notch_reveal')]);
  quotes=merge(initialQuotes,quotes);sparks=merge(initialSparks,sparks);
  // Quotes may wait for network polling; a newer native event owns input coverage.
  if(revealRevision===initialRevealRevision)notch.reveal(revealed);
  render();
  const resize=new ResizeObserver(requestSurface);resize.observe(stockRoot);
  document.fonts.ready.then(requestSurface).catch(fail);
  window.addEventListener('pagehide',()=>{
    destroyed=true;cancelClose();cancelAnimationFrame(measureFrame);events.abort();resize.disconnect();
    surface.destroy();drag.destroy();liquid.destroy();settings?.destroy();unlisteners.forEach(fn=>fn());
  },{once:true});
}
