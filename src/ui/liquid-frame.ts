const HEADER_BYTES = 12;
const MAX_PIXELS = 1_048_576;
/** Validate dimensions before exposing the native RGBA buffer; no Blob or decoder. */
export function decodeLiquidFrame(data: ArrayBuffer, width: number, height: number): Uint8ClampedArray {
  if (data.byteLength < HEADER_BYTES) throw Error("Truncated Liquid frame");
  const v = new DataView(data);
  if (v.getUint32(0, false) !== 0x4c513031 || !Number.isInteger(width) || !Number.isInteger(height)
      || width <= 0 || height <= 0 || width * height > MAX_PIXELS
      || v.getUint32(4, true) !== width || v.getUint32(8, true) !== height
      || data.byteLength !== HEADER_BYTES + width * height * 4) {
    throw Error("Invalid Liquid frame");
  }
  return new Uint8ClampedArray(data, HEADER_BYTES);
}

const WASH_MIN = .34, WASH_MAX = .72;
const LUMA_BINS = 16, LUMA_SAMPLES = 256, LUMA_PERCENTILE = .95;
/** Dark scenes stay clear; bright areas receive enough smoke to keep white text readable. */
export function liquidWash(pixels: Uint8ClampedArray): number {
  const bins = new Uint16Array(LUMA_BINS);
  const stride = Math.max(1,Math.floor(pixels.length / 4 / LUMA_SAMPLES)) * 4;
  let count=0;
  for(let i=0;i<pixels.length;i+=stride) {
    const light=pixels[i]*.2126+pixels[i+1]*.7152+pixels[i+2]*.0722;
    bins[Math.min(LUMA_BINS-1,Math.floor(light/LUMA_BINS))]++;count++;
  }
  let cumulative=0,bin=0;
  for(;bin<LUMA_BINS-1;bin++) { cumulative+=bins[bin];if(cumulative>=count*LUMA_PERCENTILE)break; }
  return WASH_MIN+(WASH_MAX-WASH_MIN)*bin/(LUMA_BINS-1);
}
