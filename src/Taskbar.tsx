import React,{useEffect,useRef,useState} from 'react';
import {invoke} from '@tauri-apps/api/core';
import {listen} from '@tauri-apps/api/event';
import {Play,Pause,SkipBack,SkipForward,Heart,TextSearch} from 'lucide-react';
import {LiquidGlow} from './LiquidGlow';
import {TaskbarTrack} from './TaskbarTrack';
import {useTrackPresentation} from './useTrackPresentation';
import type {MediaState,Action,Settings} from './media';
interface YandexFeatures {title:string;artist:string;likeAvailable:boolean;liked:boolean|null;lyricsAvailable:boolean}
export function Taskbar(){
 const [heartTap,setHeartTap]=useState(0);
 const [features,setFeatures]=useState<YandexFeatures|null>(null),[specialBusy,setSpecialBusy]=useState(false);
 const featureSequence=useRef(0),specialPending=useRef(false);

 const [settings,setSettings]=useState<Settings|null>(null),[startup,setStartup]=useState({busy:false,message:""});
 const [media,setMedia]=useState<MediaState|null>(null),[busy,setBusy]=useState(false),[error,setError]=useState('');
 useEffect(()=>{document.documentElement.dataset.view='taskbar';document.documentElement.dataset.theme='system';let disposed=false,received=false;let release:(()=>void)|undefined;let releaseSettings:(()=>void)|undefined;let releaseStartup:(()=>void)|undefined;
  void (async()=>{releaseSettings=await listen<Settings>('settings-changed',e=>{if(!disposed){document.documentElement.dataset.theme=e.payload.theme;setSettings(e.payload);}});if(disposed){releaseSettings();return;}const settings=await invoke<Settings>('get_settings');if(!disposed){document.documentElement.dataset.theme=settings.theme;setSettings(settings);}releaseStartup=await listen<{busy:boolean;message:string}>('player-startup',e=>{if(!disposed)setStartup(e.payload);});if(disposed){releaseStartup();return;}release=await listen<MediaState>('media-state',e=>{received=true;if(!disposed)setMedia(e.payload);});if(disposed){release();return;}const m=await invoke<MediaState>('get_media');if(!disposed&&!received)setMedia(m);})().catch(e=>setError(String(e)));
  return()=>{disposed=true;release?.();releaseSettings?.();releaseStartup?.();};
 },[]);
 useEffect(()=>{let disposed=false,release:(()=>void)|undefined;const apply=(light:boolean)=>{if(!disposed)document.documentElement.dataset.taskbarTheme=light?'light':'dark';};void (async()=>{release=await listen<boolean>('taskbar-theme',e=>apply(e.payload));if(disposed){release();return;}apply(await invoke<boolean>('get_taskbar_light_theme'));})().catch(()=>{});return()=>{disposed=true;release?.();};},[]);
 useEffect(()=>{
  const element=document.querySelector('.taskbar-strip');if(!element)return;
  let previous=0,disposed=false;
  const measure=()=>{if(disposed)return;const width=Math.ceil(element.getBoundingClientRect().width);if(width!==previous){previous=width;void invoke('set_taskbar_width',{width}).catch(()=>{});}};
  const observer=new ResizeObserver(measure);observer.observe(element);measure();void document.fonts.ready.then(measure);
  return()=>{disposed=true;observer.disconnect();};
 },[]);
 const refreshFeatures=async()=>{
  const sequence=++featureSequence.current;
  try{const value=await invoke<YandexFeatures>('get_yandex_features');if(sequence===featureSequence.current)setFeatures(value);}catch{if(sequence===featureSequence.current)setFeatures(null);}
 };
 useEffect(()=>{
  let disposed=false,timer:ReturnType<typeof setTimeout>;setFeatures(null);featureSequence.current++;
  if(media?.sourceId!=='ru.yandex.desktop.music'||!media?.connected)return;
  const refresh=async()=>{if(disposed)return;if(!specialPending.current)await refreshFeatures();if(!disposed)timer=setTimeout(refresh,2500);};void refresh();
  return()=>{disposed=true;clearTimeout(timer);featureSequence.current++;};
 },[media?.sourceId,media?.connected,media?.title,media?.artist]);
 useEffect(()=>()=>{featureSequence.current++;},[]);
 const special=async(action:'like'|'lyrics')=>{
  if(specialPending.current||!media)return;if((action==='like'||action==='lyrics')&&(track.current?.title!==media.title||track.current?.artist!==media.artist)){setError('Дождитесь появления нового трека');return;}specialPending.current=true;featureSequence.current++;setSpecialBusy(true);setError('');
  try{const result=await invoke<YandexFeatures>('yandex_action',{action,title:media.title,artist:media.artist,desired:action==='like'?features?.liked!==true:null});if(result?.title)setFeatures(result);await refreshFeatures();}
  catch(e){setError(String(e));}finally{specialPending.current=false;setSpecialBusy(false);}
 };
 const track=useTrackPresentation(media,settings?.playerId||'',startup.busy);
 useEffect(()=>{document.documentElement.dataset.motion=settings?.reduceMotion?'reduce':'full';},[settings?.reduceMotion]);
 const playing=media?.status==='Playing';const blocked=busy||startup.busy||specialBusy;const player=settings?.playerName||'Музыка';
 const command=async(action:Action)=>{setBusy(true);setError('');try{await invoke('playback',{action});}catch(e){setError(String(e));}finally{setBusy(false);}};
 const yandex=media?.sourceId==='ru.yandex.desktop.music'&&media.connected;
 const displayedCurrent=track.current?.title===media?.title&&track.current?.artist===media?.artist;
 const currentFeatures=displayedCurrent&&features?.title===media?.title&&features?.artist===media?.artist?features:null;
 return <div className="taskbar-shell">
  <div className={`taskbar-strip ${track.waiting?'track-waiting':''}`} aria-busy={track.waiting||startup.busy} role="group" aria-label="LUMINI на панели задач" title={error||undefined}>
  <LiquidGlow trackId={track.key} kickReactive={settings?.kickReactive!==false} cover={track.current?.cover||null} playing={playing&&Boolean(media?.connected)} enabled={settings?.visualizer!==false} reduceMotion={Boolean(settings?.reduceMotion)}/>
  <TaskbarTrack current={track.current} previous={track.previous} media={media} trackKey={track.key} player={player} starting={startup.busy} blocked={blocked} onError={setError}/>
  <button aria-label="Предыдущий трек" disabled={blocked||!media?.controls.previous} onClick={()=>void command('previous')}><SkipBack size={15} fill="currentColor"/></button>
  <button className="taskbar-play" aria-label={playing?'Пауза':'Воспроизвести'} disabled={blocked||!(playing?media?.controls.pause:(media?.controls.play||settings?.launchOnPlay))} onClick={()=>void command(playing?'pause':'play')}>{playing?<Pause size={17} fill="currentColor"/>:<Play size={17} fill="currentColor" className={startup.busy?"starting":""}/>}</button>
  <button aria-label="Следующий трек" disabled={blocked||!media?.controls.next} onClick={()=>void command('next')}><SkipForward size={15} fill="currentColor"/></button>
  <button className={`taskbar-heart ${currentFeatures?.liked===true?'liked':''}`} aria-label={currentFeatures?.liked?'Убрать из избранного':'Добавить в избранное'} aria-pressed={currentFeatures?.liked===true} title={!currentFeatures?.likeAvailable?'Избранное пока недоступно':currentFeatures.liked?'Убрать из избранного':'Добавить в избранное'} disabled={blocked||!yandex||!displayedCurrent||!currentFeatures?.likeAvailable} onClick={()=>{setHeartTap(n=>n+1);void special('like');}}><span key={heartTap} className={`heart-glyph ${heartTap?'heart-tap':''}`}><Heart size={16} fill={currentFeatures?.liked?'currentColor':'none'}/></span></button>
  {currentFeatures?.lyricsAvailable&&<button className="taskbar-lyrics" aria-label="Текст песни" title="Открыть текст в Яндекс Музыке" disabled={blocked||!yandex} onClick={()=>void special('lyrics')}><TextSearch size={19}/></button>}

 </div></div>;
}
