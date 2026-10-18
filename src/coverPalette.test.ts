import {describe,it,expect} from 'vitest';
import {paletteFromPixels,NEUTRAL_PALETTE} from './coverPalette';
const image=(colors:number[][])=>Uint8ClampedArray.from(colors.flatMap(c=>[...c,255]));
describe('цвета обложки',()=>{
 it('различает красную и синюю обложки, приглушая яркость',()=>{
  const red=paletteFromPixels(image(Array(60).fill([240,30,20])));
  const blue=paletteFromPixels(image(Array(60).fill([20,40,240])));
  expect(red[0][0]).toBeGreaterThan(red[0][2]);expect(blue[0][2]).toBeGreaterThan(blue[0][0]);
  for(const color of [...red,...blue])expect(Math.max(...color)-Math.min(...color)).toBeLessThanOrEqual(70);
 });
 it('не окрашивает чёрно-белые обложки посторонними оттенками',()=>{
  const palette=paletteFromPixels(image([...Array(40).fill([0,0,0]),...Array(40).fill([255,255,255]),...Array(30).fill([110,110,110])]));
  expect(palette.every(c=>c[0]===c[1]&&c[1]===c[2])).toBe(true);
 });
 it('сохраняет цветной объект среди белых полей и допускает прозрачную обложку',()=>{
  const palette=paletteFromPixels(image([...Array(100).fill([255,255,255]),...Array(35).fill([30,180,70])]));
  expect(palette.some(c=>c[1]>c[0]+20)).toBe(true);
  expect(paletteFromPixels([10,20,30,0])).toEqual(NEUTRAL_PALETTE);
 });
});
