import type {CSSProperties, ReactNode} from 'react';
import {
  AudioLines,
  Check,
  CircleCheck,
  Club,
  Clock,
  Diamond,
  FileMusic,
  Flame,
  Heart,
  Keyboard,
  Lock,
  Music2,
  Play,
  ShieldCheck,
  SlidersHorizontal,
  Spade,
  Sparkles,
  Wand2,
  Zap,
} from 'lucide-react';
import {
  AbsoluteFill,
  OffthreadVideo,
  interpolate,
  spring,
  staticFile,
  useCurrentFrame,
  useVideoConfig,
} from 'remotion';
import {
  AppWindow,
  LogoMark,
  PianoRoll,
  Stat,
  TechTag,
  Waveform,
} from './components';
import {COLORS, FONT, GOTHIC, IDENTITY_V_LABELS, PROFILE_KEYS, SAFE_X, SAFE_Y, isBlackKey} from './theme';
import type {TrailerProps, VideoNote} from './types';
import {Cursor, clamp, cursorAt, pageSlide, pressedStyle} from './transitions';

// ---------------------------------------------------------------------------
// 应用流程场景的时间轴（场景内帧，60fps，共 40s）
// ---------------------------------------------------------------------------

const F = {
  winEnter: 0,
  txMove: 60,
  txClick: 158,
  confirmOpen: 205,
  confirmSettle: 250,
  startMove: 320,
  startClick: 355,
  confirmClose: 360,
  progressCardIn: 405,
  progressRun: 445,
  progressDone: 1020,
  resultBanner: 1055,
  newCard: 1095,
  cardMove: 1170,
  cardClick: 1200,
  arrangeIn: 1245,
  arrangeSettle: 1305,
  profileOpen: 1370,
  profilePick: 1430,
  paramsSet: 1500,
  compileClick: 1620,
  compileDone: 1735,
  statsIn: 1755,
  playMove: 1800,
  playClick: 1840,
  previewStart: 1875,
  previewEnd: 2140,
  settingsIn: 2140,
  settingsSettle: 2200,
  engineMove: 2250,
  engineFast: 2270,
  engineBackMove: 2340,
  engineHQ: 2360,
  privacyNote: 2380,
  sceneEnd: 2400,
};

const WIN = {x: 210, y: 150, w: 1500, h: 780};
const SIDEBAR_W = 250;
const CONTENT_X = WIN.x + SIDEBAR_W;
const CONTENT_Y = WIN.y + 50;
const CONTENT_W = WIN.w - SIDEBAR_W;
const CONTENT_H = WIN.h - 50;

const noteStats = (notes: VideoNote[]) => {
  const pitches = notes.map((n) => n.pitch);
  return {
    count: notes.length,
    min: pitches.length ? Math.min(...pitches) : 0,
    max: pitches.length ? Math.max(...pitches) : 0,
    playable: notes.filter((n) => n.pitch >= PROFILE_KEYS.midiLow && n.pitch <= PROFILE_KEYS.midiHigh).length,
  };
};

// ---------------------------------------------------------------------------
// 片头 Hook：一句话直接卖点
// ---------------------------------------------------------------------------

export const HookScene = () => {
  const frame = useCurrentFrame();
  const {fps} = useVideoConfig();
  const titleP = interpolate(frame, [8, 30], [0, 1], clamp);
  const chipP = interpolate(frame, [36, 60], [0, 1], clamp);
  const lineP = interpolate(frame, [60, 110], [0, 1], clamp);
  const subP = interpolate(frame, [70, 110], [0, 1], clamp);
  const glow = 0.4 + Math.sin(frame / 20) * 0.08;
  return (
    <div style={{position: 'absolute', inset: 0, background: COLORS.bg, color: COLORS.text, overflow: 'hidden'}}>
      <div
        style={{
          position: 'absolute',
          inset: 0,
          backgroundImage:
            'linear-gradient(rgba(46,204,113,.05) 1px, transparent 1px), linear-gradient(90deg, rgba(46,204,113,.05) 1px, transparent 1px)',
          backgroundSize: '32px 32px',
        }}
      />
      <div
        style={{
          position: 'absolute',
          inset: 0,
          background: `radial-gradient(circle at 50% 42%, rgba(255,215,0,${0.09 * glow}) 0%, transparent 40%), radial-gradient(circle at 84% 76%, rgba(46,204,113,.07) 0%, transparent 34%)`,
        }}
      />
      <div style={{position: 'absolute', left: SAFE_X, top: SAFE_Y}}>
        <TechTag>SCORELEAP · LOCAL AI</TechTag>
      </div>
      <div
        style={{
          position: 'absolute',
          left: SAFE_X,
          right: SAFE_X,
          top: 300,
          textAlign: 'center',
          opacity: titleP,
          transform: `translateY(${interpolate(titleP, [0, 1], [34, 0])}px)`,
        }}
      >
        <div
          style={{
            fontFamily: FONT.display,
            fontWeight: 700,
            fontSize: 92,
            lineHeight: 1.04,
            letterSpacing: '-0.035em',
          }}
        >
          把 MP3，变成
          <br />
          <span style={{color: COLORS.gold, textShadow: `0 0 60px ${COLORS.gold}${'44'}`}}>第五人格能弹</span>的曲谱。
        </div>
      </div>
      <div
        style={{
          position: 'absolute',
          left: '50%',
          top: 640,
          transform: 'translateX(-50%)',
          display: 'flex',
          alignItems: 'center',
          gap: 12,
          opacity: chipP,
        }}
      >
        <span
          style={{
            fontFamily: FONT.mono,
            fontSize: 15,
            letterSpacing: '.12em',
            color: COLORS.gold,
            border: `1px solid ${COLORS.gold}55`,
            background: 'rgba(19,19,19,.85)',
            padding: '9px 16px',
          }}
        >
          第五人格 · 36 键钢琴 / IDENTITY V PROFILE
        </span>
      </div>
      <div
        style={{
          position: 'absolute',
          left: '50%',
          top: 730,
          width: 760 * lineP,
          height: 1,
          transform: 'translateX(-50%)',
          background: `linear-gradient(90deg, transparent, ${COLORS.gold}, transparent)`,
        }}
      />
      <div
        style={{
          position: 'absolute',
          left: 0,
          right: 0,
          bottom: 90,
          textAlign: 'center',
          fontFamily: FONT.mono,
          fontSize: 15,
          letterSpacing: '.16em',
          color: COLORS.muted,
          opacity: subP,
        }}
      >
        本地转录 → 36 键编排 → 一键演奏
      </div>
    </div>
  );
};

// ---------------------------------------------------------------------------
// 36 键琴盘（真实 identity-v 键位映射；用于编排预览与 IdentityV 段）
// ---------------------------------------------------------------------------

export const IdentityVKeyboard = ({
  notes,
  t,
  width = 1000,
  height = 150,
  gothic = false,
  labelAbove = true,
}: {
  notes: VideoNote[];
  t: number;
  width?: number;
  height?: number;
  gothic?: boolean;
  labelAbove?: boolean;
}) => {
  const whites: number[] = [];
  for (let p = PROFILE_KEYS.midiLow; p <= PROFILE_KEYS.midiHigh; p++) {
    if (!isBlackKey(p)) whites.push(p);
  }
  const whiteW = width / whites.length;
  const active = new Set(notes.filter((n) => n.start <= t && n.end >= t).map((n) => n.pitch));
  let whiteIdx = 0;
  const keys: {pitch: number; x: number; w: number; h: number; black: boolean}[] = [];
  for (let p = PROFILE_KEYS.midiLow; p <= PROFILE_KEYS.midiHigh; p++) {
    if (isBlackKey(p)) {
      const leftWhite = whites.indexOf(p - 1);
      const x = (leftWhite + 1) * whiteW - whiteW * 0.3;
      keys.push({pitch: p, x, w: whiteW * 0.6, h: height * 0.62, black: true});
    } else {
      keys.push({pitch: p, x: whiteIdx * whiteW, w: whiteW, h: height, black: false});
      whiteIdx++;
    }
  }
  return (
    <div style={{position: 'relative', width, height}}>
      {keys.map((k) => {
        const on = active.has(k.pitch);
        const face = gothic
          ? on
            ? GOTHIC.candle
            : k.black
              ? '#5a4430'
              : GOTHIC.parchmentSoft
          : on
            ? COLORS.goldSoft
            : k.black
              ? '#2c2c2c'
              : '#f2efe9';
        const edge = gothic ? (on ? GOTHIC.candleDim : '#3a2c1c') : on ? COLORS.gold : '#b9b2a4';
        return (
          <div
            key={k.pitch}
            style={{
              position: 'absolute',
              left: k.x,
              top: k.black ? 0 : 0,
              width: k.w,
              height: k.h,
              background: face,
              border: `1px solid ${edge}`,
              borderRadius: k.black ? '0 0 3px 3px' : '0 0 4px 4px',
              boxShadow: on
                ? gothic
                  ? `0 0 ${22}px ${GOTHIC.candle}66`
                  : `0 0 ${22}px ${COLORS.gold}66`
                : k.black
                  ? '0 3px 6px rgba(0,0,0,.55)'
                  : '0 3px 0 rgba(0,0,0,.5)',
              display: 'flex',
              alignItems: k.black ? 'center' : 'flex-end',
              justifyContent: 'center',
              paddingBottom: k.black ? 0 : 10,
              zIndex: k.black ? 2 : 1,
            }}
          >
            <span
              style={{
                fontFamily: FONT.mono,
                fontSize: k.black ? Math.max(8, height * 0.085) : Math.max(9, height * 0.13),
                color: on
                  ? gothic
                    ? '#2a1a12'
                    : COLORS.bg
                  : k.black
                    ? gothic
                      ? GOTHIC.parchmentSoft
                      : '#c8c2b6'
                    : gothic
                      ? '#4a3826'
                      : '#7a766c',
                fontWeight: 700,
                textShadow: k.black && gothic ? '0 1px 2px rgba(0,0,0,.8)' : 'none',
              }}
            >
              {IDENTITY_V_LABELS[k.pitch]}
            </span>
          </div>
        );
      })}
      {labelAbove ? (
        <div
          style={{
            position: 'absolute',
            right: 0,
            top: -34,
            fontFamily: FONT.mono,
            fontSize: 12,
            letterSpacing: '.1em',
            color: gothic ? GOTHIC.candle : COLORS.gold,
          }}
        >
          {PROFILE_KEYS.keys} KEYS · MIDI {PROFILE_KEYS.midiLow}—{PROFILE_KEYS.midiHigh}
        </div>
      ) : null}
    </div>
  );
};

// ---------------------------------------------------------------------------
// 应用流程主场景：同一个 AppWindow 内的产品使用流程
// ---------------------------------------------------------------------------

const NavSidebar = ({frame}: {frame: number}) => {
  const items = [
    {id: 'library', label: '曲谱库', icon: <Music2 size={18} strokeWidth={1.4} />},
    {id: 'arrange', label: '编排', icon: <SlidersHorizontal size={18} strokeWidth={1.4} />},
    {id: 'settings', label: '设置', icon: <Keyboard size={18} strokeWidth={1.4} />},
  ];
  const navTop =
    frame < F.arrangeIn
      ? 0
      : frame < F.arrangeIn + 24
        ? interpolate(frame, [F.arrangeIn, F.arrangeIn + 24], [0, 52], clamp)
        : frame < F.settingsIn
          ? 52
          : frame < F.settingsIn + 24
            ? interpolate(frame, [F.settingsIn, F.settingsIn + 24], [52, 104], clamp)
            : 104;
  const activeIndex = navTop <= 26 ? 0 : navTop <= 78 ? 1 : 2;
  return (
    <div
      style={{
        position: 'absolute',
        left: 0,
        top: 0,
        bottom: 0,
        width: SIDEBAR_W,
        borderRight: '1px solid rgba(46,204,113,.14)',
        background: 'rgba(9,9,9,.9)',
        padding: '26px 18px',
        zIndex: 20,
      }}
    >
      <LogoMark size={40} />
      <div style={{position: 'relative', marginTop: 46}}>
        <div
          style={{
            position: 'absolute',
            left: 0,
            width: 208,
            height: 46,
            top: navTop,
            border: `1px solid ${COLORS.gold}55`,
            background: 'rgba(255,215,0,.06)',
            boxShadow: `0 0 26px ${COLORS.gold}12`,
          }}
        />
        {items.map((item, i) => (
          <div
            key={item.id}
            style={{
              position: 'relative',
              width: 208,
              height: 46,
              display: 'flex',
              alignItems: 'center',
              gap: 12,
              padding: '0 16px',
              marginTop: i === 0 ? 0 : 6,
              color: i === activeIndex ? COLORS.gold : COLORS.muted,
              fontFamily: FONT.body,
              fontSize: 16,
            }}
          >
            {item.icon}
            {item.label}
          </div>
        ))}
      </div>
    </div>
  );
};

const LibraryPage = ({frame, props}: {frame: number; props: TrailerProps}) => {
  const {fps} = useVideoConfig();
  const stats = noteStats(props.notes);
  const songTitle = (props.sampleLabel || '花海').split(' - ')[0].trim() || '花海';
  const newCardP = spring({frame: Math.max(0, frame - F.newCard), fps, config: {damping: 14, stiffness: 100}});
  const bannerP = spring({frame: Math.max(0, frame - F.resultBanner), fps, config: {damping: 15, stiffness: 90}});
  const resultDone = frame >= F.progressDone;
  const highlightNew = frame >= F.cardClick - 14 && frame < F.arrangeIn;
  const cards = ['夜航 Night Drive', 'Morning Light', 'Demo Etude'];
  return (
    <div style={{position: 'absolute', inset: 0, padding: '30px 34px', color: COLORS.text}}>
      {frame >= F.resultBanner ? (
        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: 12,
            padding: '13px 18px',
            border: `1px solid ${COLORS.greenLine}55`,
            background: 'rgba(46,204,113,.09)',
            color: COLORS.green,
            fontFamily: FONT.mono,
            fontSize: 14,
            opacity: bannerP,
            transform: `translateY(${interpolate(bannerP, [0, 1], [-16, 0])}px)`,
          }}
        >
          <CircleCheck size={20} />
          已导入「{songTitle}」 · {stats.count.toLocaleString()} 音符 · 本地转录完成
          <span style={{marginLeft: 'auto', color: COLORS.gold}}>✓ 已跳过重复</span>
        </div>
      ) : null}
      <div style={{display: 'flex', alignItems: 'center', marginTop: frame >= F.resultBanner ? 26 : 40}}>
        <div>
          <div style={{fontFamily: FONT.display, fontSize: 40, fontWeight: 700}}>曲谱库</div>
          <div style={{fontFamily: FONT.mono, color: COLORS.green, fontSize: 11, letterSpacing: '.1em', marginTop: 6}}>
            LOCAL MUSIC LIBRARY
          </div>
        </div>
        <div style={{marginLeft: 'auto', display: 'flex', gap: 12}}>
          <div
            style={{
              padding: '11px 16px',
              border: '1px solid rgba(46,204,113,.28)',
              fontFamily: FONT.mono,
              color: COLORS.green,
              fontSize: 13,
            }}
          >
            导入 MIDI
          </div>
          <div
            style={{
              padding: '11px 18px',
              border: `1px solid ${COLORS.gold}`,
              background: COLORS.gold,
              color: '#221b00',
              fontFamily: FONT.mono,
              fontWeight: 700,
              fontSize: 13,
              boxShadow: `0 0 26px ${COLORS.gold}26`,
              opacity: frame > F.txClick + 22 ? 0.82 : 1,
              transform: frame > F.txClick && frame < F.txClick + 22 ? 'scale(.94)' : 'scale(1)',
            }}
          >
            从音频转录
          </div>
        </div>
      </div>
      <div style={{display: 'grid', gridTemplateColumns: 'repeat(3, 1fr)', gap: 18, marginTop: 34}}>
        {cards.map((name, index) => (
          <div
            key={name}
            style={{
              height: 176,
              padding: 20,
              border: `1px solid rgba(46,204,113,.17)`,
              background: 'rgba(28,27,27,.68)',
              opacity: 0.92,
            }}
          >
            <Music2 size={28} color={COLORS.green} strokeWidth={1.3} />
            <div style={{marginTop: 26, fontFamily: FONT.body, fontSize: 19, fontWeight: 600}}>{name}</div>
            <div style={{marginTop: 10, fontFamily: FONT.mono, color: COLORS.muted, fontSize: 11}}>MIDI · READY</div>
          </div>
        ))}
      </div>
      {resultDone ? (
        <div
          style={{
            height: 176,
            padding: 20,
            marginTop: 18,
            border: `1px solid ${highlightNew ? COLORS.gold : COLORS.gold}55`,
            background: 'rgba(42,38,20,.7)',
            boxShadow: highlightNew ? `0 0 40px ${COLORS.gold}22` : 'none',
            opacity: newCardP,
            transform: `scale(${interpolate(newCardP, [0, 1], [0.88, 1])}) translateY(${interpolate(newCardP, [0, 1], [24, 0])}px)`,
            transformOrigin: 'left top',
          }}
        >
          <div style={{display: 'flex', justifyContent: 'space-between'}}>
            <FileMusic size={28} color={COLORS.gold} strokeWidth={1.3} />
            <span
              style={{
                fontFamily: FONT.mono,
                fontSize: 10,
                letterSpacing: '.08em',
                border: '1px solid rgba(255,215,0,.5)',
                padding: '3px 8px',
                color: COLORS.gold,
              }}
            >
              AUDIO
            </span>
          </div>
          <div style={{marginTop: 18, fontFamily: FONT.body, fontSize: 19, fontWeight: 600}}>{songTitle}</div>
          <div style={{marginTop: 10, fontFamily: FONT.mono, color: COLORS.gold, fontSize: 11, letterSpacing: '.06em'}}>
            AUDIO TRANSCRIPTION · HIGH QUALITY
          </div>
          <div style={{marginTop: 6, fontFamily: FONT.mono, color: COLORS.muted, fontSize: 11}}>
            {stats.count.toLocaleString()} NOTE EVENTS · 本地 Transkun
          </div>
        </div>
      ) : null}
    </div>
  );
};

const ConfirmModal = ({frame, props}: {frame: number; props: TrailerProps}) => {
  const open = interpolate(frame, [F.confirmOpen, F.confirmSettle], [0, 1], clamp);
  const close = interpolate(frame, [F.confirmClose, F.confirmClose + 26], [1, 0], clamp);
  const visible = Math.min(open, close);
  if (visible <= 0.001) return null;
  const songTitle = (props.sampleLabel || '花海').split(' - ')[0].trim() || '花海';
  const press = frame >= F.startClick && frame < F.startClick + 16 ? Math.sin(((frame - F.startClick) / 16) * Math.PI) : 0;
  return (
    <div
      style={{
        position: 'absolute',
        inset: 0,
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        background: 'rgba(0,0,0,.58)',
        opacity: visible,
        zIndex: 30,
      }}
    >
      <div
        style={{
          width: 540,
          padding: '30px 34px',
          background: '#121110',
          border: `1px solid ${COLORS.gold}48`,
          boxShadow: '0 40px 110px rgba(0,0,0,.85), 0 0 70px rgba(255,215,0,.05)',
          transform: `translateY(${interpolate(visible, [0, 1], [70, 0])}px) scale(${interpolate(visible, [0, 1], [0.95, 1])})`,
        }}
      >
        <div style={{fontFamily: FONT.display, fontSize: 26, fontWeight: 700, color: COLORS.text}}>确认转录</div>
        <div style={{marginTop: 10, fontFamily: FONT.body, fontSize: 16, color: COLORS.text}}>{songTitle}</div>
        <div style={{marginTop: 4, fontFamily: FONT.mono, fontSize: 12, color: COLORS.muted}}>
          MP3 · 本地文件 · 不会上传
        </div>
        <div
          style={{
            marginTop: 20,
            padding: '14px 16px',
            border: `1px solid ${COLORS.gold}55`,
            background: 'rgba(255,215,0,.05)',
            display: 'flex',
            alignItems: 'center',
            gap: 14,
          }}
        >
          <span
            style={{
              width: 9,
              height: 9,
              borderRadius: 99,
              background: COLORS.gold,
              boxShadow: `0 0 14px ${COLORS.gold}`,
            }}
          />
          <div>
            <div style={{fontFamily: FONT.body, fontSize: 15, fontWeight: 600, color: COLORS.gold}}>
              高质量钢琴（Transkun v2）
            </div>
            <div style={{fontFamily: FONT.mono, fontSize: 11, color: COLORS.muted, marginTop: 4}}>
              TRANSFORMER + SEMI-CRF · CPU 本地运行
            </div>
          </div>
          <span
            style={{
              marginLeft: 'auto',
              display: 'flex',
              alignItems: 'center',
              gap: 6,
              fontFamily: FONT.mono,
              fontSize: 11,
              color: COLORS.green,
              border: '1px solid rgba(46,204,113,.4)',
              padding: '4px 9px',
            }}
          >
            <Lock size={12} /> 本地处理
          </span>
        </div>
        <div style={{marginTop: 16, fontFamily: FONT.body, fontSize: 13, color: '#bdb9b6', lineHeight: 1.8}}>
          • 支持 MP3 / WAV / FLAC，钢琴独奏效果最佳
          <br />• 首次约 30–60 秒（含模型加载），之后约 10 秒
          <br />• 音频仅在本地处理，不上传、不联网
        </div>
        <div style={{marginTop: 22, display: 'flex', justifyContent: 'flex-end', gap: 12}}>
          <div
            style={{
              padding: '11px 22px',
              border: '1px solid rgba(255,255,255,.2)',
              fontFamily: FONT.mono,
              fontSize: 13,
              color: COLORS.muted,
            }}
          >
            取消
          </div>
          <div
            style={{
              ...pressedStyle(press),
              padding: '11px 26px',
              background: COLORS.gold,
              color: '#221b00',
              fontFamily: FONT.mono,
              fontWeight: 700,
              fontSize: 13,
              boxShadow: `0 0 28px ${COLORS.gold}33`,
            }}
          >
            开始转录
          </div>
        </div>
      </div>
    </div>
  );
};

const ProgressPanel = ({frame, props}: {frame: number; props: TrailerProps}) => {
  const {fps} = useVideoConfig();
  const stats = noteStats(props.notes);
  const enter = spring({frame: Math.max(0, frame - F.progressCardIn), fps, config: {damping: 15, stiffness: 90}});
  const leave = interpolate(frame, [F.progressDone, F.progressDone + 24], [1, 0], clamp);
  const visible = Math.min(enter, leave);
  if (visible <= 0.001) return null;
  const progP = easeP(frame);
  const counted = Math.round(progP * stats.count);
  const done = frame >= F.progressDone;
  const stage =
    progP < 0.32 ? '特征提取中…' : progP < 0.72 ? '音符识别中…' : done ? '完成' : '生成曲谱…';
  return (
    <div
      style={{
        position: 'absolute',
        left: 30,
        right: 30,
        bottom: 26,
        zIndex: 25,
        opacity: visible,
        transform: `translateY(${interpolate(visible, [0, 1], [60, 0])}px)`,
      }}
    >
      <div
        style={{
          border: `1px solid ${done ? COLORS.greenLine : COLORS.gold}44`,
          background: 'rgba(12,12,12,.94)',
          boxShadow: '0 30px 90px rgba(0,0,0,.8)',
          padding: '20px 24px',
        }}
      >
        <div style={{display: 'flex', alignItems: 'center', gap: 16}}>
          <span style={{color: done ? COLORS.green : COLORS.gold}}>
            {done ? <CircleCheck size={30} /> : <AudioLines size={30} />}
          </span>
          <div style={{minWidth: 0}}>
            <div style={{fontFamily: FONT.mono, fontSize: 11, letterSpacing: '.1em', color: done ? COLORS.green : COLORS.gold}}>
              {done ? 'TRANSCRIPTION COMPLETED' : 'ACTIVE PROCESS'}
            </div>
            <div style={{fontFamily: FONT.body, fontSize: 17, fontWeight: 600, color: COLORS.text, marginTop: 3}}>
              {props.sampleLabel || '花海'} · 高质量钢琴
            </div>
          </div>
          <div style={{marginLeft: 'auto', display: 'flex', gap: 34}}>
            <div style={{textAlign: 'right'}}>
              <div style={{fontFamily: FONT.mono, fontSize: 26, fontWeight: 700, color: done ? COLORS.green : COLORS.gold}}>
                {counted.toLocaleString()}
              </div>
              <div style={{fontFamily: FONT.mono, fontSize: 10, color: COLORS.muted, letterSpacing: '.1em'}}>
                NOTE EVENTS
              </div>
            </div>
            <div style={{textAlign: 'right'}}>
              <div style={{fontFamily: FONT.mono, fontSize: 26, fontWeight: 700, color: COLORS.green}}>
                {done ? Math.round((stats.playable / stats.count) * 100) : Math.round(progP * 100)}%
              </div>
              <div style={{fontFamily: FONT.mono, fontSize: 10, color: COLORS.muted, letterSpacing: '.1em'}}>
                {done ? 'DIRECTLY PLAYABLE' : 'PROGRESS'}
              </div>
            </div>
          </div>
        </div>
        <div style={{display: 'flex', gap: 22, marginTop: 18}}>
          <div style={{width: 560, height: 84, flexShrink: 0}}>
            <Waveform imageSrc={props.waveformSrc} width={560} height={84} progress={progP} />
          </div>
          <div style={{flex: 1, height: 84, overflow: 'hidden'}}>
            <PianoRoll
              notes={props.notes}
              width={620}
              height={84}
              timeOffset={2.2 + (frame - F.progressRun) / fps}
              minPitch={48}
              maxPitch={83}
              highlightHigh
            />
          </div>
        </div>
        <div style={{marginTop: 16, display: 'flex', alignItems: 'center', gap: 14}}>
          <span style={{fontFamily: FONT.mono, fontSize: 12, color: COLORS.muted}}>{stage}</span>
          <div style={{flex: 1, height: 6, background: 'rgba(255,255,255,.08)', overflow: 'hidden'}}>
            <div
              style={{
                height: 6,
                width: `${Math.max(2, progP * 100)}%`,
                background: done
                  ? `linear-gradient(90deg, ${COLORS.greenLine}, ${COLORS.green})`
                  : `linear-gradient(90deg, ${COLORS.goldDim}, ${COLORS.gold})`,
                boxShadow: `0 0 16px ${done ? COLORS.greenLine : COLORS.gold}`,
              }}
            />
          </div>
          {done ? (
            <span
              style={{
                fontFamily: FONT.mono,
                fontSize: 12,
                color: COLORS.green,
                border: '1px solid rgba(46,204,113,.5)',
                padding: '5px 12px',
              }}
            >
              ✓ 已导入 · 查看曲谱
            </span>
          ) : null}
        </div>
      </div>
    </div>
  );
};

const easeP = (frame: number) => {
  const t = interpolate(frame, [F.progressRun, F.progressDone - 70], [0, 1], clamp);
  return 1 - Math.pow(1 - t, 2.2);
};

const ArrangePage = ({
  frame,
  props,
  playing,
}: {
  frame: number;
  props: TrailerProps;
  playing: boolean;
}) => {
  const {fps} = useVideoConfig();
  const stats = noteStats(props.notes);
  const playableNotes = props.notes.filter(
    (n) => n.pitch >= PROFILE_KEYS.midiLow && n.pitch <= PROFILE_KEYS.midiHigh,
  );
  const profileDone = frame >= F.profilePick;
  const paramsDone = frame >= F.paramsSet;
  const compileP = interpolate(frame, [F.compileClick, F.compileClick + 14], [0, 1], clamp);
  const compiling = frame >= F.compileClick && frame < F.compileDone;
  const compileOk = frame >= F.compileDone;
  const statsIn = spring({frame: Math.max(0, frame - F.statsIn), fps, config: {damping: 15, stiffness: 90}});
  const dropdownOpen = frame >= F.profileOpen && frame < F.profilePick + 20;
  const previewT = (frame - F.previewStart) / fps;
  const rollT = 6.2 + (playing ? previewT : 0);
  const pressPlay = frame >= F.playClick && frame < F.playClick + 16 ? Math.sin(((frame - F.playClick) / 16) * Math.PI) : 0;
  const pressCompile =
    frame >= F.compileClick && frame < F.compileClick + 16 ? Math.sin(((frame - F.compileClick) / 16) * Math.PI) : 0;
  return (
    <div style={{position: 'absolute', inset: 0, display: 'flex', color: COLORS.text}}>
      <div style={{width: 380, padding: '26px 24px', borderRight: '1px solid rgba(46,204,113,.12)', background: 'rgba(10,10,10,.92)'}}>
        <div style={{display: 'flex', alignItems: 'center', gap: 10, fontFamily: FONT.mono, fontSize: 12, letterSpacing: '.1em', color: COLORS.muted, marginBottom: 18}}>
          <SlidersHorizontal size={16} /> 编排参数
        </div>

        <div style={{fontFamily: FONT.mono, fontSize: 11, color: COLORS.muted, marginBottom: 8}}>游戏 PROFILE</div>
        <div
          style={{
            position: 'relative',
            border: `1px solid ${profileDone ? COLORS.gold : 'rgba(255,255,255,.16)'}`,
            background: 'rgba(28,27,27,.9)',
            padding: '12px 14px',
            boxShadow: profileDone ? `0 0 26px ${COLORS.gold}22` : 'none',
          }}
        >
          <div style={{display: 'flex', alignItems: 'center', gap: 10}}>
            <Keyboard size={16} color={profileDone ? COLORS.gold : COLORS.muted} />
            <div style={{flex: 1, minWidth: 0}}>
              <div style={{fontFamily: FONT.body, fontSize: 14, fontWeight: 600, color: profileDone ? COLORS.gold : COLORS.text, whiteSpace: 'nowrap'}}>
                第五人格 · 36 键钢琴
              </div>
              <div style={{fontFamily: FONT.mono, fontSize: 10, color: COLORS.muted, marginTop: 3, letterSpacing: '.08em'}}>
                IDENTITY V PROFILE
              </div>
            </div>
            <span style={{fontFamily: FONT.mono, fontSize: 10, color: COLORS.green}}>▼</span>
          </div>
          {dropdownOpen ? (
            <div style={{position: 'absolute', left: 0, right: 0, top: '100%', marginTop: 4, zIndex: 5, border: `1px solid ${COLORS.gold}44`, background: '#171515', boxShadow: '0 24px 60px rgba(0,0,0,.7)'}}>
              <div style={{padding: '10px 14px', fontFamily: FONT.mono, fontSize: 12, color: COLORS.muted}}>identity-v (内置)</div>
              <div style={{padding: '10px 14px', fontFamily: FONT.mono, fontSize: 12, color: COLORS.gold, background: 'rgba(255,215,0,.07)', display: 'flex', alignItems: 'center', gap: 8}}>
                <Check size={14} /> 第五人格 · 36 键钢琴
              </div>
            </div>
          ) : null}
        </div>
        {profileDone ? (
          <div style={{marginTop: 10, fontFamily: FONT.mono, fontSize: 11, color: COLORS.green, display: 'flex', gap: 14, opacity: interpolate(frame, [F.profilePick, F.profilePick + 18], [0, 1], clamp)}}>
            <span>MIDI {PROFILE_KEYS.midiLow}—{PROFILE_KEYS.midiHigh}</span>
            <span>{PROFILE_KEYS.keys} KEYS</span>
            <span>MAX {PROFILE_KEYS.maxPolyphony}</span>
          </div>
        ) : null}

        <div style={{marginTop: 22, fontFamily: FONT.mono, fontSize: 11, color: COLORS.muted, marginBottom: 8}}>音域策略</div>
        <div style={{border: `1px solid ${paramsDone ? 'rgba(255,215,0,.5)' : 'rgba(255,255,255,.16)'}`, background: 'rgba(28,27,27,.9)', padding: '12px 14px', display: 'flex', alignItems: 'center', gap: 10}}>
          <Wand2 size={16} color={paramsDone ? COLORS.gold : COLORS.muted} />
          <span style={{fontFamily: FONT.body, fontSize: 13.5}}>智能八度（SmartFold）</span>
          <span style={{marginLeft: 'auto', fontFamily: FONT.mono, fontSize: 10, color: COLORS.green}}>▼</span>
        </div>

        <div style={{marginTop: 22, display: 'flex', alignItems: 'center', justifyContent: 'space-between'}}>
          <div>
            <div style={{fontFamily: FONT.body, fontSize: 14, fontWeight: 600}}>旋律保护</div>
            <div style={{fontFamily: FONT.mono, fontSize: 10, color: COLORS.muted, marginTop: 4}}>TOP VOICE · BASS</div>
          </div>
          <div
            style={{
              width: 54,
              height: 28,
              borderRadius: 99,
              background: paramsDone ? COLORS.gold : 'rgba(255,255,255,.14)',
              position: 'relative',
              transition: 'none',
            }}
          >
            <div
              style={{
                position: 'absolute',
                top: 3,
                left: paramsDone ? 27 : 3,
                width: 22,
                height: 22,
                borderRadius: 99,
                background: '#fff',
              }}
            />
          </div>
        </div>

        <div style={{marginTop: 22}}>
          <div style={{display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 8}}>
            <span style={{fontFamily: FONT.body, fontSize: 14, fontWeight: 600}}>最大复音</span>
            <span style={{fontFamily: FONT.mono, fontSize: 13, color: COLORS.gold}}>{paramsDone ? 4 : 1}</span>
          </div>
          <div style={{display: 'grid', gridTemplateColumns: 'repeat(8, 1fr)', gap: 5}}>
            {[1, 2, 3, 4, 5, 6, 7, 8].map((n) => (
              <div
                key={n}
                style={{
                  height: 30,
                  display: 'grid',
                  placeItems: 'center',
                  fontFamily: FONT.mono,
                  fontSize: 12,
                  border: `1px solid ${paramsDone && n === 4 ? COLORS.gold : 'rgba(255,255,255,.14)'}`,
                  background: paramsDone && n === 4 ? 'rgba(255,215,0,.14)' : 'transparent',
                  color: paramsDone && n === 4 ? COLORS.gold : COLORS.muted,
                }}
              >
                {n}
              </div>
            ))}
          </div>
        </div>

        <div
          style={{
            ...pressedStyle(pressCompile),
            marginTop: 24,
            padding: '14px 0',
            textAlign: 'center',
            background: COLORS.gold,
            color: '#221b00',
            fontFamily: FONT.mono,
            fontWeight: 700,
            fontSize: 14,
            boxShadow: `0 0 30px ${COLORS.gold}2a`,
            opacity: compileOk ? 0.75 : 1,
          }}
        >
          {compiling ? '编译中…' : compileOk ? '✓ 编译完成' : '编译'}
        </div>

        {compileOk ? (
          <div style={{marginTop: 14, border: '1px solid rgba(46,204,113,.3)', background: 'rgba(13,13,13,.9)', padding: '14px', opacity: statsIn, transform: `translateY(${interpolate(statsIn, [0, 1], [18, 0])}px)`}}>
            <div style={{fontFamily: FONT.mono, fontSize: 10, letterSpacing: '.12em', color: COLORS.muted, marginBottom: 10}}>编译统计</div>
            <div style={{display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '8px 14px', fontFamily: FONT.mono, fontSize: 12}}>
              <div style={{display: 'flex', justifyContent: 'space-between'}}><span style={{color: COLORS.muted}}>输出</span><span style={{color: COLORS.text}}>{playableNotes.length.toLocaleString()}</span></div>
              <div style={{display: 'flex', justifyContent: 'space-between'}}><span style={{color: COLORS.muted}}>折叠</span><span style={{color: COLORS.text}}>1,024</span></div>
              <div style={{display: 'flex', justifyContent: 'space-between'}}><span style={{color: COLORS.muted}}>保护·Top</span><span style={{color: COLORS.green}}>{stats.playable}</span></div>
              <div style={{display: 'flex', justifyContent: 'space-between'}}><span style={{color: COLORS.muted}}>裁·超限</span><span style={{color: COLORS.muted}}>0</span></div>
            </div>
          </div>
        ) : null}

        <div
          style={{
            ...pressedStyle(pressPlay),
            marginTop: 22,
            padding: '13px 0',
            textAlign: 'center',
            border: `1px solid ${playing ? COLORS.greenLine : 'rgba(46,204,113,.4)'}`,
            background: playing ? 'rgba(46,204,113,.12)' : 'transparent',
            color: playing ? COLORS.green : COLORS.green,
            fontFamily: FONT.mono,
            fontSize: 13,
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            gap: 10,
          }}
        >
          <Play size={15} /> {playing ? '演奏中 · 发送到游戏' : '开始演奏'}
        </div>
      </div>

      <div style={{flex: 1, padding: '26px 28px', display: 'flex', flexDirection: 'column'}}>
        <div style={{display: 'flex', alignItems: 'center', gap: 12}}>
          <div style={{fontFamily: FONT.mono, fontSize: 12, color: COLORS.muted, letterSpacing: '.1em'}}>时间轴 · PIANO ROLL</div>
          <div style={{marginLeft: 'auto', display: 'flex', gap: 10}}>
            <TechTag tone={paramsDone ? 'gold' : 'neutral'} style={{fontSize: 12}}>SMART FOLD</TechTag>
            <TechTag tone={paramsDone ? 'gold' : 'neutral'} style={{fontSize: 12}}>MELODY PROTECT</TechTag>
          </div>
        </div>
        <div style={{marginTop: 16, flex: 1, minHeight: 0}}>
          <PianoRoll
            notes={playableNotes}
            width={806}
            height={playing ? 330 : 368}
            timeOffset={rollT}
            minPitch={PROFILE_KEYS.midiLow}
            maxPitch={PROFILE_KEYS.midiHigh}
            highlightHigh
          />
        </div>
        {playing ? (
          <div style={{marginTop: 16, height: 128}}>
            <IdentityVKeyboard notes={props.notes} t={rollT} width={806} height={120} />
          </div>
        ) : null}
        <div style={{marginTop: playing ? 12 : 16, display: 'flex', alignItems: 'center', gap: 34}}>
          <Stat label="RANGE" value="48—83" tone="green" />
          <Stat label="MAX POLYPHONY" value="4" />
          <Stat label="TOP VOICE" value="PROTECTED" />
          <Stat label="MODE" value="HIGH QUALITY" tone="green" />
          <div style={{marginLeft: 'auto', display: 'flex', alignItems: 'center', gap: 12}}>
            <span style={{fontFamily: FONT.mono, fontSize: 12, color: playing ? COLORS.green : COLORS.muted}}>
              {playing ? `PLAYING · ${rollT.toFixed(1)}s` : 'READY'}
            </span>
            <span
              style={{
                width: 8,
                height: 8,
                borderRadius: 99,
                background: playing ? COLORS.greenLine : COLORS.muted,
                boxShadow: playing ? `0 0 12px ${COLORS.greenLine}` : 'none',
              }}
            />
          </div>
        </div>
        {playing ? (
          <div style={{marginTop: 14, height: 4, background: 'rgba(255,255,255,.08)', position: 'relative'}}>
            <div
              style={{
                position: 'absolute',
                left: 0,
                top: 0,
                bottom: 0,
                width: `${Math.min(100, ((frame - F.previewStart) / (F.previewEnd - F.previewStart)) * 100)}%`,
                background: `linear-gradient(90deg, ${COLORS.greenLine}, ${COLORS.gold})`,
                boxShadow: `0 0 14px ${COLORS.gold}`,
              }}
            />
          </div>
        ) : null}
      </div>
    </div>
  );
};

const SettingsPage = ({
  frame,
  engine,
}: {
  frame: number;
  engine: 'high_quality' | 'fast';
}) => {
  const cards: Array<{id: 'high_quality' | 'fast'; title: string; sub: string; icon: ReactNode}> = [
    {id: 'high_quality', title: '高质量钢琴（Transkun v2）', sub: 'Transformer + Semi-CRF · CPU 本地 · 音符边界更准确', icon: <AudioLines size={22} />},
    {id: 'fast', title: '快速（Basic Pitch）', sub: '启动快、体积小 · 适合简单钢琴与快速预览', icon: <Zap size={22} />},
  ];
  return (
    <div style={{position: 'absolute', inset: 0, padding: '30px 36px', color: COLORS.text}}>
      <div style={{display: 'flex', alignItems: 'baseline', gap: 18}}>
        <div style={{fontFamily: FONT.display, fontSize: 36, fontWeight: 700}}>设置</div>
        <div style={{fontFamily: FONT.mono, fontSize: 11, color: COLORS.muted, letterSpacing: '.08em'}}>
          SYSTEM_CONFIGURATION_NODE // SCORELEAP_ENV
        </div>
        <div
          style={{
            marginLeft: 'auto',
            display: 'flex',
            alignItems: 'center',
            gap: 8,
            fontFamily: FONT.mono,
            fontSize: 11,
            color: COLORS.green,
            border: '1px solid rgba(46,204,113,.4)',
            padding: '6px 12px',
            opacity: interpolate(frame, [F.privacyNote, F.privacyNote + 18], [0, 1], clamp),
            boxShadow: frame >= F.privacyNote ? `0 0 22px ${COLORS.greenLine}33` : 'none',
          }}
        >
          <Lock size={13} /> 本地处理 · 不上传 · 无账号
        </div>
      </div>
      <div style={{fontFamily: FONT.mono, fontSize: 13, color: COLORS.gold, marginTop: 30, letterSpacing: '.1em'}}>
        转录引擎 · ENGINE
      </div>
      <div style={{display: 'flex', gap: 18, marginTop: 16}}>
        {cards.map((card) => {
          const active = engine === card.id;
          return (
            <div
              key={card.id}
              style={{
                width: 380,
                padding: '20px 22px',
                border: `1px solid ${active ? COLORS.gold : 'rgba(255,255,255,.14)'}`,
                background: active ? 'rgba(255,215,0,.06)' : 'rgba(20,20,20,.85)',
                boxShadow: active ? `0 0 34px ${COLORS.gold}20` : 'none',
                display: 'flex',
                gap: 16,
                alignItems: 'flex-start',
                position: 'relative',
              }}
            >
              <span style={{color: active ? COLORS.gold : COLORS.muted, marginTop: 2}}>{card.icon}</span>
              <div>
                <div style={{display: 'flex', alignItems: 'center', gap: 10}}>
                  <span style={{fontFamily: FONT.body, fontSize: 16, fontWeight: 600, color: active ? COLORS.gold : COLORS.text}}>
                    {card.title}
                  </span>
                  {active ? <CircleCheck size={18} color={COLORS.gold} /> : null}
                </div>
                <div style={{fontFamily: FONT.body, fontSize: 12.5, color: COLORS.muted, marginTop: 8, lineHeight: 1.5}}>
                  {card.sub}
                </div>
              </div>
            </div>
          );
        })}
      </div>
      <div style={{marginTop: 28, fontFamily: FONT.mono, fontSize: 12, color: COLORS.muted, letterSpacing: '.06em'}}>
        《第五人格》36 键钢琴 Profile 已加载 · MIDI 48—83 · MAX POLYPHONY 4
      </div>
      <div
        style={{
          marginTop: 22,
          border: '1px solid rgba(46,204,113,.25)',
          background: 'rgba(13,13,13,.9)',
          padding: '16px 18px',
          display: 'flex',
          gap: 14,
          alignItems: 'center',
        }}
      >
        <ShieldCheck size={20} color={COLORS.green} />
        <span style={{fontFamily: FONT.body, fontSize: 13.5, color: COLORS.muted}}>
          模型管理 · 下载前会显示版本、大小与来源并等待确认；音频不上传，不收集遥测数据。
        </span>
      </div>
    </div>
  );
};

export const AppDemoFlowScene = ({props}: {props: TrailerProps}) => {
  const frame = useCurrentFrame();
  const {fps} = useVideoConfig();

  const page: 'library' | 'arrange' | 'settings' =
    frame >= F.settingsIn ? 'settings' : frame >= F.arrangeIn ? 'arrange' : 'library';
  const title =
    page === 'arrange' ? 'ScoreLeap · 编排工作台' : page === 'settings' ? 'ScoreLeap · 设置' : 'ScoreLeap · 曲谱库';

  const libStyle = pageSlide(frame, -40, F.arrangeIn, 1);
  const arrangeStyle = pageSlide(frame, F.arrangeIn, F.settingsIn, 1);
  const settingsStyle = pageSlide(frame, F.settingsIn, F.sceneEnd + 60, 1);

  const cursorPath = [
    {f: 40, x: 1290, y: 820},
    {f: F.txMove, x: 1611, y: 243},
    {f: F.txClick, x: 1611, y: 243, click: true},
    {f: 250, x: 1611, y: 243},
    {f: F.startMove, x: 1265, y: 715},
    {f: F.startClick, x: 1265, y: 715, click: true},
    {f: 430, x: 1265, y: 715},
    {f: F.cardMove, x: 685, y: 684},
    {f: F.cardClick, x: 685, y: 684, click: true},
    {f: 1350, x: 685, y: 684},
    {f: F.playMove, x: 661, y: 860},
    {f: F.playClick, x: 661, y: 860, click: true},
    {f: 2000, x: 661, y: 860},
    {f: F.engineMove, x: 996, y: 375},
    {f: F.engineFast, x: 996, y: 375, click: true},
    {f: F.engineBackMove, x: 634, y: 375},
    {f: F.engineHQ, x: 634, y: 375, click: true},
    {f: 2395, x: 634, y: 375},
  ];
  const cursorVisible =
    (frame >= 35 && frame < F.progressCardIn + 30) ||
    (frame > F.cardMove && frame < F.previewStart) ||
    (frame > F.settingsIn + 40 && frame < F.sceneEnd - 4);
  const cur = cursorAt(frame, cursorPath);

  const engine: 'high_quality' | 'fast' =
    frame >= F.engineHQ ? 'high_quality' : frame >= F.engineFast ? 'fast' : 'high_quality';

  const winScale = interpolate(frame, [0, 26], [0.955, 1], clamp);

  return (
    <div style={{position: 'absolute', inset: 0, background: COLORS.bg, overflow: 'hidden'}}>
      <div
        style={{
          position: 'absolute',
          inset: 0,
          background:
            'linear-gradient(rgba(46,204,113,.05) 1px, transparent 1px), linear-gradient(90deg, rgba(46,204,113,.05) 1px, transparent 1px)',
          backgroundSize: '32px 32px',
        }}
      />
      <div style={{position: 'absolute', left: WIN.x, top: WIN.y, transform: `scale(${winScale})`, transformOrigin: 'center top'}}>
        <AppWindow width={WIN.w} height={WIN.h} title={title} perspective={false}>
          <div style={{position: 'absolute', inset: 0}}>
            <NavSidebar frame={frame} />
            <div style={{position: 'absolute', left: SIDEBAR_W, top: 0, width: CONTENT_W, height: CONTENT_H, overflow: 'hidden'}}>
              <div style={{...libStyle, position: 'absolute', inset: 0}}>
                <LibraryPage frame={frame} props={props} />
              </div>
              <div style={{...arrangeStyle, position: 'absolute', inset: 0}}>
                <ArrangePage frame={frame} props={props} playing={frame >= F.previewStart} />
              </div>
              <div style={{...settingsStyle, position: 'absolute', inset: 0}}>
                <SettingsPage frame={frame} engine={engine} />
              </div>
              <ConfirmModal frame={frame} props={props} />
              <ProgressPanel frame={frame} props={props} />
            </div>
          </div>
        </AppWindow>
      </div>
      <div style={{position: 'absolute', left: SAFE_X, top: SAFE_Y, zIndex: 10}}>
        <TechTag tone="green">REAL PRODUCT FLOW</TechTag>
      </div>
      {cursorVisible ? <Cursor x={cur.x} y={cur.y} press={cur.press} /> : null}
      <div
        style={{
          position: 'absolute',
          right: 90,
          bottom: 40,
          color: COLORS.muted,
          fontFamily: FONT.mono,
          fontSize: 12,
          letterSpacing: '.1em',
        }}
      >
        SCORELEAP DESKTOP · WINDOWS
      </div>
    </div>
  );
};

// ---------------------------------------------------------------------------
// IdentityV 段：程序化哥特/庄园表达，不使用任何未授权官方素材
// ---------------------------------------------------------------------------

const PAPER_NOISE = `url("data:image/svg+xml;utf8,${encodeURIComponent(
  `<svg xmlns='http://www.w3.org/2000/svg' width='280' height='280'><filter id='n'><feTurbulence type='fractalNoise' baseFrequency='0.85' numOctaves='2' stitchTiles='stitch'/><feColorMatrix type='saturate' values='0'/><feComponentTransfer><feFuncA type='table' tableValues='0 0.16'/></feComponentTransfer></filter><rect width='280' height='280' filter='url(%23n)'/></svg>`,
)}")`;

const GothicPaper = ({children, dark = false}: {children: ReactNode; dark?: boolean}) => {
  const frame = useCurrentFrame();
  const flicker = 0.82 + Math.sin(frame / 7) * 0.06 + Math.sin(frame / 23 + 1.3) * 0.06;
  return (
    <div style={{position: 'absolute', inset: 0, overflow: 'hidden', background: dark ? '#201009' : '#2b1a10'}}>
      <div
        style={{
          position: 'absolute',
          inset: 0,
          background: `radial-gradient(ellipse at 50% 34%, ${GOTHIC.umber} 0%, #1a0e08 78%)`,
        }}
      />
      <div style={{position: 'absolute', inset: 0, backgroundImage: PAPER_NOISE, opacity: dark ? 0.35 : 0.5, mixBlendMode: 'soft-light'}} />
      <div
        style={{
          position: 'absolute',
          inset: 0,
          background: `radial-gradient(circle at 12% 18%, rgba(255,217,160,${0.10 * flicker}) 0%, transparent 22%), radial-gradient(circle at 86% 12%, rgba(255,217,160,${0.07 * flicker}) 0%, transparent 18%), radial-gradient(circle at 8% 86%, rgba(255,217,160,${0.06 * flicker}) 0%, transparent 16%)`,
        }}
      />
      {children}
    </div>
  );
};

const GothicFrame = ({children, width, height}: {children: ReactNode; width: number; height: number}) => (
  <div
    style={{
      position: 'relative',
      width,
      height,
      padding: 16,
      background: '#241812',
      border: `2px solid ${GOTHIC.agedMetal}`,
      boxShadow: `0 0 0 4px #120b06, 0 0 0 5px ${GOTHIC.burgundy}, 0 40px 120px rgba(0,0,0,.85), inset 0 0 60px rgba(0,0,0,.6)`,
    }}
  >
    {[{t: '8px', l: '8px'}, {t: '8px', l: undefined}, {t: undefined, l: '8px'}, {t: undefined, l: undefined}].map((c, i) => (
      <div
        key={i}
        style={{
          position: 'absolute',
          top: c.t,
          left: c.l,
          right: c.l === undefined && c.t === '8px' ? '8px' : undefined,
          bottom: c.t === undefined && c.l === '8px' ? '8px' : undefined,
          width: 26,
          height: 26,
          borderColor: GOTHIC.candleDim,
          opacity: 0.75,
          ...(c.t === '8px'
            ? c.l === '8px'
              ? {borderTop: '2px solid', borderLeft: '2px solid'}
              : {borderTop: '2px solid', borderRight: '2px solid'}
            : c.l === '8px'
              ? {borderBottom: '2px solid', borderLeft: '2px solid'}
              : {borderBottom: '2px solid', borderRight: '2px solid'}),
        }}
      />
    ))}
    <div
      style={{
        position: 'absolute',
        inset: 8,
        border: `1px solid ${GOTHIC.parchmentDim}55`,
        background: 'rgba(30,18,9,.92)',
        display: 'flex',
      }}
    >
      {children}
    </div>
  </div>
);

export const IdentityVScene = ({props}: {props: TrailerProps}) => {
  const frame = useCurrentFrame();
  const {fps, durationInFrames} = useVideoConfig();
  const push = spring({frame, fps, config: {damping: 16, stiffness: 70}});
  const kbScale = interpolate(push, [0, 1], [0.86, 1.14]);
  const kbOpacity = interpolate(frame, [10, 34], [0, 1], clamp);
  const titleP = spring({frame: Math.max(0, frame - 20), fps, config: {damping: 14, stiffness: 80}});
  const suits = [<Spade key="s" />, <Heart key="h" />, <Diamond key="d" />, <Club key="c" />];
  const suitPos = [
    {left: '7%', top: '14%'},
    {right: '7%', top: '14%'},
    {left: '7%', bottom: '12%'},
    {right: '7%', bottom: '12%'},
  ];
  // 主片从 0s 开始持续播放《花海》源音频；Identity V 段在全片约 42.27s 进入。
  // 让 36 键可视化直接跟随同一时间轴，这样在约 47.90s 切入实机时，音符位置能连续衔接。
  const t = 42.2667 + frame / fps;
  return (
    <div style={{position: 'absolute', inset: 0, overflow: 'hidden'}}>
      <GothicPaper dark>
        <div
          style={{
            position: 'absolute',
            top: 60,
            left: 0,
            right: 0,
            textAlign: 'center',
            fontFamily: FONT.mono,
            letterSpacing: '.34em',
            fontSize: 15,
            color: GOTHIC.candleDim,
            opacity: titleP,
          }}
        >
          IDENTITY V PROFILE · 36 键钢琴
        </div>
        {suits.map((icon, i) => (
          <div key={i} style={{position: 'absolute', ...suitPos[i], color: GOTHIC.burgundy, opacity: 0.9}}>
            {icon}
          </div>
        ))}
        <div
          style={{
            position: 'absolute',
            top: 108,
            left: 0,
            right: 0,
            textAlign: 'center',
            fontFamily: FONT.display,
            fontSize: 74,
            fontWeight: 700,
            color: GOTHIC.bone,
            opacity: titleP,
            transform: `translateY(${interpolate(titleP, [0, 1], [30, 0])}px)`,
            textShadow: `0 0 60px ${GOTHIC.candle}33`,
          }}
        >
          第五人格 · 36 键钢琴
        </div>
        <div
          style={{
            position: 'absolute',
            top: 236,
            left: '50%',
            transform: 'translateX(-50%)',
            fontFamily: FONT.mono,
            fontSize: 13,
            letterSpacing: '.14em',
            color: GOTHIC.parchmentDim,
          }}
        >
          MIDI {PROFILE_KEYS.midiLow}—{PROFILE_KEYS.midiHigh} · {PROFILE_KEYS.keys} KEYS · MAX POLYPHONY {PROFILE_KEYS.maxPolyphony}
        </div>
        <div
          style={{
            position: 'absolute',
            left: '50%',
            top: 300,
            width: 1200,
            height: 460,
            transform: 'translateX(-50%)',
            opacity: kbOpacity,
          }}
        >
          <div
            style={{
              position: 'absolute',
              inset: 0,
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              transform: `scale(${kbScale})`,
              transformOrigin: 'center 62%',
            }}
          >
            <div style={{width: 1160}}>
              <IdentityVKeyboard notes={props.notes} t={t} width={1160} height={200} gothic labelAbove />
            </div>
          </div>
          <div
            style={{
              position: 'absolute',
              left: '50%',
              top: 430,
              transform: 'translateX(-50%)',
              fontFamily: FONT.mono,
              fontSize: 12,
              letterSpacing: '.12em',
              color: GOTHIC.candle,
              opacity: interpolate(frame, [70, 110], [0, 1], clamp),
            }}
          >
            {IDENTITY_V_LABELS[PROFILE_KEYS.midiLow]} … {IDENTITY_V_LABELS[PROFILE_KEYS.midiHigh]} · 真实键位映射 · 本地编译
          </div>
        </div>
        <div style={{position: 'absolute', left: 90, top: 430, color: GOTHIC.candleDim, opacity: 0.8}}>
          <Clock size={40} strokeWidth={1.1} />
        </div>
        <div style={{position: 'absolute', right: 90, top: 430, color: GOTHIC.candleDim, opacity: 0.8}}>
          <Flame size={40} strokeWidth={1.1} />
        </div>
        <div
          style={{
            position: 'absolute',
            left: 0,
            right: 0,
            bottom: 0,
            height: 4,
            background: `linear-gradient(90deg, transparent, ${GOTHIC.candle}, ${GOTHIC.burgundy}, transparent)`,
            opacity: interpolate(frame, [40, 90], [0, 1], clamp),
            boxShadow: `0 0 50px ${GOTHIC.candle}55`,
          }}
        />
      </GothicPaper>
    </div>
  );
};

// ---------------------------------------------------------------------------
// Gameplay 段：旧纸门帘打开揭示（实机录像或明确标注的占位）
// ---------------------------------------------------------------------------

export const GameplayRevealScene = ({props}: {props: TrailerProps}) => {
  const frame = useCurrentFrame();
  const {fps} = useVideoConfig();
  const localDuration = Math.round(props.gameplayDurationSeconds * fps);
  const curtain = easeOutCurtain(interpolate(frame, [8, 42], [0, 1], clamp));
  const contentOpacity = interpolate(frame, [30, 56], [0, 1], clamp);
  const exit = interpolate(frame, [localDuration - 30, localDuration - 4], [1, 0], clamp);
  const opacity = Math.min(contentOpacity, exit);
  const audioIn = interpolate(frame, [0, 24], [0, 1], clamp);
  const audioOut = interpolate(frame, [localDuration - 30, localDuration - 4], [1, 0], clamp);
  // 实机原始录音比宣传片音床约高 10–12dB；压到 25% 并交叉淡化，避免切入时音量突跳。
  const gameplayVolume = 0.25 * Math.min(audioIn, audioOut);
  return (
    <div style={{position: 'absolute', inset: 0, overflow: 'hidden'}}>
      {/* 纸帘：从中心向两侧打开 */}
      <div
        style={{
          position: 'absolute',
          top: 0,
          bottom: 0,
          left: 0,
          width: `${interpolate(curtain, [0, 1], [50, 0])}%`,
          background: `linear-gradient(90deg, #1c0f07, ${GOTHIC.umber} 82%, #0d0705)`,
          backgroundImage: `linear-gradient(90deg, #1c0f07, ${GOTHIC.umber} 82%, #0d0705), ${PAPER_NOISE}`,
          boxShadow: 'inset -18px 0 40px rgba(0,0,0,.8)',
          zIndex: 20,
        }}
      />
      <div
        style={{
          position: 'absolute',
          top: 0,
          bottom: 0,
          right: 0,
          width: `${interpolate(curtain, [0, 1], [50, 0])}%`,
          background: `linear-gradient(-90deg, #1c0f07, ${GOTHIC.umber} 82%, #0d0705)`,
          backgroundImage: `linear-gradient(-90deg, #1c0f07, ${GOTHIC.umber} 82%, #0d0705), ${PAPER_NOISE}`,
          boxShadow: 'inset 18px 0 40px rgba(0,0,0,.8)',
          zIndex: 20,
        }}
      />
      <div style={{position: 'absolute', inset: 0, opacity, background: '#0c0705'}}>
        {props.gameplaySrc ? (
          <>
            <OffthreadVideo
              src={staticFile(props.gameplaySrc)}
              style={{width: '100%', height: '100%', objectFit: 'cover'}}
              volume={gameplayVolume}
            />
            <div style={{position: 'absolute', top: 34, left: 40, zIndex: 5}}>
              <TechTag>IDENTITY V · 花海 · REAL GAMEPLAY</TechTag>
            </div>
          </>
        ) : (
          <GothicPaper dark>
            <div style={{position: 'absolute', inset: 60}}>
              <GothicFrame width={1800} height={960}>
                <div style={{flex: 1, display: 'flex', flexDirection: 'column'}}>
                  <div
                    style={{
                      height: 86,
                      display: 'flex',
                      alignItems: 'center',
                      padding: '0 34px',
                      borderBottom: `1px solid ${GOTHIC.agedMetal}66`,
                      fontFamily: FONT.mono,
                      letterSpacing: '.26em',
                      color: GOTHIC.candleDim,
                      fontSize: 14,
                    }}
                  >
                    IDENTITY V · 游戏钢琴目标框
                  </div>
                  <div style={{flex: 1, display: 'flex', position: 'relative'}}>
                    {/* 音符落点轨道 */}
                    <div style={{position: 'absolute', inset: '40px 40px 0', display: 'flex', gap: 6}}>
                      {Array.from({length: 36}, (_, i) => (
                        <div
                          key={i}
                          style={{
                            flex: 1,
                            borderLeft: '1px solid rgba(217,201,163,.08)',
                            background: i % 7 === 3 ? 'rgba(217,201,163,.05)' : 'transparent',
                          }}
                        />
                      ))}
                    </div>
                    <div
                      style={{
                        position: 'absolute',
                        left: 0,
                        right: 0,
                        top: 0,
                        height: 96,
                        borderTop: `2px solid ${GOTHIC.candle}66`,
                        borderBottom: `1px solid ${GOTHIC.candle}33`,
                        boxShadow: `0 0 30px ${GOTHIC.candle}22`,
                      }}
                    />
                    <div
                      style={{
                        position: 'absolute',
                        left: 0,
                        right: 0,
                        bottom: 0,
                        padding: '0 34px 22px',
                      }}
                    >
                      <IdentityVKeyboard notes={props.notes} t={9 + frame / 60} width={1640} height={150} gothic labelAbove={false} />
                    </div>
                    <div
                      style={{
                        position: 'absolute',
                        inset: 0,
                        display: 'flex',
                        alignItems: 'center',
                        justifyContent: 'center',
                        flexDirection: 'column',
                        gap: 14,
                        background: 'rgba(12,6,4,.72)',
                      }}
                    >
                      <div
                        style={{
                          fontFamily: FONT.mono,
                          fontSize: 30,
                          letterSpacing: '.2em',
                          color: GOTHIC.candle,
                          textShadow: `0 0 40px ${GOTHIC.candle}55`,
                          border: `1px solid ${GOTHIC.candle}55`,
                          padding: '14px 26px',
                          background: 'rgba(20,10,5,.7)',
                        }}
                      >
                        PLACEHOLDER · 等待真实实机
                      </div>
                      <div style={{fontFamily: FONT.body, fontSize: 18, color: GOTHIC.parchmentDim}}>
                        后续放入《第五人格》实机录像后，此处自动接管时间线
                      </div>
                    </div>
                  </div>
                </div>
              </GothicFrame>
            </div>
            <div style={{position: 'absolute', bottom: 40, left: 0, right: 0, textAlign: 'center', fontFamily: FONT.mono, fontSize: 12, color: GOTHIC.parchmentDim, letterSpacing: '.14em'}}>
              AUDIO → NOTES → 36 KEYS → GAME INPUT
            </div>
          </GothicPaper>
        )}
      </div>
    </div>
  );
};

const easeOutCurtain = (t: number) => 1 - Math.pow(1 - t, 3);

// ---------------------------------------------------------------------------
// 简洁 Ending
// ---------------------------------------------------------------------------

export const EndingScene = () => {
  const frame = useCurrentFrame();
  const {fps} = useVideoConfig();
  const p = spring({frame: Math.max(0, frame - 12), fps, config: {damping: 15, stiffness: 80}});
  const lineP = interpolate(frame, [38, 70], [0, 1], clamp);
  return (
    <div style={{position: 'absolute', inset: 0, background: COLORS.bg, color: COLORS.text, overflow: 'hidden'}}>
      <div
        style={{
          position: 'absolute',
          inset: 0,
          background: `radial-gradient(circle at 50% 40%, rgba(255,215,0,.08) 0%, transparent 42%)`,
        }}
      />
      <div
        style={{
          position: 'absolute',
          left: 0,
          right: 0,
          top: 220,
          display: 'flex',
          justifyContent: 'center',
          opacity: p,
          transform: `scale(${0.9 + p * 0.1})`,
        }}
      >
        <LogoMark size={120} />
      </div>
      <div
        style={{
          position: 'absolute',
          left: 0,
          right: 0,
          top: 430,
          textAlign: 'center',
          fontFamily: FONT.display,
          fontSize: 60,
          fontWeight: 700,
        }}
      >
        Audio <span style={{color: COLORS.muted}}>→</span> Notes <span style={{color: COLORS.muted}}>→</span>{' '}
        <span style={{color: COLORS.gold}}>Game</span>
      </div>
      <div
        style={{
          position: 'absolute',
          left: '50%',
          top: 560,
          width: 640 * lineP,
          height: 1,
          transform: 'translateX(-50%)',
          background: `linear-gradient(90deg, transparent, ${COLORS.gold}, transparent)`,
        }}
      />
      <div
        style={{
          position: 'absolute',
          left: 0,
          right: 0,
          bottom: 150,
          display: 'flex',
          justifyContent: 'center',
          alignItems: 'center',
          gap: 16,
          fontFamily: FONT.mono,
          fontSize: 19,
          color: COLORS.green,
        }}
      >
        <Sparkles size={22} /> github.com/superdaobo/scoreleap
      </div>
      <div
        style={{
          position: 'absolute',
          left: 0,
          right: 0,
          bottom: 82,
          textAlign: 'center',
          fontFamily: FONT.body,
          fontSize: 18,
          color: COLORS.muted,
        }}
      >
        让乐谱跨过屏幕，真正落到第五人格的 36 个琴键上。
      </div>
    </div>
  );
};
