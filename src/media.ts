export interface MediaState {
  sourceId:string; connected: boolean; title: string; artist: string; album: string; cover: string | null;
  status: string; controls: {play:boolean;pause:boolean;next:boolean;previous:boolean;seek:boolean};
  position: number; start: number; end: number; minSeek: number; maxSeek: number; capturedAt: number; error: string|null;
}
export interface Settings {theme:'system'|'light'|'dark';hotkeys:boolean;hotkey:string;reduceMotion:boolean;visualizer:boolean;kickReactive:boolean;taskbar:boolean;taskbarOffset:number;playerId:string;launchAppId:string;playerName:string;launchOnPlay:boolean;minimizeOnLaunch:boolean;bootstrapYandex:boolean}
export type Action = 'play'|'pause'|'next'|'previous'|{seek:number};
export function projectedPosition(media: MediaState, now: number): number {
  if (!media.connected || media.end <= media.start) return media.start;
  const elapsed = media.status === 'Playing' ? Math.max(0,(now-media.capturedAt)/1000) : 0;
  return Math.max(media.start,Math.min(media.end,media.position+elapsed));
}
export function clock(seconds:number):string {
  if (!Number.isFinite(seconds) || seconds<0) return '0:00';
  const total = Math.floor(seconds); return `${Math.floor(total/60)}:${String(total%60).padStart(2,'0')}`;
}
