import React, {useEffect,useState} from 'react';
import {createRoot} from 'react-dom/client';
import {invoke} from '@tauri-apps/api/core';
import {listen} from '@tauri-apps/api/event';
import {ArrowUpRight,Settings2,X,Check} from 'lucide-react';
import {type MediaState,type Settings} from './media';
import './style.css';
import {Taskbar} from './Taskbar';
import {PlayerSettings} from './PlayerSettings';
import {QueuePanel} from './QueuePanel';

function App(){
  const [media,setMedia]=useState<MediaState|null>(null);
  const [settings,setSettings]=useState<Settings>({theme:'system',hotkeys:false,hotkey:'Ctrl+Alt+M',reduceMotion:false,visualizer:true,kickReactive:true,taskbar:true,taskbarOffset:160,playerId:"ru.yandex.desktop.music",launchAppId:"ru.yandex.desktop.music",playerName:"Яндекс Музыка",launchOnPlay:true,minimizeOnLaunch:true,bootstrapYandex:true});
  const [expanded,setExpanded]=useState(false),[settingsOpen,setSettingsOpen]=useState(false);
  const [startup,setStartup]=useState({busy:false,message:''});
  const [busy,setBusy]=useState(false),[error,setError]=useState('');
  const [visible,setVisible]=useState(false);
  const [autostart,setAutostart]=useState(false),[motionAllowed,setMotionAllowed]=useState(false);
  const run=async(fn:()=>Promise<unknown>)=>{setError('');setBusy(true);try{await fn();}catch(e){setError(String(e));}finally{setBusy(false);}};
  useEffect(()=>{
    let disposed=false,receivedEvent=false; const releases:Array<()=>void>=[];
    const register=async()=>{
      // Subscribe before requesting a snapshot, so startup cannot miss a session change.
      for (const [event,handler] of [
        ['player-startup',(payload:unknown)=>setStartup(payload as {busy:boolean;message:string})],
        ['settings-changed',(payload:unknown)=>setSettings(payload as Settings)],
        ['media-state',(payload:unknown)=>{receivedEvent=true;setMedia(payload as MediaState);}],
        ['panel-visible',(payload:unknown)=>{setVisible(Boolean(payload));invoke<boolean>('motion_allowed').then(setMotionAllowed).catch(()=>setMotionAllowed(false));}],
        ['panel-reset',(_payload:unknown)=>{setExpanded(false);setSettingsOpen(false);}],
      ] as const){const release=await listen(event,e=>{if(!disposed)handler(e.payload);});if(disposed)release();else releases.push(release);}
      const [snapshot,s,auto,motion]=await Promise.all([invoke<MediaState>('get_media'),invoke<Settings>('get_settings'),invoke<boolean>('get_autostart'),invoke<boolean>('motion_allowed')]);
      if(!disposed){if(!receivedEvent)setMedia(snapshot);setSettings(s);setAutostart(auto);setMotionAllowed(motion);}
    };
    register().catch(e=>{if(!disposed)setError(String(e));});
    return()=>{disposed=true;releases.forEach(release=>release());};
  },[]);
  useEffect(()=>{document.documentElement.dataset.theme=settings.theme;document.documentElement.dataset.motion=(!motionAllowed||settings.reduceMotion)?'reduce':'full';},[settings,motionAllowed]);
  useEffect(()=>{const handle=(e:KeyboardEvent)=>{if(e.key==='Escape')void invoke('hide_panel');};window.addEventListener('keydown',handle);return()=>window.removeEventListener('keydown',handle);},[]);
  const blocked=busy||startup.busy;
  const save=(next:Settings)=>run(async()=>{await invoke('set_settings',{settings:next});setSettings(next);});
  return <main className={`panel ${expanded?'expanded':''}`}>
    <header><div className="brand"><span className="logo"><i/><i/><i/></span><span>LUMINI <small>Музыка</small></span></div><button className="icon subtle" aria-label="Скрыть панель" onClick={()=>void invoke('hide_panel')}><X size={16}/></button></header>
    {settingsOpen?<section className="preferences">
      <div className="section-title"><h1>Ваш ритм</h1><span>Настройки</span></div>
      <PlayerSettings settings={settings} save={save} busy={blocked}/>
      <label className="theme-label">Оформление</label><div className="segmented">{(['system','light','dark'] as const).map((theme,i)=><button key={theme} aria-pressed={settings.theme===theme} disabled={blocked} onClick={()=>void save({...settings,theme})}>{['Система','Светлое','Тёмное'][i]}</button>)}</div>
      <label className="setting"><span><b>Запускать с Windows</b><small>Работать в фоне после входа</small></span><input type="checkbox" checked={autostart} disabled={blocked} onChange={e=>{const enabled=e.target.checked;void run(async()=>{await invoke('set_autostart',{enabled});setAutostart(enabled);});}}/></label>
      <label className="setting"><span><b>Горячая клавиша</b><small>{settings.hotkey} — открыть панель</small></span><input type="checkbox" checked={settings.hotkeys} disabled={blocked} onChange={e=>void save({...settings,hotkeys:e.target.checked})}/></label>
      <div className="segmented shortcut-choices" aria-label="Комбинация клавиш">{['Ctrl+Alt+M','Ctrl+Alt+Space','Ctrl+Shift+F11'].map(key=><button key={key} disabled={blocked} title={key} aria-pressed={settings.hotkey===key} onClick={()=>void save({...settings,hotkey:key})}>{key}</button>)}</div>
      <label className="setting"><span><b>Жидкое свечение</b><small>Мягкие переливы в цветах обложки на панели задач</small></span><input type="checkbox" checked={settings.visualizer} disabled={blocked} onChange={e=>void save({...settings,visualizer:e.target.checked})}/></label>
      <label className="setting"><span><b>Реакция на музыку</b><small>Кик, бас и мягкое движение от остальных частот</small></span><input type="checkbox" checked={settings.kickReactive} disabled={blocked} onChange={e=>void save({...settings,kickReactive:e.target.checked})}/></label>
      <label className="setting"><span><b>Меньше анимаций</b><small>Настройка Windows учитывается всегда</small></span><input type="checkbox" checked={settings.reduceMotion} disabled={blocked} onChange={e=>void save({...settings,reduceMotion:e.target.checked})}/></label>
      <label className="setting"><span><b>Плеер на панели задач</b><small>Постоянная полоска рядом с погодой</small></span><input type="checkbox" checked={settings.taskbar} disabled={blocked} onChange={e=>void save({...settings,taskbar:e.target.checked})}/></label>
      <label className="setting"><span><b>Отступ слева</b><small>Чтобы освободить место погоде и значкам</small></span><input aria-label="Отступ полоски слева" type="number" min="8" max="1200" step="8" value={settings.taskbarOffset} disabled={blocked} onChange={e=>{const value=Number(e.target.value);if(value>=8&&value<=1200)void save({...settings,taskbarOffset:value});}}/></label>
      <p className="privacy"><Check size={15}/> Без аккаунтов и телеметрии</p>
    </section>:<QueuePanel media={media} settings={settings} visible={visible} blocked={blocked}/>}
    {(error||media?.error)&&<div className="error" role="alert" title={error||media?.error||''}>{error||media?.error}</div>}
    <footer><button className="open-app" disabled={blocked} onClick={()=>void run(()=>invoke('open_player'))}>{settings.playerName} <ArrowUpRight size={14}/></button><div className="footer-actions"><button className="icon subtle" aria-label={settingsOpen?'Вернуться к очереди':'Настройки'} aria-pressed={settingsOpen} disabled={blocked} onClick={()=>void run(async()=>{await invoke('set_expanded',{expanded:!settingsOpen});setExpanded(!settingsOpen);setSettingsOpen(!settingsOpen);})}><Settings2 size={16}/></button></div></footer>
  </main>;
}
createRoot(document.getElementById('root')!).render(<React.StrictMode>{new URLSearchParams(location.search).get("view")==="taskbar"?<Taskbar/>:<App/>}</React.StrictMode>);
