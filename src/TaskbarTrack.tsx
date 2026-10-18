import React,{useEffect,useRef,useState,type CSSProperties} from 'react';
import {invoke} from '@tauri-apps/api/core';
import {Music2} from 'lucide-react';
import {clock,projectedPosition,type MediaState} from './media';
import {trackKey as mediaKey} from './useTrackPresentation';
interface Props {current:MediaState|null;previous:MediaState|null;media:MediaState|null;trackKey:string;player:string;starting:boolean;blocked:boolean;onError:(message:string)=>void}
type Drag={key:string;media:MediaState;value:number;pointerId:number};
export function TaskbarTrack({current,previous,media,trackKey,player,starting,blocked,onError}:Props){
 const [hover,setHover]=useState(false),[keyboard,setKeyboard]=useState(false),[dragging,setDragging]=useState(false),[preview,setPreview]=useState<number|null>(null),[held,setHeld]=useState<{key:string;value:number;at:number}|null>(null),[working,setWorking]=useState(false),[now,setNow]=useState(Date.now());
 const range=useRef<HTMLInputElement>(null),region=useRef<HTMLDivElement>(null),drag=useRef<Drag|null>(null),generation=useRef(0),pending=useRef(false),latest=useRef(media);latest.current=media;
 const key=mediaKey(media);
 const finite=Boolean(media&&[media.minSeek,media.maxSeek,media.start,media.end,media.position].every(Number.isFinite));
 const available=Boolean(finite&&media?.connected&&media.controls.seek&&media.maxSeek>media.minSeek&&media.end>media.start&&trackKey===key&&current&&!blocked);
 const open=available&&(hover||keyboard||dragging||working);
 const min=available?media!.minSeek:0,max=available?media!.maxSeek:1;
 const clamp=(value:number)=>Math.max(min,Math.min(max,value));
 const base=held?.key===key?held.value+(media?.status==='Playing'?Math.max(0,(now-held.at)/1000):0):media?projectedPosition(media,now):0;
 const value=clamp(preview??base),fill=max>min?(value-min)/(max-min)*100:0;
 const cancelDrag=()=>{const old=drag.current;drag.current=null;setDragging(false);setPreview(null);if(old&&range.current?.hasPointerCapture(old.pointerId))range.current.releasePointerCapture(old.pointerId);};
 useEffect(()=>{generation.current++;cancelDrag();setHeld(null);setWorking(false);pending.current=false;},[key]);
 useEffect(()=>()=>{generation.current++;},[]);
 useEffect(()=>{if(!open)return;setNow(Date.now());const timer=setInterval(()=>{if(!document.hidden)setNow(Date.now());},250);return()=>clearInterval(timer);},[open]);
 useEffect(()=>{if(!held)return;const timer=setTimeout(()=>setHeld(null),1400);return()=>clearTimeout(timer);},[held]);
 useEffect(()=>{if(held&&media&&held.key===key&&Math.abs(projectedPosition(media,Date.now())-(held.value+(media.status==='Playing'?(Date.now()-held.at)/1000:0)))<1.5)setHeld(null);},[media,held,key]);
 useEffect(()=>{if(open&&keyboard&&document.activeElement===region.current)range.current?.focus({preventScroll:true});},[open,keyboard]);
 const seek=async(target:number,source:MediaState)=>{
  if(pending.current||!available||!Number.isFinite(target)||mediaKey(latest.current)!==mediaKey(source))return;
  const token=generation.current,wanted=Math.max(source.minSeek,Math.min(source.maxSeek,target));pending.current=true;setWorking(true);setHeld({key:mediaKey(source),value:wanted,at:Date.now()});setPreview(null);onError('');
  try{await invoke('playback',{action:{seekTrack:{value:wanted,sourceId:source.sourceId,title:source.title,artist:source.artist,album:source.album}}});}
  catch(error){if(token===generation.current){setHeld(null);onError(String(error));}}
  finally{if(token===generation.current){pending.current=false;setWorking(false);}}
 };
 const update=(value:number)=>{if(drag.current){drag.current.value=value;setPreview(value);}};
 const title=current?.title||player,artist=current?.artist||(starting?'Подключаем плеер…':'Нет активного воспроизведения');
 return <div className="taskbar-info">
  <button className="taskbar-cover" aria-label="Открыть плеер LUMINI" title={`${title} — ${artist}`} onClick={()=>void invoke('show_from_taskbar')}>
   {previous&&<span className="cover-layer outgoing" aria-hidden="true">{previous.cover?<img src={previous.cover} alt=""/>:<Music2 size={18}/>}</span>}
   <span className={`cover-layer ${previous?'incoming':''}`} key={trackKey+'cover'}>{current?.cover?<img src={current.cover} alt="Обложка" draggable={false}/>:<Music2 size={18}/>}</span>
  </button>
  <div ref={region} className={`taskbar-text ${open?'seek-open':''}`} role="group" aria-label="Название трека и перемотка" tabIndex={available?0:-1} onPointerEnter={()=>setHover(true)} onPointerLeave={()=>setHover(false)} onFocusCapture={e=>{if(e.target.matches(':focus-visible'))setKeyboard(true);}} onBlurCapture={e=>{if(!e.currentTarget.contains(e.relatedTarget as Node|null))setKeyboard(false);}}>
   <button className="taskbar-label" aria-label={`Открыть плеер: ${title} — ${artist}`} tabIndex={available?-1:0} onClick={()=>void invoke('show_from_taskbar')}>
    {previous&&<span className="track-copy outgoing" aria-hidden="true"><b>{previous.title}</b><small>{previous.artist}</small></span>}
    <span className={`track-copy ${previous?'incoming':''}`} key={trackKey}><b>{title}</b><small>{artist}</small></span>
   </button>
   <div className="taskbar-seek" style={{'--seek-fill':`${fill}%`} as CSSProperties}>
    <input ref={range} type="range" aria-label="Перемотать текущий трек" aria-valuetext={`${clock(value-(media?.start??0))} из ${clock((media?.end??0)-(media?.start??0))}`} min={min} max={max} step="0.1" value={value} disabled={!available||working} tabIndex={open?0:-1}
     onPointerDown={e=>{if(!available||pending.current||!media||e.button!==0)return;drag.current={key,media:{...media},value:Number(e.currentTarget.value),pointerId:e.pointerId};setDragging(true);e.currentTarget.setPointerCapture(e.pointerId);}}
     onInput={e=>update(Number(e.currentTarget.value))}
     onChange={e=>{const next=Number(e.currentTarget.value);if(drag.current)update(next);else if(media)void seek(next,media);}}
     onPointerUp={e=>{const active=drag.current;if(!active)return;const target=active.value;cancelDrag();if(active.key===mediaKey(latest.current))void seek(target,active.media);}}
     onPointerCancel={cancelDrag} onLostPointerCapture={()=>{if(drag.current)cancelDrag();}}
     onKeyDown={e=>{if(e.key==='Escape'){e.preventDefault();cancelDrag();setKeyboard(false);e.currentTarget.blur();return;}if(!available||!media||pending.current)return;const base=held?.key===key?held.value:projectedPosition(media,Date.now());const targets:Record<string,number>={ArrowLeft:base-1,ArrowRight:base+1,PageDown:base-10,PageUp:base+10,Home:min,End:max};if(e.key in targets){e.preventDefault();void seek(clamp(targets[e.key]),{...media});}}}/>
    <div className="taskbar-seek-time" aria-hidden="true"><span>{clock(Math.max(0,value-(media?.start??0)))}</span><span>{clock(Math.max(0,(media?.end??0)-(media?.start??0)))}</span></div>
   </div>
  </div>
 </div>;
}
