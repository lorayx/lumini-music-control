import {describe,it,expect} from 'vitest';
import {projectedPosition,clock,type MediaState} from './media';
const media = {sourceId:"ru.yandex.desktop.music",connected:true,status:'Playing',start:0,end:180,position:10,capturedAt:1000} as MediaState;
describe('Windows timeline',()=>{
  it('interpolates from LastUpdatedTime and clamps to actual duration',()=>{
    expect(projectedPosition(media,6000)).toBe(15); expect(projectedPosition(media,999999)).toBe(180);
  });
  it('does not advance paused or disconnected media',()=>{
    expect(projectedPosition({...media,status:'Paused'},9000)).toBe(10);
    expect(projectedPosition({...media,connected:false},9000)).toBe(0);
  });
  it('handles invalid and nonzero timelines',()=>{
    expect(projectedPosition({...media,start:5,end:0},5000)).toBe(5);
    expect(clock(NaN)).toBe('0:00'); expect(clock(331.4)).toBe('5:31');
  });
});
