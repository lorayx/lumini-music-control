export type AudioBands={active?:boolean;kick?:number;bass?:number;sub?:number;mid?:number;treble?:number;timestamp?:number};
export type MotionBands={kick:number;bass:number;sub:number;mid:number;treble:number};
const zero=():MotionBands=>({kick:0,bass:0,sub:0,mid:0,treble:0});
const clamp=(n:unknown)=>typeof n==='number'&&Number.isFinite(n)?Math.max(0,Math.min(1,n)):0;

/** Audio stays fast; render envelopes retain a quiet, liquid release. */
export class AudioMotion{
 private input=zero();private value=zero();private received=-Infinity;private stamp=0;
 receive(sample:AudioBands|null,now:number,wallTime:number){
  if(!sample?.active){this.input=zero();this.received=now;return;}
  const age=wallTime-(sample.timestamp??wallTime);
  if(!Number.isFinite(age)||age>450||age< -1000)return;
  if(sample.timestamp&&sample.timestamp<this.stamp)return;
  this.stamp=sample.timestamp??wallTime;this.received=now;
  this.input={kick:clamp(sample.kick),bass:clamp(sample.bass),sub:clamp(sample.sub),mid:clamp(sample.mid),treble:clamp(sample.treble)};
 }
 step(now:number,dt:number,enabled:boolean):MotionBands{
  const fresh=enabled&&now-this.received<450;
  const elapsed=Number.isFinite(dt)?Math.max(0,Math.min(1,dt)):0;
  for(const key of Object.keys(this.value) as (keyof MotionBands)[]){
   const target=fresh?this.input[key]:0;
   const attack=key==='kick'?.008:key==='treble'?.065:.045;
   const release=key==='kick'?.18:key==='treble'?.28:.24;
   this.value[key]+=(target-this.value[key])*(1-Math.exp(-elapsed/(target>this.value[key]?attack:release)));
  }
  return {...this.value};
 }
 reset(){this.input=zero();this.value=zero();this.received=-Infinity;this.stamp=0;}
}
