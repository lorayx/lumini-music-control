import React,{useEffect,useState} from 'react';
import {invoke} from '@tauri-apps/api/core';
import type {Settings} from './media';
type Player={name:string;appId:string};
export function PlayerSettings({settings,save,busy}:{settings:Settings;save:(s:Settings)=>Promise<unknown>;busy:boolean}){
 const [players,setPlayers]=useState<Player[]>([]),[sessions,setSessions]=useState<string[]>([]),[loading,setLoading]=useState(false),[error,setError]=useState(''),[all,setAll]=useState(false);
 const refresh=async()=>{setLoading(true);setError('');try{const [p,s]=await Promise.all([invoke<Player[]>('get_players'),invoke<string[]>('get_active_players')]);setPlayers(p);setSessions(s);}catch(e){setError(String(e));}finally{setLoading(false);}};
 useEffect(()=>{void refresh();},[]);
 const visible=players.filter(p=>all||p.appId===settings.launchAppId||/music|музык|spotify|vlc|aimp|foobar|deezer|tidal|winamp/i.test(p.name));
 return <section className="player-settings" aria-label="Музыкальное приложение">
  <label htmlFor="music-player">Музыкальное приложение</label>
  <select id="music-player" disabled={busy||loading} value={settings.launchAppId} onChange={e=>{const player=players.find(p=>p.appId===e.target.value);if(player)void save({...settings,launchAppId:player.appId,playerId:player.appId,playerName:player.name});}}>
   {!visible.some(p=>p.appId===settings.launchAppId)&&<option value={settings.launchAppId}>{settings.playerName}</option>}
   {visible.map(p=><option key={p.appId} value={p.appId}>{p.name}</option>)}
  </select>
  <label className="all-players"><input type="checkbox" checked={all} onChange={e=>setAll(e.target.checked)}/>Показать все приложения</label>
  <label className="setting"><span><b>Запускать плеер по Play</b><small>Если он закрыт, открыть и дождаться музыки</small></span><input type="checkbox" checked={settings.launchOnPlay} disabled={busy} onChange={e=>void save({...settings,launchOnPlay:e.target.checked})}/></label>
  <label className="setting"><span><b>Запускать в фоне</b><small>Сворачивать новое окно. Плеер может кратко показать его</small></span><input type="checkbox" checked={settings.minimizeOnLaunch} disabled={busy} onChange={e=>void save({...settings,minimizeOnLaunch:e.target.checked})}/></label>
  {settings.launchAppId==="ru.yandex.desktop.music"&&<label className="setting"><span><b>Первый Play в Яндекс Музыке</b><small>Если очередь пуста, нажать Play в самом плеере через Windows Accessibility</small></span><input type="checkbox" checked={settings.bootstrapYandex} disabled={busy} onChange={e=>void save({...settings,bootstrapYandex:e.target.checked})}/></label>}
  <details><summary>Если другой плеер не определяется</summary>
   <p>Один раз включите в нём музыку, обновите список и выберите его системную медиасессию. Браузер автоматически не выбирается.</p>
   <select aria-label="Медиасессия выбранного плеера" disabled={busy||loading} value={settings.playerId} onChange={e=>void save({...settings,playerId:e.target.value})}>
    {!sessions.includes(settings.playerId)&&<option value={settings.playerId}>{settings.playerName} — сохранённый выбор</option>}
    {sessions.map(id=><option key={id} value={id}>{players.find(p=>p.appId===id)?.name||id}</option>)}
   </select>
   <button className="refresh-players" disabled={busy||loading} onClick={()=>void refresh()}>{loading?'Обновляем…':'Обновить список'}</button>
  </details>
  {error&&<p className="player-warning" role="status">{error}</p>}
  <p className="player-hint">Продолжение зависит от плеера: может понадобиться вход или первый выбор трека. Настройки сами музыку не запускают.</p>
 </section>;
}
