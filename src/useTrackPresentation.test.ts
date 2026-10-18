import {describe,it,expect} from 'vitest';
import {trackReady} from './useTrackPresentation';
import type {MediaState} from './media';

describe('готовность карточки трека',()=>{
 const ready={sourceId:'ru.yandex.desktop.music',connected:true,title:'Трек',artist:'Исполнитель',status:'Playing'} as MediaState;
 it('не принимает служебный заголовок Яндекса за готовый трек',()=>{
  expect(trackReady({...ready,title:'Яндекс Музыка — собираем музыку для вас',artist:'',status:'Paused'})).toBe(false);
  expect(trackReady(ready)).toBe(true);
 });
 it('ждёт воспроизведения, но допускает готовый трек на паузе',()=>{
  expect(trackReady({...ready,status:'Opened'})).toBe(false);
  expect(trackReady({...ready,connected:false})).toBe(false);
  expect(trackReady({...ready,status:'Paused'})).toBe(true);
 });
});
