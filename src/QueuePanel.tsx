import React,{useEffect,useRef,useState} from 'react';
import {invoke} from '@tauri-apps/api/core';
import {AudioLines,ListMusic,Music2,Play,RefreshCw} from 'lucide-react';
import type {MediaState,Settings} from './media';
type Queue={token:string;tracks:Array<{title:string;artist:string;current:boolean}>};
export function QueuePanel({media,settings,visible,blocked}:{media:MediaState|null;settings:Settings;visible:boolean;blocked:boolean}){
 const [queue,setQueue]=useState<Queue|null>(null),[loading,setLoading]=useState(false),[busy,setBusy]=useState(false),[error,setError]=useState('');
 const generation=useRef(0),mounted=useRef(true);
 const yandex=settings.playerId==='ru.yandex.desktop.music';
 const refresh=async()=>{
  const id=++generation.current;setLoading(true);setError('');
  try{const result=await invoke<Queue>('get_queue');if(mounted.current&&id===generation.current)setQueue(result);}
  catch(e){if(mounted.current&&id===generation.current){setQueue(null);setError(String(e));}}
  finally{if(mounted.current&&id===generation.current)setLoading(false);}
 };
 useEffect(()=>{mounted.current=true;return()=>{mounted.current=false;generation.current++;};},[]);
 useEffect(()=>{
  generation.current++;setError('');
  if(!visible){setLoading(false);return;}
  if(!yandex){setQueue(null);setLoading(false);return;}
  if(busy||blocked){setLoading(false);return;}
  if(!media?.connected){setQueue(null);setLoading(false);return;}
  setLoading(true);
  const timer=setTimeout(()=>void refresh(),650);return()=>{clearTimeout(timer);generation.current++;};
 },[visible,yandex,media?.connected,media?.sourceId,media?.title,media?.artist,busy,blocked]);
 const act=async(command:string,args:Record<string,unknown>={})=>{
  setBusy(true);setError('');
  try{await invoke(command,args);}
  catch(e){if(mounted.current)setError(String(e));}
  finally{if(mounted.current)setBusy(false);}
 };
 const disabled=blocked||busy||loading;
 return <section className="queue-view" aria-label="Очередь треков">
  <div className="queue-heading"><div><span className="queue-eyebrow">ВАША МУЗЫКА</span><h1>Далее в очереди</h1></div>
   {yandex&&<button className="wave-button" disabled={disabled} onClick={()=>void act('play_wave')}><span className="wave-symbol"><AudioLines size={23}/></span><span>Моя волна<small>Под ваш ритм</small></span><Play size={12} fill="currentColor"/></button>}
  </div>
  <div className="queue-caption"><span><ListMusic size={14}/>{queue?`${queue.tracks.length} ${queue.tracks.length%10===1&&queue.tracks.length%100!==11?'трек':queue.tracks.length%10>=2&&queue.tracks.length%10<=4&&!(queue.tracks.length%100>=12&&queue.tracks.length%100<=14)?'трека':'треков'} из Яндекс Музыки`:settings.playerName}</span><button className="icon subtle" aria-label="Обновить очередь" disabled={disabled||!yandex||!media?.connected} onClick={()=>void refresh()}><RefreshCw size={14} className={loading?'queue-refreshing':''}/></button></div>
  <div className="queue-list" aria-busy={loading}>
   {loading&&!queue?<div className="queue-skeleton" aria-label="Загружаем очередь">{[0,1,2,3].map(i=><div key={i}><i/><span/></div>)}</div>:queue?.tracks.map((track,index)=><button key={`${index}:${track.title}:${track.artist}`} className={`queue-row ${track.current?'current':''}`} disabled={disabled} aria-label={`Воспроизвести ${track.title}, ${track.artist}`} onClick={()=>void act('play_queue',{token:queue.token,index})}><span className="queue-number">{track.current?<AudioLines size={16}/>:<><span>{String(index+1).padStart(2,'0')}</span><Play size={13} fill="currentColor"/></>}</span><span className="queue-track"><b>{track.title}</b><small>{track.artist||'Исполнитель не указан'}</small></span>{track.current&&<span className="queue-now">Сейчас</span>}</button>)}
   {!loading&&!queue&&<div className="queue-empty"><Music2 size={28} strokeWidth={1.3}/><b>{!yandex?'Очередь пока доступна для Яндекс Музыки':!media?.connected?'Начнём с музыки':'Очередь пока недоступна'}</b><p>{error||(!media?.connected?'Нажмите Play на панели задач или включите Мою волну.':'Обновите список после загрузки трека в Яндекс Музыке.')}</p></div>}
  </div>
  {queue&&error&&<p className="queue-message" role="alert">{error}</p>}
  {busy&&<p className="queue-message" role="status">Ожидаем подтверждение Яндекс Музыки…</p>}
  <p className="queue-note">{yandex?'Показаны подготовленные треки. Моя волна дополняет очередь во время прослушивания.':'Управление воспроизведением остаётся на панели задач.'}</p>
 </section>;
}
