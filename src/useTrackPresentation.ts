import {useEffect,useRef,useState} from 'react';
import type {MediaState} from './media';

export function trackReady(media:MediaState|null):boolean {
 return Boolean(media?.connected&&media.title.trim()&&(media.sourceId!=='ru.yandex.desktop.music'||media.artist.trim())&&['Playing','Paused'].includes(media.status));
}
export function trackKey(media:MediaState|null){return media?[media.sourceId,media.title,media.artist,media.album].join('\u0000'):'';}

// Presentation is buffered; transport controls always use the live media state.
export function useTrackPresentation(media:MediaState|null,playerId:string,starting:boolean){
 const [current,setCurrent]=useState<MediaState|null>(null),[previous,setPrevious]=useState<MediaState|null>(null);
 const generation=useRef(0),shown=useRef<MediaState|null>(null);
 useEffect(()=>{generation.current++;shown.current=null;setCurrent(null);setPrevious(null);},[playerId]);
 useEffect(()=>{
  const token=++generation.current;
  if(media?.sourceId&&media.sourceId!==playerId)return;
  if(!media||!trackReady(media)){
   if(starting||media?.connected)return;
   const timer=setTimeout(()=>{if(generation.current===token){shown.current=null;setCurrent(null);setPrevious(null);}},30000);
   return()=>clearTimeout(timer);
  }
  let cancelled=false;
  const commit=()=>{
   if(cancelled||generation.current!==token)return;
   const old=shown.current;
   if(trackKey(old)===trackKey(media)&&old?.cover&&!media.cover)return;
   if(trackKey(old)!==trackKey(media)||old?.cover!==media.cover){setPrevious(old);shown.current=media;setCurrent(media);}
  };
  const timer=setTimeout(()=>{
   if(!media.cover){commit();return;}
   const image=new Image();image.src=media.cover;
   void image.decode().catch(()=>{}).then(commit);
  },media.cover?160:900);
  return()=>{cancelled=true;clearTimeout(timer);};
 },[media?.sourceId,media?.connected,media?.title,media?.artist,media?.album,media?.cover,media?.status,playerId,starting]);
 useEffect(()=>{if(!previous)return;const timer=setTimeout(()=>setPrevious(null),280);return()=>clearTimeout(timer);},[previous,current]);
 return {current,previous,key:trackKey(current),waiting:Boolean(current&&(!trackReady(media)||trackKey(current)!==trackKey(media)))};
}
