import React,{useEffect,useRef,useState} from 'react';
import {listen} from '@tauri-apps/api/event';
import {invoke} from '@tauri-apps/api/core';
import {coverPalette,NEUTRAL_PALETTE,type Palette} from './coverPalette';
import {AudioMotion,type AudioBands} from './audioMotion';

export function LiquidGlow({cover,trackId,playing,enabled,reduceMotion,kickReactive}:{cover:string|null;trackId:string;playing:boolean;enabled:boolean;reduceMotion:boolean;kickReactive:boolean}){
 const canvas=useRef<HTMLCanvasElement>(null),clock=useRef(0),energy=useRef(.16);
 const colors=useRef<Palette>(NEUTRAL_PALETTE.map(c=>[...c]) as Palette);
 const targetPalette=useRef<Palette>(NEUTRAL_PALETTE),transition=useRef({key:trackId,phase:'steady',opacity:1,ready:true});
 const audio=useRef(new AudioMotion());
 const [palette,setPalette]=useState(NEUTRAL_PALETTE),[fromCover,setFromCover]=useState(false);
 useEffect(()=>{
  let cancelled=false;if(!enabled)return;
  const t=transition.current;
  if(t.key!==trackId){const hadTrack=Boolean(t.key);t.key=trackId;t.phase=hadTrack?'out':'in';if(!hadTrack)t.opacity=0;t.ready=false;}
  const apply=(value:Palette,from:boolean)=>{if(cancelled)return;targetPalette.current=value;t.ready=true;setPalette(value);setFromCover(from);};
  void coverPalette(cover).then(value=>apply(value,Boolean(cover))).catch(()=>apply(NEUTRAL_PALETTE,false));
  return()=>{cancelled=true;};
 },[cover,trackId,enabled]);
 useEffect(()=>{
  let disposed=false,release:(()=>void)|undefined;audio.current.reset();if(!enabled||!kickReactive||reduceMotion)return;
  const update=(value:AudioBands|null)=>{if(!disposed)audio.current.receive(value,performance.now(),Date.now());};
  void(async()=>{release=await listen<AudioBands>('audio-reactivity',e=>update(e.payload));if(disposed){release();return;}update(await invoke('get_audio_reactivity'));})().catch(()=>{});
  return()=>{disposed=true;release?.();};
 },[enabled,kickReactive,reduceMotion,trackId]);
 useEffect(()=>{
  const element=canvas.current;if(!element||!enabled)return;
  const ctx=element.getContext('2d');if(!ctx)return;
  const surface=document.createElement('canvas'),paint=surface.getContext('2d');if(!paint)return;
  const preference=matchMedia('(prefers-reduced-motion:reduce)');let reduced=reduceMotion||preference.matches;
  let frame=0,last=0,previous=0,width=0,height=0,disposed=false;
  const goal=playing?1:.16;
  const draw=(now:number)=>{
   if(disposed)return;
   const dt=previous?Math.min((now-previous)/1000,1):1/30;previous=now;
   const factor=reduced?1:1-Math.exp(-dt*2);
   energy.current+=(goal-energy.current)*factor;
   const t=transition.current;
   if(reduced){t.opacity=1;t.phase='steady';}
   else if(t.phase==='out'){t.opacity=Math.max(0,t.opacity-dt/.24);if(t.opacity===0&&t.ready){colors.current=targetPalette.current.map(c=>[...c]) as Palette;t.phase='in';}}
   else if(t.phase==='in'){t.opacity=Math.min(1,t.opacity+dt/.48);if(t.opacity===1)t.phase='steady';}
   const sound=audio.current.step(now,dt,kickReactive&&playing&&!reduced);
   let difference=0;
   colors.current=colors.current.map((c,i)=>c.map((v,j)=>{difference+=Math.abs(v-targetPalette.current[i][j]);return t.phase==='out'?v:v+(targetPalette.current[i][j]-v)*factor;}) as [number,number,number]) as Palette;
   if(!reduced&&playing)clock.current+=dt*(.65+sound.mid*.1);
   if(!width||!height)return;
   const dpr=Math.min(devicePixelRatio||1,2);paint.setTransform(dpr,0,0,dpr,0,0);paint.clearRect(0,0,width,height);
   const time=clock.current,base=height+7;
   for(let layer=0;layer<3;layer++){
    paint.save();paint.beginPath();paint.moveTo(0,base);
    for(let x=0;x<=width+2;x+=2){const u=Math.min(1,x/width),envelope=Math.pow(Math.sin(Math.PI*u),.65);const amplitude=(25+7*Math.sin(u*8+time*.52+layer*.9)+5*Math.sin(u*14-time*.4+layer)+sound.mid*1.6*Math.sin(u*19+time*.8+layer))*(1+layer*.12)*(1+sound.kick*.2+sound.sub*.1+sound.bass*.08);paint.lineTo(x,base-envelope*amplitude);}
    paint.lineTo(width,height+8);paint.lineTo(0,height+8);paint.closePath();paint.clip();
    colors.current.forEach((color,i)=>{const px=[.17,.52,.82][i],py=[.45,.57,.48][i],x=width*(px+(.075+sound.mid*.012)*Math.sin(time*.3+i*2+layer)),y=height*(py+.13*Math.cos(time*.27+i+layer*.4)-sound.sub*.035);const gradient=paint.createRadialGradient(x,y,0,x,y,width*(.36+sound.sub*.025));const rgb=color.map(Math.round).join(','),alpha=(.60+sound.kick*.16+sound.bass*.04+sound.treble*.015)*energy.current*t.opacity;gradient.addColorStop(0,`rgba(${rgb},${alpha*(layer===0?.52:.29)})`);gradient.addColorStop(.54,`rgba(${rgb},${alpha*.25})`);gradient.addColorStop(1,`rgba(${rgb},0)`);paint.fillStyle=gradient;paint.fillRect(0,0,width,height);});paint.restore();
   }
   paint.globalCompositeOperation='destination-in';const fade=paint.createLinearGradient(0,0,0,height);fade.addColorStop(0,'transparent');fade.addColorStop(.25,'rgba(0,0,0,.9)');fade.addColorStop(.65,'black');fade.addColorStop(1,'transparent');paint.fillStyle=fade;paint.fillRect(0,0,width,height);paint.globalCompositeOperation='source-over';
   ctx.setTransform(dpr,0,0,dpr,0,0);ctx.clearRect(0,0,width,height);ctx.filter='blur(3px)';ctx.drawImage(surface,0,0,width,height);
   element.dataset.energy=energy.current.toFixed(2);element.dataset.kick=sound.kick.toFixed(3);element.dataset.bass=sound.bass.toFixed(3);element.dataset.sub=sound.sub.toFixed(3);element.dataset.mid=sound.mid.toFixed(3);element.dataset.treble=sound.treble.toFixed(3);element.dataset.transition=t.phase;element.dataset.opacity=t.opacity.toFixed(3);
   return difference>1||Math.abs(goal-energy.current)>.005||t.phase!=='steady'||Object.values(sound).some(v=>v>.005);
  };
  const tick=(now:number)=>{frame=0;if(disposed||document.hidden)return;if(now-last<1000/30){frame=requestAnimationFrame(tick);return;}last=now;const settling=draw(now);if(!reduced&&(playing||settling))frame=requestAnimationFrame(tick);};
  const restart=()=>{if(!frame&&!disposed){previous=0;frame=requestAnimationFrame(tick);}};
  const resize=()=>{const bounds=element.getBoundingClientRect();width=bounds.width;height=bounds.height;const dpr=Math.min(devicePixelRatio||1,2);surface.width=Math.ceil(width*dpr);surface.height=Math.ceil(height*dpr);if(element.width!==surface.width)element.width=surface.width;if(element.height!==surface.height)element.height=surface.height;draw(performance.now());restart();};
  const observer=new ResizeObserver(resize);observer.observe(element);
  const visibility=()=>{if(document.hidden){cancelAnimationFrame(frame);frame=0;}else restart();};
  const motion=()=>{reduced=reduceMotion||preference.matches;restart();};
  document.addEventListener('visibilitychange',visibility);preference.addEventListener('change',motion);resize();
  return()=>{disposed=true;cancelAnimationFrame(frame);observer.disconnect();document.removeEventListener('visibilitychange',visibility);preference.removeEventListener('change',motion);};
 },[palette,playing,enabled,reduceMotion,kickReactive,trackId]);
 if(!enabled)return null;
 return <canvas ref={canvas} className="liquid-glow" aria-hidden="true" data-palette={JSON.stringify(palette)} data-palette-source={fromCover?'cover':'fallback'}/>;
}
