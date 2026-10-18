export type RGB=[number,number,number];
export type Palette=[RGB,RGB,RGB];
export const NEUTRAL_PALETTE:Palette=[[133,149,149],[168,149,146],[166,158,141]];
const distance=(a:RGB,b:RGB)=>a.reduce((sum,v,i)=>sum+(v-b[i])**2,0);
function soften(rgb:RGB):RGB {
 const grey=rgb[0]*.2126+rgb[1]*.7152+rgb[2]*.0722;
 const range=Math.max(...rgb)-Math.min(...rgb),chroma=range>88?88/range:1;
 const luminance=86+grey*.40;
 return rgb.map(v=>Math.round(Math.max(48,Math.min(211,luminance+(v-grey)*chroma*.78)))) as RGB;
}
// Small deterministic weighted clustering. Dark/white margins cannot drown out
// the coloured subject; monochrome artwork remains monochrome.
export function paletteFromPixels(pixels:ArrayLike<number>):Palette {
 const buckets=new Map<number,{rgb:RGB;weight:number}>();
 for(let i=0;i+3<pixels.length;i+=4){
  if(pixels[i+3]<128)continue;
  const rgb:RGB=[pixels[i],pixels[i+1],pixels[i+2]],light=(rgb[0]+rgb[1]+rgb[2])/3;
  const weight=light<16||light>242?.12:1;
  const key=(rgb[0]>>4)*256+(rgb[1]>>4)*16+(rgb[2]>>4);
  const bucket=buckets.get(key);
  if(bucket){const total=bucket.weight+weight;bucket.rgb=bucket.rgb.map((v,k)=>(v*bucket.weight+rgb[k]*weight)/total) as RGB;bucket.weight=total;}
  else buckets.set(key,{rgb,weight});
 }
 const samples=[...buckets.values()].sort((a,b)=>b.weight-a.weight);
 if(!samples.length)return NEUTRAL_PALETTE.map(c=>[...c]) as Palette;
 const centers:RGB[]=[[...samples[0].rgb]];
 while(centers.length<3){const farthest=[...samples].sort((a,b)=>Math.min(...centers.map(c=>distance(b.rgb,c)))*Math.sqrt(b.weight)-Math.min(...centers.map(c=>distance(a.rgb,c)))*Math.sqrt(a.weight))[0];centers.push([...farthest.rgb]);}
 let weights=[0,0,0];
 for(let iteration=0;iteration<6;iteration++){
  const sums:RGB[]=[[0,0,0],[0,0,0],[0,0,0]];weights=[0,0,0];
  for(const sample of samples){let index=0;for(let k=1;k<3;k++)if(distance(sample.rgb,centers[k])<distance(sample.rgb,centers[index]))index=k;weights[index]+=sample.weight;for(let k=0;k<3;k++)sums[index][k]+=sample.rgb[k]*sample.weight;}
  centers.forEach((_,i)=>{if(weights[i])centers[i]=sums[i].map(v=>v/weights[i]) as RGB;});
 }
 return centers.map((rgb,i)=>({rgb,weight:weights[i]})).sort((a,b)=>b.weight-a.weight).map(c=>soften(c.rgb)) as Palette;
}
const cache=new Map<string,Palette>();
export async function coverPalette(cover:string|null):Promise<Palette>{
 if(!cover||!/^data:image\/(png|jpeg);base64,/.test(cover))return NEUTRAL_PALETTE;
 const cached=cache.get(cover);if(cached)return cached;
 const image=new Image();image.src=cover;await image.decode();
 const canvas=document.createElement('canvas');canvas.width=canvas.height=48;
 const ctx=canvas.getContext('2d',{willReadFrequently:true});if(!ctx)return NEUTRAL_PALETTE;
 ctx.drawImage(image,0,0,48,48);const palette=paletteFromPixels(ctx.getImageData(0,0,48,48).data);
 if(cache.size>=8)cache.delete(cache.keys().next().value!);cache.set(cover,palette);return palette;
}
