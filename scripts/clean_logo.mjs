// 清洗 AI 生成的 Logo 主图（2048x2048）：
//   1) 去掉右下角「豆包AI生成」水印：水印区按几何圆角方块 + 平坦底色重建（设计为平面色块，无折痕穿过该区）
//   2) 白底转透明：按检测到的圆角方块几何做 4x4 超采样 alpha 覆盖率，边缘像素对白色做反混合
// 纯 Node 实现（zlib + 手写 PNG 编解码），无第三方依赖。
// 用法：node scripts/clean_logo.mjs <输入.png> <输出清洗图.png> [app-icon.png 输出路径]
import { deflateSync, inflateSync } from "node:zlib";
import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { dirname } from "node:path";

// ---- PNG 解码（8-bit 非隔行，灰度/RGB/调色板/灰度A/RGBA） ----
function decodePNG(buf) {
  const sig = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
  for (let i = 0; i < 8; i++) if (buf[i] !== sig[i]) throw new Error("not a PNG");
  let pos = 8, width = 0, height = 0, bitDepth = 0, colorType = 0, interlace = 0;
  let plte = null, trns = null;
  const idat = [];
  while (pos < buf.length) {
    const len = buf.readUInt32BE(pos);
    const type = buf.toString("ascii", pos + 4, pos + 8);
    const data = buf.subarray(pos + 8, pos + 8 + len);
    if (type === "IHDR") {
      width = data.readUInt32BE(0);
      height = data.readUInt32BE(4);
      bitDepth = data[8];
      colorType = data[9];
      interlace = data[12];
    } else if (type === "PLTE") plte = data;
    else if (type === "tRNS") trns = data;
    else if (type === "IDAT") idat.push(data);
    else if (type === "IEND") break;
    pos += 12 + len;
  }
  if (bitDepth !== 8 || interlace !== 0) throw new Error(`unsupported PNG: depth=${bitDepth} interlace=${interlace}`);
  const ch = { 0: 1, 2: 3, 3: 1, 4: 2, 6: 4 }[colorType];
  if (!ch) throw new Error(`unsupported colorType ${colorType}`);
  const raw = inflateSync(Buffer.concat(idat));
  const stride = width * ch;
  const out = Buffer.alloc(height * stride);
  let rp = 0;
  for (let y = 0; y < height; y++) {
    const filter = raw[rp++];
    const row = out.subarray(y * stride, (y + 1) * stride);
    const prev = y > 0 ? out.subarray((y - 1) * stride, y * stride) : null;
    for (let x = 0; x < stride; x++) {
      const fv = raw[rp + x];
      const a = x >= ch ? row[x - ch] : 0;
      const b = prev ? prev[x] : 0;
      const c = prev && x >= ch ? prev[x - ch] : 0;
      let v;
      switch (filter) {
        case 0: v = fv; break;
        case 1: v = fv + a; break;
        case 2: v = fv + b; break;
        case 3: v = fv + ((a + b) >> 1); break;
        case 4: {
          const p = a + b - c, pa = Math.abs(p - a), pb = Math.abs(p - b), pc = Math.abs(p - c);
          v = fv + (pa <= pb && pa <= pc ? a : pb <= pc ? b : c);
          break;
        }
        default: throw new Error(`bad filter ${filter}`);
      }
      row[x] = v & 0xff;
    }
    rp += stride;
  }
  // 统一转 RGBA
  const rgba = new Uint8Array(width * height * 4);
  for (let i = 0, j = 0; i < width * height; i++, j += 4) {
    if (colorType === 2) { rgba[j] = out[i * 3]; rgba[j + 1] = out[i * 3 + 1]; rgba[j + 2] = out[i * 3 + 2]; rgba[j + 3] = 255; }
    else if (colorType === 6) { rgba[j] = out[i * 4]; rgba[j + 1] = out[i * 4 + 1]; rgba[j + 2] = out[i * 4 + 2]; rgba[j + 3] = out[i * 4 + 3]; }
    else if (colorType === 0) { rgba[j] = rgba[j + 1] = rgba[j + 2] = out[i]; rgba[j + 3] = 255; }
    else if (colorType === 4) { rgba[j] = rgba[j + 1] = rgba[j + 2] = out[i * 2]; rgba[j + 3] = out[i * 2 + 1]; }
    else if (colorType === 3) { const k = out[i] * 3; rgba[j] = plte[k]; rgba[j + 1] = plte[k + 1]; rgba[j + 2] = plte[k + 2]; rgba[j + 3] = trns ? (out[i] < trns.length ? trns[out[i]] : 255) : 255; }
  }
  return { width, height, rgba };
}

// ---- PNG 编码（RGBA，同 make_icon.mjs） ----
function crc32(buf) {
  let table = crc32.table;
  if (!table) {
    table = crc32.table = new Int32Array(256);
    for (let n = 0; n < 256; n++) {
      let c = n;
      for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
      table[n] = c;
    }
  }
  let c = 0xffffffff;
  for (let i = 0; i < buf.length; i++) c = table[(c ^ buf[i]) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}
function chunk(type, data) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length);
  const typeBuf = Buffer.from(type, "ascii");
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(Buffer.concat([typeBuf, data])));
  return Buffer.concat([len, typeBuf, data, crc]);
}
function encodePNG(width, height, rgba) {
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(width, 0);
  ihdr.writeUInt32BE(height, 4);
  ihdr[8] = 8;
  ihdr[9] = 6;
  const raw = Buffer.alloc(height * (1 + width * 4));
  for (let y = 0; y < height; y++) {
    raw[y * (1 + width * 4)] = 0;
    Buffer.from(rgba.buffer, rgba.byteOffset + y * width * 4, width * 4).copy(raw, y * (1 + width * 4) + 1);
  }
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk("IHDR", ihdr),
    chunk("IDAT", deflateSync(raw, { level: 9 })),
    chunk("IEND", Buffer.alloc(0)),
  ]);
}

// ---- 圆角方块几何 ----
function inRoundRect(x, y, L, T, R, B, r) {
  if (x < L || x > R || y < T || y > B) return false;
  const cx = Math.max(L + r, Math.min(x, R - r));
  const cy = Math.max(T + r, Math.min(y, B - r));
  const dx = x - cx, dy = y - cy;
  return dx * dx + dy * dy <= r * r;
}
// 4x4 超采样覆盖率（x,y 为像素左上角）
function coverage(x, y, geo) {
  const { L, T, R, B, r } = geo;
  let hit = 0;
  for (let j = 0; j < 4; j++) for (let i = 0; i < 4; i++) {
    if (inRoundRect(x + 0.125 + i * 0.25, y + 0.125 + j * 0.25, L, T, R, B, r)) hit++;
  }
  return hit / 16;
}

// 左上角圆弧预测：距顶边 dy 处的左边界 x（dy>=r 为直线边段）
function predictX(dy, L, r) {
  if (dy >= r) return L;
  return L + r - Math.sqrt(Math.max(0, r * r - (r - dy) * (r - dy)));
}

// ---- 主流程 ----
const [inPath, outPath, appIconPath] = process.argv.slice(2);
if (!inPath || !outPath) {
  console.error("usage: node scripts/clean_logo.mjs <input.png> <output_clean.png> [app-icon.png]");
  process.exit(1);
}
const img = decodePNG(readFileSync(inPath));
const { width: W, height: H, rgba } = img;
console.log(`input: ${inPath} (${W}x${H})`);

const px = (x, y) => (y * W + x) * 4;
const isWhite = (x, y) => { const i = px(x, y); return rgba[i] >= 244 && rgba[i + 1] >= 244 && rgba[i + 2] >= 244; };

// 水印/边距污染区：右下角。检测 bbox 与圆角时避开：
//   xR 只统计 y∈[300,1860]；yB 只统计 x∈[300,1620]
const WM_X0 = 1640, WM_Y0 = 1895; // 水印区重建起点（文字约在 1675..2005 / 1932..2010）
const SAMPLE_Y0 = 1875, SAMPLE_Y1 = 1925; // 底色采样带（折痕止于 y≈1870，文字起于 y≈1932）

let yT = H, xL = W, xR = 0, yB = 0;
for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
  if (!isWhite(x, y)) {
    if (y < yT) yT = y;
    if (x < xL) xL = x;
    if (y >= 300 && y <= 1860 && x > xR) xR = x;
    if (x >= 300 && x <= 1620 && y > yB) yB = y;
    break;
  }
}
for (let y = 0; y < H; y++) {
  let last = -1;
  for (let x = W - 1; x >= 0; x--) { if (!isWhite(x, y)) { last = x; break; } }
  if (last >= 0 && y >= 300 && y <= 1860 && last > xR) xR = last;
}
for (let x = 0; x < W; x++) {
  let last = -1;
  for (let y = H - 1; y >= 0; y--) { if (!isWhite(x, y)) { last = y; break; } }
  if (last >= 0 && x >= 300 && x <= 1620 && last > yB) yB = last;
}
console.log(`bbox: x ${xL}..${xR}  y ${yT}..${yB}`);

// 左上角边界扫描拟合圆角半径
const xb = [];
for (let y = yT + 6; y < yT + 700 && y < H; y++) {
  let first = -1;
  for (let x = xL; x < xL + 900 && x < W; x++) { if (!isWhite(x, y)) { first = x; break; } }
  if (first >= 0) xb.push([y, first]);
}
let bestR = 0, bestErr = Infinity;
for (let r = 40; r <= 800; r += 1) {
  let err = 0, n = 0;
  for (const [y, x] of xb) {
    const xp = predictX(y - yT, xL, r);
    err += Math.abs(xp - x); n++;
  }
  if (n > 20 && err / n < bestErr) { bestErr = err / n; bestR = r; }
}
console.log(`corner radius: ${bestR} (mean fit err ${bestErr.toFixed(2)}px)`);
const geo = { L: xL, T: yT, R: xR, B: yB, r: bestR };

// 底色采样：重建区内每列的平坦紫（y 1875..1925 内、贴近紫色的像素取中位数）
const purpleAt = [];
let gR = [], gG = [], gB = [];
for (let x = WM_X0; x < W; x++) {
  const rs = [], gs = [], bs = [];
  for (let y = SAMPLE_Y0; y <= SAMPLE_Y1; y++) {
    const i = px(x, y);
    const R = rgba[i], G = rgba[i + 1], B = rgba[i + 2];
    if (R < 170 && B > 120 && Math.abs(R - 104) < 30) { rs.push(R); gs.push(G); bs.push(B); }
  }
  if (rs.length > 10) {
    rs.sort((a, b) => a - b); gs.sort((a, b) => a - b); bs.sort((a, b) => a - b);
    purpleAt[x] = [rs[rs.length >> 1], gs[gs.length >> 1], bs[bs.length >> 1]];
    gR.push(purpleAt[x][0]); gG.push(purpleAt[x][1]); gB.push(purpleAt[x][2]);
  }
}
gR.sort((a, b) => a - b); gG.sort((a, b) => a - b); gB.sort((a, b) => a - b);
const globalPurple = [gR[gR.length >> 1], gG[gG.length >> 1], gB[gB.length >> 1]];
console.log(`flat purple estimate: rgb(${globalPurple})`);
// 缺列用邻近列插值
let lastValid = -1;
for (let x = WM_X0; x < W; x++) {
  if (purpleAt[x]) lastValid = x;
  else if (lastValid >= 0) purpleAt[x] = purpleAt[lastValid];
}
for (let x = W - 1; x >= WM_X0; x--) {
  if (!purpleAt[x]) purpleAt[x] = purpleAt[x + 1] || globalPurple;
}

// 重建水印区：按几何覆盖率把平坦紫合成到白底上（与真实光栅化边缘一致）
for (let y = WM_Y0; y < H; y++) {
  for (let x = WM_X0; x < W; x++) {
    const a = coverage(x, y, geo);
    const [pr, pg, pb] = purpleAt[x];
    const i = px(x, y);
    rgba[i] = Math.round(pr * a + 255 * (1 - a));
    rgba[i + 1] = Math.round(pg * a + 255 * (1 - a));
    rgba[i + 2] = Math.round(pb * a + 255 * (1 - a));
    rgba[i + 3] = 255;
  }
}

// 白底转透明：几何覆盖率作 alpha，边缘对白色反混合还原本色
for (let y = 0; y < H; y++) {
  for (let x = 0; x < W; x++) {
    const a = coverage(x, y, geo);
    const i = px(x, y);
    if (a >= 0.996) {
      rgba[i + 3] = 255;
    } else if (a <= 0.004) {
      rgba[i] = rgba[i + 1] = rgba[i + 2] = 0; rgba[i + 3] = 0;
    } else {
      for (let k = 0; k < 3; k++) {
        rgba[i + k] = Math.max(0, Math.min(255, Math.round((rgba[i + k] - 255 * (1 - a)) / a)));
      }
      rgba[i + 3] = Math.round(a * 255);
    }
  }
}

const png = encodePNG(W, H, rgba);
mkdirSync(dirname(outPath), { recursive: true });
writeFileSync(outPath, png);
console.log("clean written:", outPath);
if (appIconPath) {
  mkdirSync(dirname(appIconPath), { recursive: true });
  writeFileSync(appIconPath, png);
  console.log("app-icon written:", appIconPath);
}
