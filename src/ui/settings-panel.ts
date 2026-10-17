import { invoke } from "@tauri-apps/api/core";
import { mountSettingsPanel as mountGeneralPanel } from "./general-settings";
import { mountWatchlist } from "./watchlist";
import { opacityToPct, pctToOpacity } from "./opacity";
import { OPACITY_DEFAULT } from "./types";
import type { AppSettings, AppTheme, PersistedState } from "./types";
export { applyPanelOpacity } from "./opacity";

type Tab = "appearance" | "symbols" | "general";
const TABS: [Tab,string][] = [["appearance","모양"],["symbols","종목"],["general","일반"]];
const TAB_ICONS: Record<Tab,string> = {
  appearance:'<path d="M2 5h3m3 0h6M2 11h6m3 0h3"/><circle cx="6.5" cy="5" r="1.5"/><circle cx="9.5" cy="11" r="1.5"/>',
  symbols:'<path d="M3 3h10v10H3zM3 6.5h10M6.5 6.5V13"/>',
  general:'<circle cx="8" cy="8" r="2.5"/><path d="M8 1.5v1.8m0 9.4v1.8m6.5-6.5h-1.8m-9.4 0H1.5m11.1-4.6-1.3 1.3m-6.6 6.6-1.3 1.3m9.2 0-1.3-1.3m-6.6-6.6-1.3-1.3"/>',
};
const FALLBACK_HEIGHT = 440;
const MAX_HEIGHT = 560;
const PREFS = [["always_show_notch","노치 항상 표시"],["hover_detail","마우스 올릴 때 상세 열기"],["show_animation","모션 효과"]] as const;
const THEMES: [AppTheme,string][] = [["dark","다크"],["glass","글라스"],["liquid","리퀴드"]];
export function mountSettingsPanel(root: HTMLElement, initial: PersistedState, handlers: {
  appearance: (settings: AppSettings) => void; openDetail: (id:string) => void;
  close: () => void; resize: () => void; fail: (e: unknown) => void;
}, appVersion: string) {
  const settings = initial.settings;
  let lastHeight = FALLBACK_HEIGHT;
  root.classList.add("settings-root");
  root.setAttribute('role','dialog');root.setAttribute('aria-modal','true');root.setAttribute('aria-label','설정');
  root.innerHTML = `<header class="settings-heading"><strong>설정</strong><button class="icon-btn" id="settings-close" aria-label="설정 닫기">×</button></header>
    <div class="settings-tabs" role="tablist" aria-label="설정">${TABS.map(([key,label])=>`<button type="button" role="tab" id="tab-btn-${key}" aria-controls="tab-panel-${key}" data-tab="${key}"><svg class="tab-icon" width="13" height="13" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${TAB_ICONS[key]}</svg><span>${label}</span></button>`).join('')}</div>
    <div class="settings-scroll">${TABS.map(([key])=>`<section role="tabpanel" id="tab-panel-${key}" aria-labelledby="tab-btn-${key}" class="settings-tab-panel"></section>`).join('')}</div>`;
  const panel = (tab:Tab) => root.querySelector<HTMLElement>(`#tab-panel-${tab}`)!;
  const appearance = panel("appearance");
  appearance.innerHTML = `<div class="settings-section"><span class="settings-label">테마</span><div class="settings-theme-picker" role="group" aria-label="테마">${THEMES.map(([key,label])=>`<button type="button" class="settings-theme-option" data-theme-option="${key}" aria-pressed="false"><span class="theme-swatch theme-swatch-${key}"></span>${label}</button>`).join('')}</div></div>
    <div class="settings-section"><label class="settings-label" for="appearance-opacity">불투명도</label><div class="opacity-setting"><div class="opacity-meter"><span class="opacity-meter-fill" aria-hidden="true"></span><input id="appearance-opacity" type="range" min="0" max="100" step="5"/></div><output id="appearance-opacity-value"></output></div></div>
    <div class="settings-section"><span class="settings-label">화면 표시</span><div class="settings-group">${PREFS.map(([key,label])=>`<label class="settings-toggle"><span class="settings-toggle-title">${label}</span><input type="checkbox" data-pref="${key}"/><span class="settings-switch" aria-hidden="true"></span></label>`).join('')}</div></div>`;
  function paintAppearance() {
    appearance.querySelectorAll<HTMLButtonElement>("[data-theme-option]").forEach(b=>b.setAttribute("aria-pressed",String(b.dataset.themeOption===(settings.theme ?? 'dark'))));
    for (const [key] of PREFS) appearance.querySelector<HTMLInputElement>(`[data-pref="${key}"]`)!.checked=settings[key] ?? key==='show_animation';
    const percent=opacityToPct(settings.opacity ?? OPACITY_DEFAULT);
    appearance.querySelector<HTMLInputElement>("#appearance-opacity")!.value=String(percent);
    appearance.querySelector<HTMLElement>('.opacity-setting')!.style.setProperty('--opacity-value',percent+'%');
    appearance.querySelector("output")!.textContent=percent+'%';
  }
  async function save(action:()=>Promise<void>) {
    const controls=appearance.querySelectorAll<HTMLInputElement|HTMLButtonElement>("input,button");
    if (appearance.getAttribute('aria-busy')==='true') return;
    controls.forEach(b=>b.disabled=true); appearance.setAttribute('aria-busy','true');
    delete appearance.dataset.saveState;
    try { await action(); }
    catch(e) { appearance.dataset.saveState='restored';console.error('Settings save failed',e); }
    finally { controls.forEach(b=>b.disabled=false); appearance.setAttribute('aria-busy','false'); paintAppearance(); handlers.appearance(settings); handlers.resize(); }
  }
  appearance.querySelectorAll<HTMLButtonElement>("[data-theme-option]").forEach(button=>button.addEventListener("click",()=>void save(async()=>{
    const theme=button.dataset.themeOption as AppTheme;
    await invoke("set_theme",{theme}); settings.theme=theme;
  })));
  appearance.querySelectorAll<HTMLInputElement>("[data-pref]").forEach(input=>input.addEventListener("change",()=>void save(async()=>{
    const next={hover_detail:settings.hover_detail ?? false,always_show_notch:settings.always_show_notch ?? false,show_animation:settings.show_animation ?? true};
    next[input.dataset.pref as keyof typeof next]=input.checked;
    await invoke("set_notch_preferences",{hoverDetail:next.hover_detail,alwaysShowNotch:next.always_show_notch,showAnimation:next.show_animation}); Object.assign(settings,next);
  })));
  const opacity=appearance.querySelector<HTMLInputElement>("#appearance-opacity")!;
  opacity.addEventListener("input",()=>{
    appearance.querySelector("output")!.textContent=opacity.value+'%';
    appearance.querySelector<HTMLElement>('.opacity-setting')!.style.setProperty('--opacity-value',opacity.value+'%');
    handlers.appearance({...settings,opacity:pctToOpacity(Number(opacity.value))});
  });
  opacity.addEventListener("change",()=>void save(async()=>{ const value=pctToOpacity(Number(opacity.value)); await invoke("set_opacity",{opacity:value});settings.opacity=value; }));
  const manager=mountWatchlist(panel("symbols"),{management:true,columnRatios:settings.column_ratios,onOpenDetail:handlers.openDetail});
  manager.setItems(initial.watchlist);
  const generalRoot=document.createElement('div'); panel('general').append(generalRoot);
  const general=mountGeneralPanel(generalRoot,{quoteRefreshMs:settings.quote_refresh_ms ?? settings.quote_refresh_secs ?? 500,autostart:settings.autostart,appVersion},{embedded:true});
  general.show(); generalRoot.removeAttribute('role');generalRoot.removeAttribute('aria-modal');
  function switchTab(tab:Tab) {
    manager.closeMenu();
    root.querySelector<HTMLElement>('.settings-tabs')!.style.setProperty('--active-tab',String(TABS.findIndex(([key])=>key===tab)));
    for(const [key] of TABS) {
      panel(key).hidden=key!==tab;
      const b=root.querySelector<HTMLButtonElement>(`#tab-btn-${key}`)!;
      b.setAttribute('aria-selected',String(key===tab));b.tabIndex=key===tab ? 0 : -1;
    }
    handlers.resize();
  }
  root.querySelectorAll<HTMLButtonElement>('[data-tab]').forEach(button=>button.addEventListener('click',()=>switchTab(button.dataset.tab as Tab)));
  root.querySelector('#settings-close')!.addEventListener('click',handlers.close);
  const observer=new MutationObserver(()=>handlers.resize());
  observer.observe(root.querySelector('.settings-scroll')!,{subtree:true,childList:true,characterData:true});
  paintAppearance();switchTab('appearance');
  return {
    show(tab?:Tab){root.hidden=false;if(tab)switchTab(tab);},
    hide(){manager.closeMenu();root.hidden=true;},
    setItems(items:PersistedState['watchlist']){manager.setItems(items);},
    getContentHeight(scale=1){
      if(!root.isConnected || !root.clientWidth)return lastHeight;
      const scroll=root.querySelector<HTMLElement>('.settings-scroll')!;
      let tallest=0;
      for(const [key] of TABS){const el=panel(key);const old=el.getAttribute('style');const wasHidden=el.hidden;el.hidden=false;el.style.cssText='position:absolute;visibility:hidden;display:flex;flex:none;min-height:0;height:max-content;max-height:none;overflow:visible;width:'+scroll.clientWidth+'px;';tallest=Math.max(tallest,el.scrollHeight);if(old===null)el.removeAttribute('style');else el.setAttribute('style',old);el.hidden=wasHidden;}
      const chrome=['.settings-heading','.settings-tabs'].reduce((height,selector)=>{
        const el=root.querySelector<HTMLElement>(selector)!,style=getComputedStyle(el);
        return height+el.offsetHeight+(parseFloat(style.marginTop)||0)+(parseFloat(style.marginBottom)||0);
      },0);
      const container=root.closest<HTMLElement>('.notch-detail');
      const css=container ? getComputedStyle(container) : null;
      const padding=css ? ['paddingTop','paddingBottom','borderTopWidth','borderBottomWidth'].reduce((sum,key)=>sum+(parseFloat(css[key as keyof CSSStyleDeclaration] as string)||0),0) : 34*scale;
      const gaps=(parseFloat(getComputedStyle(root).rowGap)||0)*2;
      lastHeight=Math.min(MAX_HEIGHT*scale,Math.ceil(tallest+chrome+padding+gaps+2));
      return lastHeight;
    },
    destroy(){observer.disconnect();manager.destroy();general.destroy();},
  };
}
