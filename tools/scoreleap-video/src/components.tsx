import type {CSSProperties, ReactNode} from 'react';
import {FileMusic, Music2} from 'lucide-react';
import {
  AbsoluteFill,
  Img,
  interpolate,
  spring,
  staticFile,
  useCurrentFrame,
  useVideoConfig,
} from 'remotion';
import type {VideoNote} from './types';
import {COLORS, FONT} from './theme';

const clamp = {extrapolateLeft: 'clamp', extrapolateRight: 'clamp'} as const;

export const fadeInOut = (frame: number, duration: number, edge = 18) => {
  return Math.min(
    interpolate(frame, [0, edge], [0, 1], clamp),
    interpolate(frame, [duration - edge, duration], [1, 0], clamp),
  );
};

export const TechBackground = ({
  children,
  accent = 'gold',
  quiet = false,
}: {
  children?: ReactNode;
  accent?: 'gold' | 'green' | 'mixed';
  quiet?: boolean;
}) => {
  const frame = useCurrentFrame();
  const gold = accent !== 'green' ? COLORS.gold : 'transparent';
  const green = accent !== 'gold' ? COLORS.greenLine : 'transparent';
  const gridShift = (frame * 0.28) % 32;
  const glowX = 46 + Math.sin(frame / 95) * 8;
  const glowY = 24 + Math.cos(frame / 130) * 5;

  return (
    <AbsoluteFill
      style={{
        background: COLORS.bg,
        color: COLORS.text,
        overflow: 'hidden',
        fontFamily: FONT.body,
      }}
    >
      <AbsoluteFill
        style={{
          opacity: quiet ? 0.22 : 0.45,
          backgroundImage: `linear-gradient(rgba(46,204,113,.055) 1px, transparent 1px), linear-gradient(90deg, rgba(46,204,113,.055) 1px, transparent 1px)`,
          backgroundSize: '32px 32px',
          backgroundPosition: `${gridShift}px ${gridShift}px`,
        }}
      />
      <AbsoluteFill
        style={{
          background: `radial-gradient(circle at ${glowX}% ${glowY}%, ${gold}20 0%, transparent 32%), radial-gradient(circle at 82% 80%, ${green}18 0%, transparent 30%)`,
          opacity: quiet ? 0.42 : 0.8,
        }}
      />
      <AbsoluteFill
        style={{
          background:
            'linear-gradient(180deg, rgba(5,5,5,.12) 0%, rgba(5,5,5,.02) 46%, rgba(5,5,5,.55) 100%)',
        }}
      />
      <AbsoluteFill
        style={{
          boxShadow: 'inset 0 0 190px rgba(0,0,0,.94)',
          border: '1px solid rgba(255,215,0,.045)',
        }}
      />
      {children}
    </AbsoluteFill>
  );
};

export const TechTag = ({
  children,
  tone = 'gold',
  style,
}: {
  children: ReactNode;
  tone?: 'gold' | 'green' | 'neutral' | 'red';
  style?: CSSProperties;
}) => {
  const color =
    tone === 'gold'
      ? COLORS.gold
      : tone === 'green'
        ? COLORS.green
        : tone === 'red'
          ? COLORS.red
          : COLORS.muted;
  return (
    <div
      style={{
        display: 'inline-flex',
        alignItems: 'center',
        gap: 10,
        padding: '8px 12px',
        border: `1px solid ${color}55`,
        background: `${COLORS.surface}d9`,
        color,
        fontFamily: FONT.mono,
        fontSize: 15,
        fontWeight: 600,
        letterSpacing: '0.09em',
        textTransform: 'uppercase',
        ...style,
      }}
    >
      <span style={{width: 6, height: 6, background: color, boxShadow: `0 0 16px ${color}`}} />
      {children}
    </div>
  );
};

export const BigTitle = ({
  kicker,
  title,
  subtitle,
  align = 'left',
  accentWord,
}: {
  kicker?: string;
  title: string;
  subtitle?: string;
  align?: 'left' | 'center';
  accentWord?: string;
}) => {
  const frame = useCurrentFrame();
  const {fps} = useVideoConfig();
  const enter = spring({frame, fps, config: {damping: 16, stiffness: 110, mass: 0.8}});
  const y = interpolate(enter, [0, 1], [30, 0]);
  const renderTitle = () => {
    if (!accentWord || !title.includes(accentWord)) return title;
    const [before, after] = title.split(accentWord);
    return (
      <>
        {before}
        <span style={{color: COLORS.gold, textShadow: `0 0 42px ${COLORS.gold}44`}}>{accentWord}</span>
        {after}
      </>
    );
  };

  return (
    <div
      style={{
        textAlign: align,
        opacity: enter,
        transform: `translateY(${y}px)`,
        maxWidth: align === 'center' ? 1420 : 1320,
      }}
    >
      {kicker ? <TechTag style={{marginBottom: 24}}>{kicker}</TechTag> : null}
      <div
        style={{
          fontFamily: FONT.display,
          fontWeight: 700,
          fontSize: 92,
          lineHeight: 1.02,
          letterSpacing: '-0.035em',
          color: COLORS.text,
          textWrap: 'balance',
        }}
      >
        {renderTitle()}
      </div>
      {subtitle ? (
        <div
          style={{
            marginTop: 24,
            fontFamily: FONT.body,
            fontSize: 28,
            lineHeight: 1.45,
            color: '#bdb9b6',
            letterSpacing: '-0.01em',
          }}
        >
          {subtitle}
        </div>
      ) : null}
    </div>
  );
};

export const LogoMark = ({size = 72, label = true}: {size?: number; label?: boolean}) => (
  <div style={{display: 'flex', alignItems: 'center', gap: 18}}>
    <Img
      src={staticFile('brand/scoreleap-icon.png')}
      style={{
        width: size,
        height: size,
        objectFit: 'contain',
        filter: 'drop-shadow(0 0 24px rgba(255,215,0,.18))',
      }}
    />
    {label ? (
      <div>
        <div style={{fontFamily: FONT.display, fontSize: size * 0.48, lineHeight: 1, fontWeight: 700}}>谱跃</div>
        <div
          style={{
            fontFamily: FONT.mono,
            color: COLORS.gold,
            fontSize: size * 0.18,
            letterSpacing: '.16em',
            marginTop: 7,
          }}
        >
          SCORELEAP
        </div>
      </div>
    ) : null}
  </div>
);

export const FileCard = ({label = 'PIANO_DEMO.MP3'}: {label?: string}) => {
  const frame = useCurrentFrame();
  const {fps} = useVideoConfig();
  const enter = spring({frame, fps, config: {damping: 13, stiffness: 100}});
  const tilt = interpolate(enter, [0, 1], [-9, -2]);
  const lift = interpolate(enter, [0, 1], [100, 0]);
  return (
    <div
      style={{
        width: 560,
        padding: '30px 34px',
        border: `1px solid ${COLORS.gold}66`,
        background: 'linear-gradient(135deg, rgba(42,42,42,.96), rgba(14,14,14,.96))',
        boxShadow: `0 30px 90px rgba(0,0,0,.6), 0 0 65px ${COLORS.gold}10`,
        transform: `perspective(900px) rotateY(${tilt}deg) translateY(${lift}px) scale(${0.9 + enter * 0.1})`,
        opacity: enter,
      }}
    >
      <div style={{display: 'flex', alignItems: 'center', gap: 26}}>
        <div
          style={{
            width: 90,
            height: 90,
            display: 'grid',
            placeItems: 'center',
            border: `1px solid ${COLORS.gold}55`,
            background: `${COLORS.gold}10`,
          }}
        >
          <FileMusic size={48} strokeWidth={1.4} color={COLORS.gold} />
        </div>
        <div style={{minWidth: 0}}>
          <div
            style={{
              fontFamily: FONT.mono,
              fontSize: 15,
              letterSpacing: '.12em',
              color: COLORS.green,
              marginBottom: 10,
            }}
          >
            AUDIO INPUT · MP3
          </div>
          <div
            style={{
              fontFamily: FONT.body,
              fontWeight: 600,
              fontSize: 27,
              whiteSpace: 'nowrap',
              overflow: 'hidden',
              textOverflow: 'ellipsis',
              maxWidth: 365,
            }}
          >
            {label}
          </div>
          <div style={{fontFamily: FONT.mono, fontSize: 14, color: COLORS.muted, marginTop: 10}}>
            DROP INTO SCORELEAP
          </div>
        </div>
      </div>
    </div>
  );
};

export const Waveform = ({
  imageSrc,
  width = 1120,
  height = 210,
  progress,
}: {
  imageSrc?: string | null;
  width?: number;
  height?: number;
  progress?: number;
}) => {
  const frame = useCurrentFrame();
  const {fps} = useVideoConfig();
  const p = progress ?? ((frame / fps) % 8) / 8;
  const reveal = interpolate(frame, [0, 28], [0, 1], clamp);

  if (imageSrc) {
    return (
      <div
        style={{
          position: 'relative',
          width,
          height,
          overflow: 'hidden',
          borderTop: `1px solid ${COLORS.greenLine}28`,
          borderBottom: `1px solid ${COLORS.greenLine}28`,
          opacity: reveal,
          background: 'rgba(5,5,5,.45)',
        }}
      >
        <Img
          src={staticFile(imageSrc)}
          style={{width: '100%', height: '100%', objectFit: 'fill', opacity: 0.5, filter: 'saturate(.4)'}}
        />
        <div
          style={{
            position: 'absolute',
            inset: 0,
            width: `${Math.max(2, p * 100)}%`,
            overflow: 'hidden',
            borderRight: `2px solid ${COLORS.gold}`,
            boxShadow: `12px 0 34px ${COLORS.gold}18`,
          }}
        >
          <Img
            src={staticFile(imageSrc)}
            style={{width, height: '100%', maxWidth: 'none', objectFit: 'fill', filter: 'sepia(1) saturate(2.5) hue-rotate(355deg)'}}
          />
        </div>
      </div>
    );
  }

  return (
    <div
      style={{
        width,
        height,
        display: 'flex',
        alignItems: 'center',
        gap: 4,
        overflow: 'hidden',
        opacity: reveal,
      }}
    >
      {Array.from({length: 150}, (_, index) => {
        const a = Math.sin(index * 0.42) * 0.28 + Math.sin(index * 0.13 + 1.4) * 0.18;
        const pulse = 0.65 + Math.sin(index * 0.32 + frame * 0.035) * 0.2;
        const h = 20 + Math.abs(a) * 150 + ((index * 17) % 31) * 1.1;
        const active = index / 150 <= p;
        return (
          <div
            key={index}
            style={{
              width: 4,
              height: Math.min(height - 10, h * pulse),
              background: active ? COLORS.gold : COLORS.greenLine,
              opacity: active ? 0.9 : 0.3,
              boxShadow: active ? `0 0 12px ${COLORS.gold}55` : 'none',
            }}
          />
        );
      })}
    </div>
  );
};

const midiName = (pitch: number) => {
  const names = ['C', 'C♯', 'D', 'D♯', 'E', 'F', 'F♯', 'G', 'G♯', 'A', 'A♯', 'B'];
  return `${names[pitch % 12]}${Math.floor(pitch / 12) - 1}`;
};

export const PianoRoll = ({
  notes,
  timeOffset = 0,
  width = 1380,
  height = 420,
  minPitch = 45,
  maxPitch = 101,
  highlightHigh = false,
}: {
  notes: VideoNote[];
  timeOffset?: number;
  width?: number;
  height?: number;
  minPitch?: number;
  maxPitch?: number;
  highlightHigh?: boolean;
}) => {
  const frame = useCurrentFrame();
  const {fps} = useVideoConfig();
  const t = timeOffset + frame / fps;
  const lookBehind = 0.9;
  const lookAhead = 5.3;
  const span = lookBehind + lookAhead;
  const visible = notes.filter((n) => n.end >= t - lookBehind && n.start <= t + lookAhead);

  return (
    <div
      style={{
        position: 'relative',
        width,
        height,
        background: 'linear-gradient(180deg, rgba(19,19,19,.75), rgba(5,5,5,.92))',
        border: `1px solid ${COLORS.greenLine}26`,
        overflow: 'hidden',
        boxShadow: 'inset 0 0 70px rgba(0,0,0,.75)',
      }}
    >
      {Array.from({length: 12}, (_, i) => (
        <div
          key={`v-${i}`}
          style={{
            position: 'absolute',
            left: `${(i / 12) * 100}%`,
            top: 0,
            bottom: 0,
            borderLeft: '1px solid rgba(255,255,255,.035)',
          }}
        />
      ))}
      {Array.from({length: 8}, (_, i) => (
        <div
          key={`h-${i}`}
          style={{
            position: 'absolute',
            top: `${(i / 8) * 100}%`,
            left: 0,
            right: 0,
            borderTop: '1px solid rgba(46,204,113,.045)',
          }}
        />
      ))}
      <div
        style={{
          position: 'absolute',
          left: `${(lookBehind / span) * 100}%`,
          top: 0,
          bottom: 0,
          width: 2,
          background: COLORS.gold,
          boxShadow: `0 0 25px ${COLORS.gold}`,
          zIndex: 5,
        }}
      />
      {visible.map((note, index) => {
        const x = ((note.start - (t - lookBehind)) / span) * width;
        const durationPx = Math.max(18, ((note.end - note.start) / span) * width);
        const yRatio = (maxPitch - note.pitch) / (maxPitch - minPitch);
        const y = yRatio * (height - 28);
        const high = note.pitch >= 84;
        const active = note.start <= t && note.end >= t;
        const color = high && highlightHigh ? COLORS.gold : active ? COLORS.goldSoft : COLORS.green;
        return (
          <div
            key={`${note.start}-${note.pitch}-${index}`}
            style={{
              position: 'absolute',
              left: x,
              top: y,
              width: durationPx,
              height: 17,
              background: `${color}${active ? 'f2' : 'a8'}`,
              border: `1px solid ${color}`,
              boxShadow: active ? `0 0 20px ${color}66` : 'none',
              borderRadius: 2,
            }}
          >
            {durationPx > 64 ? (
              <span
                style={{
                  position: 'absolute',
                  left: 6,
                  top: 0,
                  lineHeight: '15px',
                  color: COLORS.bg,
                  fontFamily: FONT.mono,
                  fontSize: 9,
                  fontWeight: 700,
                }}
              >
                {midiName(note.pitch)}
              </span>
            ) : null}
          </div>
        );
      })}
      <div
        style={{
          position: 'absolute',
          left: 18,
          bottom: 16,
          display: 'flex',
          alignItems: 'center',
          gap: 10,
          fontFamily: FONT.mono,
          fontSize: 12,
          letterSpacing: '.09em',
          color: COLORS.muted,
        }}
      >
        <Music2 size={15} /> LIVE NOTE EVENTS · {t.toFixed(2)}s
      </div>
    </div>
  );
};

export const AppWindow = ({
  children,
  title = 'ScoreLeap · 谱跃',
  width = 1450,
  height = 760,
  perspective = true,
}: {
  children: ReactNode;
  title?: string;
  width?: number;
  height?: number;
  perspective?: boolean;
}) => {
  const frame = useCurrentFrame();
  const {fps} = useVideoConfig();
  const enter = spring({frame, fps, config: {damping: 17, stiffness: 90, mass: 0.9}});
  return (
    <div
      style={{
        width,
        height,
        background: '#0d0d0d',
        border: '1px solid rgba(255,215,0,.2)',
        boxShadow: '0 42px 120px rgba(0,0,0,.72), 0 0 80px rgba(255,215,0,.05)',
        overflow: 'hidden',
        opacity: enter,
        transform: perspective
          ? `perspective(1500px) rotateX(${interpolate(enter, [0, 1], [9, 1])}deg) rotateY(${interpolate(
              enter,
              [0, 1],
              [-7, -1],
            )}deg) translateY(${interpolate(enter, [0, 1], [70, 0])}px)`
          : undefined,
      }}
    >
      <div
        style={{
          height: 50,
          display: 'flex',
          alignItems: 'center',
          padding: '0 18px',
          gap: 12,
          borderBottom: '1px solid rgba(46,204,113,.12)',
          background: '#151515',
        }}
      >
        <div style={{display: 'flex', gap: 7}}>
          {['#ff6b6b', '#ffd166', '#2ecc71'].map((color) => (
            <span key={color} style={{width: 9, height: 9, borderRadius: 99, background: color, opacity: 0.72}} />
          ))}
        </div>
        <div style={{fontFamily: FONT.mono, fontSize: 13, color: COLORS.muted, marginLeft: 8}}>{title}</div>
        <div
          style={{
            marginLeft: 'auto',
            fontFamily: FONT.mono,
            fontSize: 11,
            color: COLORS.green,
            letterSpacing: '.1em',
          }}
        >
          LOCAL · READY
        </div>
      </div>
      <div style={{height: height - 50, position: 'relative'}}>{children}</div>
    </div>
  );
};

export const Stat = ({label, value, tone = 'gold'}: {label: string; value: string; tone?: 'gold' | 'green'}) => (
  <div style={{minWidth: 170}}>
    <div style={{fontFamily: FONT.mono, fontSize: 11, color: COLORS.muted, letterSpacing: '.12em'}}>{label}</div>
    <div
      style={{
        fontFamily: FONT.mono,
        fontSize: 28,
        marginTop: 7,
        color: tone === 'gold' ? COLORS.gold : COLORS.green,
      }}
    >
      {value}
    </div>
  </div>
);

export const FlowArrow = ({progress = 1, width = 180}: {progress?: number; width?: number}) => (
  <div style={{width, height: 2, background: 'rgba(255,255,255,.08)', position: 'relative'}}>
    <div
      style={{
        position: 'absolute',
        left: 0,
        top: 0,
        height: 2,
        width: `${Math.max(0, Math.min(1, progress)) * 100}%`,
        background: `linear-gradient(90deg, ${COLORS.greenLine}, ${COLORS.gold})`,
        boxShadow: `0 0 18px ${COLORS.gold}66`,
      }}
    />
    <div
      style={{
        position: 'absolute',
        right: -2,
        top: -4,
        width: 0,
        height: 0,
        borderTop: '5px solid transparent',
        borderBottom: '5px solid transparent',
        borderLeft: `8px solid ${COLORS.gold}`,
      }}
    />
  </div>
);
