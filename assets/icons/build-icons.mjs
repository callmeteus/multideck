#!/usr/bin/env node
/**
 * Builds the MultiDeck icon set without any external dependency.
 *
 * The mark is rasterised from the same geometry as `assets/brand/logo-mark.svg`
 * with a signed distance field, which gives clean anti-aliased edges at every
 * size. PNG files are written by a small encoder on top of `node:zlib`, and the
 * ICO container embeds those PNGs, which is the format Windows accepts since
 * Vista.
 *
 * Usage: node assets/icons/build-icons.mjs
 */

import { deflateSync } from 'node:zlib';
import { mkdirSync, writeFileSync, statSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

// #region Geometry

const CANVAS = 128;
const PANEL = { x: 4, y: 4, w: 120, h: 120, r: 28 };
const PANEL_BORDER = 1.5;
const KEY = { w: 24, h: 24, r: 7 };
const KEY_BORDER = 1.2;
const COLUMNS = [20, 52, 84];
const ROWS = [36, 68];
const ACCENT_KEY = { column: 0, row: 0 };

const COLOR = {
  panelTop: [0x1b, 0x1f, 0x27],
  panelBottom: [0x11, 0x14, 0x1a],
  panelBorder: [0x3b, 0x43, 0x52],
  keyTop: [0x27, 0x2c, 0x36],
  keyBottom: [0x19, 0x1d, 0x25],
  keyBorder: [0x38, 0x40, 0x4d],
  accentTop: [0x7f, 0xa5, 0xff],
  accentBottom: [0x4a, 0x78, 0xf0],
  accentBorder: [0xa9, 0xc3, 0xff],
  accentGlow: [0x5c, 0x8b, 0xff],
  highlight: [0xff, 0xff, 0xff],
  trayKey: [0xed, 0xf1, 0xf6],
  trayAccent: [0x5c, 0x8b, 0xff],
};

// #endregion

// #region Canvas

/** Creates a transparent RGBA float canvas. */
function createCanvas(size) {
  return { size, pixels: new Float32Array(size * size * 4) };
}

const clamp01 = (value) => (value < 0 ? 0 : value > 1 ? 1 : value);

function mix(from, to, amount) {
  return [
    from[0] + (to[0] - from[0]) * amount,
    from[1] + (to[1] - from[1]) * amount,
    from[2] + (to[2] - from[2]) * amount,
  ];
}

/** Composites one source color over a pixel, straight alpha in, straight alpha out. */
function blend(canvas, index, color, alpha) {
  const sourceAlpha = clamp01(alpha);
  if (sourceAlpha <= 0) {
    return;
  }
  const pixels = canvas.pixels;
  const destinationAlpha = pixels[index + 3];
  const outAlpha = sourceAlpha + destinationAlpha * (1 - sourceAlpha);
  if (outAlpha <= 0) {
    return;
  }
  for (let channel = 0; channel < 3; channel += 1) {
    const source = color[channel] / 255;
    const destination = pixels[index + channel];
    pixels[index + channel] =
      (source * sourceAlpha + destination * destinationAlpha * (1 - sourceAlpha)) / outAlpha;
  }
  pixels[index + 3] = outAlpha;
}

/** Signed distance to a rounded rectangle, negative inside. */
function roundRectDistance(px, py, rect, scale) {
  const centerX = (rect.x + rect.w / 2) * scale;
  const centerY = (rect.y + rect.h / 2) * scale;
  const halfWidth = (rect.w / 2) * scale;
  const halfHeight = (rect.h / 2) * scale;
  const radius = rect.r * scale;
  const qx = Math.abs(px - centerX) - (halfWidth - radius);
  const qy = Math.abs(py - centerY) - (halfHeight - radius);
  const outsideX = Math.max(qx, 0);
  const outsideY = Math.max(qy, 0);
  return Math.hypot(outsideX, outsideY) + Math.min(Math.max(qx, qy), 0) - radius;
}

/** Fills a rounded rectangle, optionally with a vertical gradient. */
function fillRoundRect(canvas, rect, scale, colorAt, alpha = 1) {
  const { size, pixels } = canvas;
  for (let y = 0; y < size; y += 1) {
    for (let x = 0; x < size; x += 1) {
      const distance = roundRectDistance(x + 0.5, y + 0.5, rect, scale);
      const coverage = clamp01(0.5 - distance);
      if (coverage <= 0) {
        continue;
      }
      const index = (y * size + x) * 4;
      blend(canvas, index, colorAt(x + 0.5, y + 0.5), coverage * alpha);
    }
  }
}

/** Draws a stroke centred on the outline of a rounded rectangle. */
function strokeRoundRect(canvas, rect, width, scale, color, alpha = 1) {
  const { size } = canvas;
  const halfWidth = (width * scale) / 2;
  for (let y = 0; y < size; y += 1) {
    for (let x = 0; x < size; x += 1) {
      const distance = Math.abs(roundRectDistance(x + 0.5, y + 0.5, rect, scale));
      const coverage = clamp01(0.5 + halfWidth - distance);
      if (coverage <= 0) {
        continue;
      }
      const index = (y * size + x) * 4;
      blend(canvas, index, color, coverage * alpha);
    }
  }
}

/** Adds a soft accent halo around a key, clipped to the panel. */
function addGlow(canvas, keyRect, scale, panelRect) {
  const { size } = canvas;
  const sigma = Math.max(1.2, 7 * scale);
  for (let y = 0; y < size; y += 1) {
    for (let x = 0; x < size; x += 1) {
      const px = x + 0.5;
      const py = y + 0.5;
      const panelCoverage = clamp01(0.5 - roundRectDistance(px, py, panelRect, scale));
      if (panelCoverage <= 0) {
        continue;
      }
      const distance = Math.max(0, roundRectDistance(px, py, keyRect, scale));
      const intensity = Math.exp(-distance / sigma) * 0.45;
      if (intensity <= 0.002) {
        continue;
      }
      blend(canvas, (y * size + x) * 4, COLOR.accentGlow, intensity * panelCoverage);
    }
  }
}

/** Converts the float canvas to 8 bit RGBA with premultiplied alpha. */
function toRgba(canvas) {
  const { size, pixels } = canvas;
  const output = new Uint8Array(size * size * 4);
  for (let index = 0; index < pixels.length; index += 4) {
    const alpha = clamp01(pixels[index + 3]);
    if (alpha <= 0) {
      continue;
    }
    output[index] = Math.round(clamp01(pixels[index]) * 255);
    output[index + 1] = Math.round(clamp01(pixels[index + 1]) * 255);
    output[index + 2] = Math.round(clamp01(pixels[index + 2]) * 255);
    output[index + 3] = Math.round(alpha * 255);
  }
  return output;
}

// #endregion

// #region Mark

function keyRect(column, row) {
  return {
    x: COLUMNS[column],
    y: ROWS[row],
    w: KEY.w,
    h: KEY.h,
    r: KEY.r,
  };
}

/** Renders the app mark: a rounded deck panel with a 3x2 key grid. */
function renderMark(size) {
  const canvas = createCanvas(size);
  const scale = size / CANVAS;

  fillRoundRect(canvas, PANEL, scale, (_x, y) => {
    const top = PANEL.y * scale;
    const span = PANEL.h * scale;
    const amount = clamp01((y - top) / span);
    return mix(COLOR.panelTop, COLOR.panelBottom, amount);
  });

  addGlow(canvas, keyRect(ACCENT_KEY.column, ACCENT_KEY.row), scale, PANEL);
  strokeRoundRect(canvas, PANEL, PANEL_BORDER, scale, COLOR.panelBorder);

  for (let row = 0; row < ROWS.length; row += 1) {
    for (let column = 0; column < COLUMNS.length; column += 1) {
      const rect = keyRect(column, row);
      const isAccent = column === ACCENT_KEY.column && row === ACCENT_KEY.row;
      fillRoundRect(canvas, rect, scale, (_x, y) => {
        const amount = clamp01((y - rect.y * scale) / (rect.h * scale));
        return isAccent
          ? mix(COLOR.accentTop, COLOR.accentBottom, amount)
          : mix(COLOR.keyTop, COLOR.keyBottom, amount);
      });
      strokeRoundRect(
        canvas,
        rect,
        KEY_BORDER,
        scale,
        isAccent ? COLOR.accentBorder : COLOR.keyBorder,
      );
    }
  }

  const accent = keyRect(ACCENT_KEY.column, ACCENT_KEY.row);
  const highlight = {
    x: accent.x + 5,
    y: accent.y + 5,
    w: accent.w - 10,
    h: 1.6,
    r: 0.8,
  };
  fillRoundRect(canvas, highlight, scale, () => COLOR.highlight, 0.28);

  return toRgba(canvas);
}

/** Renders the tray variant: keys only, on a transparent background. */
function renderTray(size) {
  const canvas = createCanvas(size);
  const unit = size / 32;
  const keySize = 9.2 * unit;
  const gap = 2 * unit;
  const radius = 2.4 * unit;
  const totalWidth = keySize * 3 + gap * 2;
  const totalHeight = keySize * 2 + gap;
  const startX = (size - totalWidth) / 2;
  const startY = (size - totalHeight) / 2;
  const inset = 0.8 * unit;

  for (let row = 0; row < 2; row += 1) {
    for (let column = 0; column < 3; column += 1) {
      const rect = {
        x: startX + column * (keySize + gap) + inset,
        y: startY + row * (keySize + gap) + inset,
        w: keySize - inset * 2,
        h: keySize - inset * 2,
        r: radius,
      };
      const isAccent = column === 0 && row === 0;
      fillRoundRect(
        canvas,
        rect,
        unit,
        () => (isAccent ? COLOR.trayAccent : COLOR.trayKey),
        isAccent ? 1 : 0.92,
      );
    }
  }

  return toRgba(canvas);
}

// #endregion

// #region PNG

const CRC_TABLE = (() => {
  const table = new Uint32Array(256);
  for (let index = 0; index < 256; index += 1) {
    let value = index;
    for (let bit = 0; bit < 8; bit += 1) {
      value = value & 1 ? 0xedb88320 ^ (value >>> 1) : value >>> 1;
    }
    table[index] = value >>> 0;
  }
  return table;
})();

function crc32(buffer) {
  let crc = 0xffffffff;
  for (const byte of buffer) {
    crc = CRC_TABLE[(crc ^ byte) & 0xff] ^ (crc >>> 8);
  }
  return (crc ^ 0xffffffff) >>> 0;
}

function chunk(type, data) {
  const length = Buffer.alloc(4);
  length.writeUInt32BE(data.length, 0);
  const body = Buffer.concat([Buffer.from(type, 'ascii'), data]);
  const checksum = Buffer.alloc(4);
  checksum.writeUInt32BE(crc32(body), 0);
  return Buffer.concat([length, body, checksum]);
}

/** Encodes 8 bit RGBA pixels as a PNG file. */
function encodePng(size, rgba) {
  const header = Buffer.alloc(13);
  header.writeUInt32BE(size, 0);
  header.writeUInt32BE(size, 4);
  header[8] = 8;
  header[9] = 6;
  header[10] = 0;
  header[11] = 0;
  header[12] = 0;

  const stride = size * 4;
  const raw = Buffer.alloc((stride + 1) * size);
  for (let y = 0; y < size; y += 1) {
    raw[y * (stride + 1)] = 0;
    Buffer.from(rgba.buffer, rgba.byteOffset + y * stride, stride).copy(
      raw,
      y * (stride + 1) + 1,
    );
  }

  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk('IHDR', header),
    chunk('IDAT', deflateSync(raw, { level: 9 })),
    chunk('IEND', Buffer.alloc(0)),
  ]);
}

// #endregion

// #region ICO

/** Assembles a PNG-in-ICO container from rendered images. */
function buildIco(images) {
  const count = images.length;
  const directory = Buffer.alloc(6 + count * 16);
  directory.writeUInt16LE(0, 0);
  directory.writeUInt16LE(1, 2);
  directory.writeUInt16LE(count, 4);

  let offset = directory.length;
  for (let index = 0; index < count; index += 1) {
    const { size, data } = images[index];
    const entry = 6 + index * 16;
    directory[entry] = size >= 256 ? 0 : size;
    directory[entry + 1] = size >= 256 ? 0 : size;
    directory[entry + 2] = 0;
    directory[entry + 3] = 0;
    directory.writeUInt16LE(1, entry + 4);
    directory.writeUInt16LE(32, entry + 6);
    directory.writeUInt32LE(data.length, entry + 8);
    directory.writeUInt32LE(offset, entry + 12);
    offset += data.length;
  }

  return Buffer.concat([directory, ...images.map((image) => image.data)]);
}

// #endregion

// #region Build

const HERE = dirname(fileURLToPath(import.meta.url));
const ICO_SIZES = [16, 32, 48, 64, 128, 256];

function write(name, data) {
  const path = join(HERE, name);
  writeFileSync(path, data);
  const bytes = statSync(path).size;
  console.log(`wrote ${name} (${bytes} bytes)`);
}

function main() {
  mkdirSync(HERE, { recursive: true });

  const icoImages = ICO_SIZES.map((size) => ({
    size,
    data: encodePng(size, renderMark(size)),
  }));
  write('icon.ico', buildIco(icoImages));
  write('icon-512.png', encodePng(512, renderMark(512)));
  write('tray-32.png', encodePng(32, renderTray(32)));
}

main();

// #endregion
