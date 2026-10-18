import React, {useEffect,useState,useRef} from 'react';
import {createRoot} from 'react-dom/client';
import {invoke} from '@tauri-apps/api/core';
import {listen} from '@tauri-apps/api/event';
import {Music2,Play,Pause,SkipBack,SkipForward,ArrowUpRight,ChevronDown,ChevronUp,Settings2,X,Check} from 'lucide-react';
import {clock,projectedPosition,type MediaState,type Settings,type Action} from './media';
import './style.css';
import {useTrackPresentation} from './useTrackPresentation';
import {Taskbar} from './Taskbar';
import {PlayerSettings} from './PlayerSettings';

function App(){
  const [media,setMedia]=useState<MediaState|null>(null);
  const [settings,setSettings]=useState<Settings>({theme:'system',hotkeys:false,hotkey:'Ctrl+Alt+M',reduceMotion:false,visualizer:true,kickReactive:true,taskbar:true,taskbarOffset:160,playerId:"ru.yandex.desktop.music",launchAppId:"ru.yandex.desktop.music",playerName:"Яндекс Музыка",launchOnPlay:true,minimizeOnLaunch:true,bootstrapYandex:true});
  const [expanded,setExpanded]=useState(false),[settingsOpen,setSettingsOpen]=useState(false);
  const [startup,setStartup]=useState({busy:false,message:''});
  const [busy,setBusy]=useState(false),[error,setError]=useState(''),[now,setNow]=useState(Date.now());
  const [visible,setVisible]=useState(false),[drag,setDrag]=useState<number|null>(null);
  const seekValue=useRef<number|null>(null);
  const [autostart,setAutostart]=useState(false),[motionAllowed,setMotionAllowed]=useState(false);
  const run=async(fn:()=>Promise<unknown>)=>{setError('');setBusy(true);try{await fn();}catch(e){setError(String(e));}finally{setBusy(false);}};
  useEffect(()=>{
    let disposed=false,receivedEvent=false; const releases:Array<()=>void>=[];
    const register=async()=>{
      // Subscribe before requesting a snapshot, so startup cannot miss a session change.
      for (const [event,handler] of [
        ['player-startup',(payload:unknown)=>setStartup(payload as {busy:boolean;message:string})],
        ['settings-changed',(payload:unknown)=>setSettings(payload as Settings)],
        ['media-state',(payload:unknown)=>{receivedEvent=true;setMedia(payload as MediaState);setNow(Date.now());}],
        ['panel-visible',(payload:unknown)=>{setVisible(Boolean(payload));setNow(Date.now());invoke<boolean>('motion_allowed').then(setMotionAllowed).catch(()=>setMotionAllowed(false));}],
        ['panel-reset',(_payload:unknown)=>{setExpanded(false);setSettingsOpen(false);setDrag(null);seekValue.current=null;}],
      ] as const){const release=await listen(event,e=>{if(!disposed)handler(e.payload);});if(disposed)release();else releases.push(release);}
      const [snapshot,s,auto,motion]=await Promise.all([invoke<MediaState>('get_media'),invoke<Settings>('get_settings'),invoke<boolean>('get_autostart'),invoke<boolean>('motion_allowed')]);
      if(!disposed){if(!receivedEvent)setMedia(snapshot);setSettings(s);setAutostart(auto);setMotionAllowed(motion);}
    };
    register().catch(e=>{if(!disposed)setError(String(e));});
    return()=>{disposed=true;releases.forEach(release=>release());};
  },[]);
  useEffect(()=>{document.documentElement.dataset.theme=settings.theme;document.documentElement.dataset.motion=(!motionAllowed||settings.reduceMotion)?'reduce':'full';},[settings,motionAllowed]);
  useEffect(()=>{if(!visible||media?.status!=='Playing')return;const id=setInterval(()=>setNow(Date.now()),500);return()=>clearInterval(id);},[visible,media?.status]);
  useEffect(()=>{const handle=(e:KeyboardEvent)=>{if(e.key==='Escape')void invoke('hide_panel');};window.addEventListener('keydown',handle);return()=>window.removeEventListener('keydown',handle);},[]);
  const track=useTrackPresentation(media,settings.playerId,startup.busy);
  const blocked=busy||startup.busy;
  const playing=media?.status==='Playing';
  const title=track.current?.title.trim()||(settings.launchOnPlay?'Нажмите Play, чтобы начать':'Нет активного воспроизведения');
  const position=media?projectedPosition(media,now):0;
  const duration=media?Math.max(0,media.end-media.start):0;
  const canSeek=Boolean(media?.connected&&media.controls.seek&&media.maxSeek>media.minSeek&&duration>0);
  const command=(action:Action)=>run(()=>invoke('playback',{action}));
  const changeExpanded=()=>run(async()=>{await invoke('set_expanded',{expanded:!expanded});setExpanded(!expanded);setSettingsOpen(false);});
  const save=(next:Settings)=>run(async()=>{await invoke('set_settings',{settings:next});setSettings(next);});
  const updateSeek=(value:number)=>{seekValue.current=value;setDrag(value);};
  const seek=(value:number)=>{seekValue.current=null;setDrag(null);if(canSeek&&Number.isFinite(value))void command({seek:value});};
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
    </section>:<>
      <section className="track">
        <div className="art" key={track.current?.cover||"empty"}>{track.current?.cover?<img src={track.current.cover} alt="Обложка текущего альбома" draggable={false}/>:<Music2 size={expanded?64:28} strokeWidth={1.3}/>}</div>
        <div className="metadata"><div className="eyebrow"><span className={`status-dot ${playing?'playing':''}`}/>{playing?'Сейчас играет':media?.connected?'На паузе':settings.playerName}</div><h1 key={track.key} className="track-title" title={title}>{title}</h1><p key={track.key+'artist'} className="track-artist" title={track.current?.artist||''}>{track.current?.artist||startup.message||settings.playerName}</p>{expanded&&track.current?.album&&<span className="album">{track.current.album}</span>}</div>
      </section>
      {expanded&&<section className="timeline"><input aria-label="Позиция воспроизведения" type="range" min={canSeek?media!.minSeek:0} max={canSeek?media!.maxSeek:1} step="0.1" disabled={!canSeek||blocked} value={canSeek?Math.max(media!.minSeek,Math.min(media!.maxSeek,drag??position)):0} onChange={e=>updateSeek(Number(e.target.value))} onInput={e=>updateSeek(Number(e.currentTarget.value))} onPointerUp={e=>seek(seekValue.current??Number(e.currentTarget.value))} onKeyDown={e=>{if(!canSeek||blocked||!media)return;const base=seekValue.current??position;const targets:Record<string,number>={Home:media.minSeek,End:media.maxSeek,ArrowLeft:base-1,ArrowRight:base+1,PageDown:base-10,PageUp:base+10};if(e.key in targets){e.preventDefault();seek(Math.max(media.minSeek,Math.min(media.maxSeek,targets[e.key])));}}} onBlur={()=>{seekValue.current=null;setDrag(null);}}/><div><span>{clock(Math.max(0,(drag??position)-(media?.start??0)))}</span><span>{clock(duration)}</span></div></section>}
      <section className="transport" aria-label="Управление воспроизведением"><button className="icon" aria-label="Предыдущий трек" disabled={blocked||!media?.controls.previous} onClick={()=>void command('previous')}><SkipBack size={22} fill="currentColor"/></button><button className="primary" aria-label={playing?'Пауза':'Воспроизвести'} disabled={blocked||!(playing?media?.controls.pause:(media?.controls.play||settings.launchOnPlay))} onClick={()=>void command(playing?'pause':'play')}>{playing?<Pause size={24} fill="currentColor"/>:<Play size={24} fill="currentColor" className={startup.busy?'starting':''}/>}</button><button className="icon" aria-label="Следующий трек" disabled={blocked||!media?.controls.next} onClick={()=>void command('next')}><SkipForward size={22} fill="currentColor"/></button></section>
    </>}
    {(error||media?.error)&&<div className="error" role="alert" title={error||media?.error||''}>{error||media?.error}</div>}
    <footer><button className="open-app" disabled={blocked} onClick={()=>void run(()=>invoke('open_player'))}>{settings.playerName} <ArrowUpRight size={14}/></button><div className="footer-actions"><button className="icon subtle" aria-label={settingsOpen?'Вернуться к плееру':'Настройки'} aria-pressed={settingsOpen} disabled={blocked} onClick={()=>void run(async()=>{if(!expanded){await invoke('set_expanded',{expanded:true});setExpanded(true);}setSettingsOpen(!settingsOpen);})}><Settings2 size={16}/></button><button className="icon subtle" aria-label={expanded?'Компактный режим':'Расширенный режим'} disabled={blocked} onClick={()=>void changeExpanded()}>{expanded?<ChevronDown size={18}/>:<ChevronUp size={18}/>}</button></div></footer>
  </main>;
}
createRoot(document.getElementById('root')!).render(<React.StrictMode>{new URLSearchParams(location.search).get("view")==="taskbar"?<Taskbar/>:<App/>}</React.StrictMode>);
