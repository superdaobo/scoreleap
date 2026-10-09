export const COLORS = {
  bg: '#050505',
  surface: '#131313',
  surface2: '#201f1f',
  surface3: '#2a2a2a',
  text: '#e5e2e1',
  muted: '#999077',
  gold: '#ffd700',
  goldDim: '#e9c400',
  goldSoft: '#ffe16d',
  green: '#b4cdba',
  greenDeep: '#364c3d',
  greenLine: '#2ecc71',
  red: '#ff6b6b',
} as const;

export const FONT = {
  display: '"Playfair Display", "Microsoft YaHei", serif',
  body: 'Geist, "Microsoft YaHei", "Segoe UI", sans-serif',
  mono: '"JetBrains Mono", "Cascadia Mono", Consolas, monospace',
} as const;

// 第五人格段落使用的第二色板：荒诞哥特 / 旧纸张 / 暗红褐 / 旧金属 / 烛光。
// 只作为 Identity V 段的 secondary，不覆盖主片的深黑+金+绿。
export const GOTHIC = {
  parchment: '#d9c9a3',
  parchmentSoft: '#c7b48c',
  parchmentDim: '#9c8a68',
  parchmentDark: '#5a4c33',
  burgundy: '#7a2b2b',
  wine: '#541b22',
  umber: '#2a1a12',
  candle: '#ffd9a0',
  candleDim: '#c98d4f',
  agedMetal: '#6f6654',
  bone: '#efe6d2',
} as const;

// 真实 identity-v Profile：MIDI 48..=83，共 36 键。
export const PROFILE_KEYS = {
  midiLow: 48,
  midiHigh: 83,
  keys: 36,
  maxPolyphony: 4,
} as const;

// identity-v Profile 真实键位（Scan Code Set 1 → 键盘字面），来自
// apps/scoreleap/src-tauri/resources/game-profiles/identity-v/windows-keymap.json。
export const IDENTITY_V_LABELS: Record<number, string> = {
  48: ',',
  49: 'L',
  50: '.',
  51: ';',
  52: '/',
  53: 'I',
  54: '9',
  55: 'O',
  56: '0',
  57: 'P',
  58: '-',
  59: '[',
  60: 'Z',
  61: 'S',
  62: 'X',
  63: 'D',
  64: 'C',
  65: 'V',
  66: 'G',
  67: 'B',
  68: 'H',
  69: 'N',
  70: 'J',
  71: 'M',
  72: 'Q',
  73: '2',
  74: 'W',
  75: '3',
  76: 'E',
  77: 'R',
  78: '5',
  79: 'T',
  80: '6',
  81: 'Y',
  82: '7',
  83: 'U',
} as const;

export const isBlackKey = (pitch: number) => [1, 3, 6, 8, 10].includes(pitch % 12);

export const SAFE_X = 112;
export const SAFE_Y = 72;
