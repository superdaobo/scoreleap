import {
  ArrowDownToLine,
  AudioLines,
  Check,
  CloudOff,
  Cpu,
  KeyboardMusic,
  LockKeyhole,
  Music2,
  ShieldCheck,
  Sparkles,
  UserRoundX,
  WandSparkles,
  X,
} from 'lucide-react';
import {
  AbsoluteFill,
  Img,
  OffthreadVideo,
  interpolate,
  spring,
  staticFile,
  useCurrentFrame,
  useVideoConfig,
} from 'remotion';
import {
  AppWindow,
  BigTitle,
  FileCard,
  FlowArrow,
  LogoMark,
  PianoRoll,
  Stat,
  TechBackground,
  TechTag,
  Waveform,
  fadeInOut,
} from './components';
import {COLORS, FONT, SAFE_X, SAFE_Y} from './theme';
import type {TrailerProps, VideoNote} from './types';

const clamp = {extrapolateLeft: 'clamp', extrapolateRight: 'clamp'} as const;

const noteStats = (notes: VideoNote[]) => {
  const pitches = notes.map((n) => n.pitch);
  return {
    count: notes.length,
    min: pitches.length ? Math.min(...pitches) : 0,
    max: pitches.length ? Math.max(...pitches) : 0,
    high: notes.filter((n) => n.pitch >= 84).length,
    playable: notes.filter((n) => n.pitch >= 48 && n.pitch <= 83).length,
  };
};

export const ColdOpenScene = () => {
  const frame = useCurrentFrame();
  const {fps} = useVideoConfig();
  const pulse = spring({frame: Math.max(0, frame - 18), fps, config: {damping: 13, stiffness: 85}});
  const question = interpolate(frame, [55, 95], [0, 1], clamp);
  const line = interpolate(frame, [145, 210], [0, 1], clamp);
  return (
    <TechBackground quiet accent="mixed">
      <div
        style={{
          position: 'absolute',
          left: '50%',
          top: 290,
          width: 12,
          height: 12,
          borderRadius: 99,
          background: COLORS.gold,
          transform: `translateX(-50%) scale(${0.35 + pulse * 1.1})`,
          boxShadow: `0 0 ${30 + pulse * 70}px ${COLORS.gold}`,
        }}
      />
      <div
        style={{
          position: 'absolute',
          left: '50%',
          top: 335,
          transform: 'translateX(-50%)',
          color: COLORS.gold,
          fontFamily: FONT.mono,
          fontSize: 13,
          letterSpacing: '.18em',
          opacity: pulse,
        }}
      >
        ONE NOTE
      </div>
      <div
        style={{
          position: 'absolute',
          top: 465,
          left: SAFE_X,
          right: SAFE_X,
          textAlign: 'center',
          opacity: question,
          transform: `translateY(${interpolate(question, [0, 1], [22, 0])}px)`,
          fontFamily: FONT.display,
          fontSize: 72,
          fontWeight: 700,
          letterSpacing: '-.028em',
        }}
      >
        一段 <span style={{color: COLORS.gold}}>MP3</span>，能直接变成游戏里的演奏吗？
      </div>
      <div
        style={{
          position: 'absolute',
          left: '50%',
          top: 620,
          width: 420 * line,
          height: 1,
          transform: 'translateX(-50%)',
          background: `linear-gradient(90deg, transparent, ${COLORS.gold}, transparent)`,
          boxShadow: `0 0 22px ${COLORS.gold}55`,
        }}
      />
      <div
        style={{
          position: 'absolute',
          bottom: 65,
          left: 0,
          right: 0,
          textAlign: 'center',
          fontFamily: FONT.mono,
          color: COLORS.muted,
          fontSize: 12,
          letterSpacing: '.18em',
          opacity: interpolate(frame, [200, 250], [0, 0.7], clamp),
        }}
      >
        AUDIO → NOTES → GAME
      </div>
    </TechBackground>
  );
};

export const Mp3HookScene = ({props}: {props: TrailerProps}) => {
  const frame = useCurrentFrame();
  const progress = interpolate(frame, [90, 230], [0, 1], clamp);
  return (
    <TechBackground accent="mixed">
      <div style={{position: 'absolute', left: SAFE_X, top: SAFE_Y}}>
        <TechTag tone="green">FROM AUDIO. NOT MIDI.</TechTag>
      </div>
      <div
        style={{
          position: 'absolute',
          left: SAFE_X,
          right: SAFE_X,
          top: 185,
          display: 'grid',
          gridTemplateColumns: '560px 1fr 330px 1fr 580px',
          alignItems: 'center',
          gap: 24,
        }}
      >
        <FileCard label={props.sampleLabel} />
        <FlowArrow progress={progress} width={150} />
        <div
          style={{
            display: 'grid',
            placeItems: 'center',
            border: `1px solid ${COLORS.gold}48`,
            height: 180,
            background: 'rgba(19,19,19,.78)',
            boxShadow: `0 0 80px ${COLORS.gold}0d`,
          }}
        >
          <LogoMark size={78} />
        </div>
        <FlowArrow progress={interpolate(frame, [170, 300], [0, 1], clamp)} width={150} />
        <div style={{transform: 'scale(.72)', transformOrigin: 'left center'}}>
          <PianoRoll notes={props.notes} width={760} height={330} timeOffset={0} highlightHigh />
        </div>
      </div>
      <div style={{position: 'absolute', left: SAFE_X, right: SAFE_X, bottom: 120}}>
        <BigTitle
          title="没有 MIDI？从音频开始。"
          accentWord="音频"
          subtitle="MP3 / WAV / FLAC → 本地 AI 转录 → 游戏可弹奏曲谱"
        />
      </div>
    </TechBackground>
  );
};

export const TranscriptionScene = ({props}: {props: TrailerProps}) => {
  const frame = useCurrentFrame();
  const stats = noteStats(props.notes);
  const process = interpolate(frame, [40, 420], [0, 1], clamp);
  const glow = 0.35 + Math.sin(frame / 18) * 0.1;
  return (
    <TechBackground accent="green">
      <div style={{position: 'absolute', left: SAFE_X, top: SAFE_Y}}>
        <BigTitle
          kicker={props.dataMode === 'local-real' ? 'REAL LOCAL AUDIO DATA' : 'DEMO DATA'}
          title="把声音，变成音符。"
          accentWord="音符"
          subtitle="高质量钢琴转录在本地运行，输出真实 Note Events。"
        />
      </div>
      <div style={{position: 'absolute', left: SAFE_X, right: SAFE_X, top: 355}}>
        <Waveform imageSrc={props.waveformSrc} width={1696} height={190} progress={process} />
      </div>
      <div
        style={{
          position: 'absolute',
          left: SAFE_X,
          top: 600,
          width: 330,
          height: 190,
          display: 'grid',
          placeItems: 'center',
          border: `1px solid ${COLORS.greenLine}55`,
          background: 'rgba(19,19,19,.85)',
          boxShadow: `0 0 70px rgba(46,204,113,${glow * 0.18})`,
        }}
      >
        <Cpu color={COLORS.green} size={58} strokeWidth={1.25} />
        <div style={{position: 'absolute', bottom: 24, fontFamily: FONT.mono, fontSize: 13, color: COLORS.green}}>
          TRANSKUN · LOCAL AI
        </div>
      </div>
      <div style={{position: 'absolute', left: 500, top: 570}}>
        <PianoRoll notes={props.notes} width={1305} height={320} timeOffset={2.2} highlightHigh />
      </div>
      <div style={{position: 'absolute', left: SAFE_X, right: SAFE_X, bottom: 62, display: 'flex', gap: 44}}>
        <Stat label="NOTE EVENTS" value={String(stats.count)} />
        <Stat label="PITCH RANGE" value={`${stats.min || '--'}—${stats.max || '--'}`} tone="green" />
        <Stat label="HIGH REGISTER" value={String(stats.high)} />
        <Stat label="DIRECTLY PLAYABLE" value={String(stats.playable)} tone="green" />
      </div>
    </TechBackground>
  );
};

const PianoRange = ({progress}: {progress: number}) => {
  const keys = Array.from({length: 88}, (_, i) => i + 21);
  return (
    <div style={{position: 'relative', width: 1640, height: 230}}>
      <div style={{display: 'flex', gap: 3, height: 150, alignItems: 'stretch'}}>
        {keys.map((pitch) => {
          const playable = pitch >= 48 && pitch <= 83;
          const isC = pitch % 12 === 0;
          return (
            <div
              key={pitch}
              style={{
                flex: 1,
                minWidth: 0,
                background: playable ? `${COLORS.gold}${isC ? 'd8' : '9a'}` : 'rgba(229,226,225,.12)',
                border: playable ? `1px solid ${COLORS.gold}66` : '1px solid rgba(255,255,255,.05)',
                boxShadow: playable && progress > 0.2 ? `0 0 ${8 + 14 * progress}px ${COLORS.gold}22` : 'none',
                transform: `scaleY(${playable ? 0.9 + 0.1 * progress : 0.82})`,
                transformOrigin: 'bottom',
              }}
            />
          );
        })}
      </div>
      <div
        style={{
          position: 'absolute',
          left: `${((48 - 21) / 88) * 100}%`,
          width: `${((83 - 48 + 1) / 88) * 100}%`,
          top: 166,
          height: 48,
          borderTop: `2px solid ${COLORS.gold}`,
          color: COLORS.gold,
          fontFamily: FONT.mono,
          fontSize: 16,
          letterSpacing: '.12em',
          display: 'flex',
          justifyContent: 'center',
          paddingTop: 12,
        }}
      >
        GAME PROFILE · MIDI 48—83 · 36 KEYS
      </div>
    </div>
  );
};

export const RangeScene = () => {
  const frame = useCurrentFrame();
  const {fps} = useVideoConfig();
  const progress = spring({frame: Math.max(0, frame - 20), fps, config: {damping: 18, stiffness: 80}});
  const mapP = interpolate(frame, [150, 280], [0, 1], clamp);
  const xFrom = 1395;
  const xTo = 1120;
  const x = interpolate(mapP, [0, 1], [xFrom, xTo]);
  const y = interpolate(mapP, [0, 1], [340, 660]);
  return (
    <TechBackground accent="gold">
      <div style={{position: 'absolute', left: SAFE_X, top: SAFE_Y}}>
        <BigTitle kicker="GAME ARRANGEMENT" title="88 键，压进 36 键。" accentWord="36 键" />
      </div>
      <div style={{position: 'absolute', left: 140, top: 390}}>
        <PianoRange progress={progress} />
      </div>
      <div
        style={{
          position: 'absolute',
          left: x,
          top: y,
          width: 126,
          height: 74,
          display: 'grid',
          placeItems: 'center',
          border: `1px solid ${COLORS.gold}`,
          background: COLORS.surface,
          boxShadow: `0 0 36px ${COLORS.gold}44`,
          fontFamily: FONT.mono,
          color: COLORS.gold,
          transform: `scale(${1 - mapP * 0.12})`,
        }}
      >
        <strong style={{fontSize: 22}}>C7</strong>
        <span style={{fontSize: 12}}>MIDI 96</span>
      </div>
      <div
        style={{
          position: 'absolute',
          left: 1245,
          top: 560,
          fontFamily: FONT.mono,
          fontSize: 14,
          color: COLORS.green,
          opacity: interpolate(frame, [190, 250], [0, 1], clamp),
        }}
      >
        SMART FOLD
        <div style={{marginTop: 7, color: COLORS.muted}}>same pitch class · avoid collision · keep contour</div>
      </div>
      <div
        style={{
          position: 'absolute',
          left: 1080,
          top: 760,
          padding: '12px 18px',
          border: `1px solid ${COLORS.greenLine}55`,
          background: 'rgba(19,19,19,.9)',
          color: COLORS.green,
          fontFamily: FONT.mono,
          opacity: interpolate(mapP, [0.65, 1], [0, 1], clamp),
        }}
      >
        C5 · MIDI 72 · PLAYABLE
      </div>
    </TechBackground>
  );
};

const chord = [48, 55, 60, 64, 67, 72, 76];

const ChordLane = ({mode, progress}: {mode: 'old' | 'new'; progress: number}) => {
  const oldKept = new Set([48, 55, 60, 64]);
  const newKept = new Set([48, 64, 72, 76]);
  const kept = mode === 'old' ? oldKept : newKept;
  return (
    <div style={{display: 'flex', gap: 24, alignItems: 'flex-end'}}>
      {chord.map((pitch, index) => {
        const keep = kept.has(pitch);
        const top = pitch === 76;
        const bass = pitch === 48;
        const color = mode === 'new' && top ? COLORS.gold : mode === 'new' && bass ? COLORS.greenLine : COLORS.text;
        return (
          <div key={pitch} style={{width: 160, textAlign: 'center', opacity: 0.35 + index * 0.08}}>
            <div
              style={{
                height: 68 + (pitch - 48) * 4.2,
                minHeight: 90,
                border: `1px solid ${keep ? color : COLORS.red}88`,
                background: keep ? `${color}16` : `${COLORS.red}0c`,
                display: 'flex',
                alignItems: 'flex-end',
                justifyContent: 'center',
                paddingBottom: 18,
                position: 'relative',
                boxShadow: keep ? `0 0 ${28 * progress}px ${color}22` : 'none',
              }}
            >
              <div style={{fontFamily: FONT.mono, fontSize: 21, color: keep ? color : COLORS.red}}>
                {pitch === 48 ? 'C3' : pitch === 55 ? 'G3' : pitch === 60 ? 'C4' : pitch === 64 ? 'E4' : pitch === 67 ? 'G4' : pitch === 72 ? 'C5' : 'E5'}
              </div>
              <div style={{position: 'absolute', top: 14, right: 14}}>
                {keep ? <Check size={18} color={color} /> : <X size={18} color={COLORS.red} />}
              </div>
            </div>
            <div
              style={{
                marginTop: 12,
                fontFamily: FONT.mono,
                fontSize: 11,
                color: top && mode === 'new' ? COLORS.gold : bass && mode === 'new' ? COLORS.green : COLORS.muted,
                letterSpacing: '.08em',
              }}
            >
              {top && mode === 'new' ? 'TOP VOICE' : bass && mode === 'new' ? 'BASS' : keep ? 'VOICE' : 'DROPPED'}
            </div>
          </div>
        );
      })}
    </div>
  );
};

export const MelodyScene = () => {
  const frame = useCurrentFrame();
  const {fps} = useVideoConfig();
  const before = frame < 205;
  const switchP = spring({frame: Math.max(0, frame - 205), fps, config: {damping: 14, stiffness: 105}});
  const glitch = before ? Math.sin(frame * 1.3) * 1.5 : 0;
  return (
    <TechBackground accent={before ? 'mixed' : 'gold'}>
      <div style={{position: 'absolute', left: SAFE_X, top: SAFE_Y}}>
        <TechTag tone={before ? 'red' : 'gold'}>{before ? 'MAX POLYPHONY · 4' : 'MELODY-AWARE · 4 VOICES'}</TechTag>
      </div>
      <div
        style={{
          position: 'absolute',
          top: 150,
          left: SAFE_X,
          right: SAFE_X,
          fontFamily: FONT.display,
          fontWeight: 700,
          fontSize: 80,
          transform: `translateX(${glitch}px)`,
        }}
      >
        {before ? (
          <>
            只保留“更响”的音？ <span style={{color: COLORS.red}}>旋律会消失。</span>
          </>
        ) : (
          <>
            复音有限，也先保住 <span style={{color: COLORS.gold}}>主旋律。</span>
          </>
        )}
      </div>
      <div
        style={{
          position: 'absolute',
          left: 220,
          top: 385,
          transform: before ? 'none' : `translateX(${interpolate(switchP, [0, 1], [20, 0])}px)`,
          opacity: before ? 1 : switchP,
        }}
      >
        <ChordLane mode={before ? 'old' : 'new'} progress={before ? 0.2 : switchP} />
      </div>
      <div
        style={{
          position: 'absolute',
          bottom: 95,
          left: SAFE_X,
          right: SAFE_X,
          display: 'flex',
          justifyContent: 'space-between',
          alignItems: 'center',
        }}
      >
        <div style={{fontFamily: FONT.mono, color: COLORS.muted, fontSize: 15, letterSpacing: '.12em'}}>
          {before ? 'LOW NOTES FILL THE SLOTS' : 'TOP VOICE + BASS + IMPORTANT HARMONY'}
        </div>
        <div
          style={{
            fontFamily: FONT.mono,
            fontSize: 34,
            fontWeight: 700,
            letterSpacing: '.12em',
            color: before ? COLORS.red : COLORS.gold,
            textShadow: `0 0 38px ${before ? COLORS.red : COLORS.gold}44`,
          }}
        >
          {before ? 'MELODY LOST' : 'MELODY FIRST'}
        </div>
      </div>
    </TechBackground>
  );
};

const LibraryUi = () => (
  <AbsoluteFill style={{background: '#090909'}}>
    <Img
      src={staticFile('brand/bg-library.webp')}
      style={{position: 'absolute', width: '100%', height: '100%', objectFit: 'cover', opacity: 0.16}}
    />
    <div style={{position: 'absolute', left: 34, top: 34, bottom: 34, width: 260, borderRight: '1px solid rgba(46,204,113,.15)'}}>
      <LogoMark size={50} />
      {['曲谱库', '编排', '设置'].map((item, i) => (
        <div
          key={item}
          style={{
            marginTop: i === 0 ? 55 : 16,
            width: 190,
            padding: '15px 16px',
            border: i === 0 ? `1px solid ${COLORS.gold}55` : '1px solid transparent',
            color: i === 0 ? COLORS.gold : COLORS.muted,
            fontFamily: FONT.body,
            fontSize: 17,
          }}
        >
          {item}
        </div>
      ))}
    </div>
    <div style={{position: 'absolute', left: 340, top: 52, right: 56}}>
      <div style={{display: 'flex', alignItems: 'center'}}>
        <div>
          <div style={{fontFamily: FONT.display, fontSize: 44, fontWeight: 700}}>曲谱库</div>
          <div style={{fontFamily: FONT.mono, color: COLORS.green, fontSize: 12, letterSpacing: '.1em', marginTop: 8}}>
            LOCAL MUSIC LIBRARY
          </div>
        </div>
        <div style={{marginLeft: 'auto', display: 'flex', gap: 12}}>
          <div style={{padding: '13px 18px', border: '1px solid rgba(46,204,113,.25)', fontFamily: FONT.mono, color: COLORS.green}}>
            导入 MIDI
          </div>
          <div
            style={{
              padding: '13px 20px',
              border: `1px solid ${COLORS.gold}`,
              background: COLORS.gold,
              color: '#221b00',
              fontFamily: FONT.mono,
              fontWeight: 700,
              boxShadow: `0 0 34px ${COLORS.gold}28`,
            }}
          >
            从音频转录
          </div>
        </div>
      </div>
      <div style={{display: 'grid', gridTemplateColumns: 'repeat(3, 1fr)', gap: 22, marginTop: 44}}>
        {['花海 · Piano', 'Night Drive', 'Demo Etude'].map((name, index) => (
          <div
            key={name}
            style={{
              height: 210,
              padding: 24,
              border: index === 0 ? `1px solid ${COLORS.gold}55` : '1px solid rgba(46,204,113,.17)',
              background: 'rgba(28,27,27,.68)',
            }}
          >
            <Music2 size={32} color={index === 0 ? COLORS.gold : COLORS.green} strokeWidth={1.3} />
            <div style={{marginTop: 38, fontFamily: FONT.body, fontSize: 23, fontWeight: 600}}>{name}</div>
            <div style={{marginTop: 12, fontFamily: FONT.mono, color: COLORS.muted, fontSize: 12}}>
              {index === 0 ? 'AUDIO TRANSCRIPTION · HIGH QUALITY' : 'MIDI · READY'}
            </div>
          </div>
        ))}
      </div>
    </div>
  </AbsoluteFill>
);

const ArrangeUi = ({notes}: {notes: VideoNote[]}) => (
  <AbsoluteFill style={{background: '#080808'}}>
    <Img
      src={staticFile('brand/bg-arrange.webp')}
      style={{position: 'absolute', width: '100%', height: '100%', objectFit: 'cover', opacity: 0.14}}
    />
    <div style={{position: 'absolute', left: 36, top: 32, right: 36, display: 'flex', alignItems: 'center'}}>
      <LogoMark size={46} />
      <div style={{marginLeft: 'auto', display: 'flex', gap: 12}}>
        <TechTag tone="green">SMART FOLD</TechTag>
        <TechTag>MELODY PROTECTION</TechTag>
      </div>
    </div>
    <div style={{position: 'absolute', left: 50, top: 130, right: 50}}>
      <PianoRoll notes={notes} width={1350} height={410} timeOffset={6.5} minPitch={45} maxPitch={103} highlightHigh />
      <div style={{display: 'flex', gap: 36, marginTop: 30}}>
        <Stat label="RANGE" value="48—83" tone="green" />
        <Stat label="MAX POLYPHONY" value="4" />
        <Stat label="TOP VOICE" value="PROTECTED" />
        <Stat label="MODE" value="HIGH QUALITY" tone="green" />
      </div>
    </div>
  </AbsoluteFill>
);

export const ProductScene = ({props}: {props: TrailerProps}) => {
  const frame = useCurrentFrame();
  const first = frame < 340;
  const change = interpolate(frame, [300, 380], [0, 1], clamp);
  return (
    <TechBackground quiet accent="mixed">
      <div style={{position: 'absolute', left: SAFE_X, top: 48, zIndex: 5}}>
        <TechTag tone="green">REAL PRODUCT FLOW</TechTag>
      </div>
      <div style={{position: 'absolute', left: 220, top: 150}}>
        <AppWindow width={1480} height={800} title={first ? 'ScoreLeap · 曲谱库' : 'ScoreLeap · 游戏编排'}>
          <div style={{position: 'absolute', inset: 0, opacity: 1 - change}}>
            <LibraryUi />
          </div>
          <div style={{position: 'absolute', inset: 0, opacity: change}}>
            <ArrangeUi notes={props.notes} />
          </div>
        </AppWindow>
      </div>
      <div
        style={{
          position: 'absolute',
          right: 90,
          bottom: 55,
          color: COLORS.muted,
          fontFamily: FONT.mono,
          fontSize: 12,
          letterSpacing: '.1em',
        }}
      >
        SCORELEAP DESKTOP · WINDOWS
      </div>
    </TechBackground>
  );
};

export const PrivacyScene = () => {
  const frame = useCurrentFrame();
  const {fps} = useVideoConfig();
  const items = [
    {label: 'LOCAL AI', sub: '音频在本机处理', icon: Cpu, tone: COLORS.gold},
    {label: 'NO CLOUD UPLOAD', sub: '不上传你的音乐', icon: CloudOff, tone: COLORS.greenLine},
    {label: 'NO ACCOUNT', sub: '打开就能用', icon: UserRoundX, tone: COLORS.green},
  ];
  return (
    <TechBackground quiet accent="mixed">
      <div style={{position: 'absolute', top: 160, left: 0, right: 0, textAlign: 'center'}}>
        <BigTitle title="音乐留在你的电脑里。" accentWord="你的电脑" align="center" />
      </div>
      <div style={{position: 'absolute', left: 210, right: 210, top: 490, display: 'grid', gridTemplateColumns: 'repeat(3, 1fr)', gap: 30}}>
        {items.map((item, index) => {
          const p = spring({frame: Math.max(0, frame - 45 - index * 28), fps, config: {damping: 15, stiffness: 90}});
          const Icon = item.icon;
          return (
            <div
              key={item.label}
              style={{
                height: 300,
                border: `1px solid ${item.tone}42`,
                background: 'rgba(19,19,19,.7)',
                display: 'grid',
                placeItems: 'center',
                textAlign: 'center',
                opacity: p,
                transform: `translateY(${interpolate(p, [0, 1], [40, 0])}px)`,
                boxShadow: `0 24px 80px rgba(0,0,0,.38)`,
              }}
            >
              <div>
                <Icon size={54} color={item.tone} strokeWidth={1.25} />
                <div style={{fontFamily: FONT.mono, fontSize: 24, fontWeight: 700, color: item.tone, marginTop: 28, letterSpacing: '.08em'}}>
                  {item.label}
                </div>
                <div style={{fontFamily: FONT.body, fontSize: 18, color: COLORS.muted, marginTop: 15}}>{item.sub}</div>
              </div>
            </div>
          );
        })}
      </div>
    </TechBackground>
  );
};

export const RealityTransitionScene = () => {
  const frame = useCurrentFrame();
  const q = interpolate(frame, [30, 90, 220, 260], [0, 1, 1, 0], clamp);
  const answer = interpolate(frame, [250, 310], [0, 1], clamp);
  const slash = interpolate(frame, [300, 390], [0, 1], clamp);
  return (
    <AbsoluteFill style={{background: '#030303', color: COLORS.text, overflow: 'hidden'}}>
      <div
        style={{
          position: 'absolute',
          left: 0,
          right: 0,
          top: 390,
          textAlign: 'center',
          fontFamily: FONT.display,
          fontSize: 88,
          fontWeight: 700,
          opacity: q,
        }}
      >
        那么，它真的能弹吗？
      </div>
      <div
        style={{
          position: 'absolute',
          left: 0,
          right: 0,
          top: 435,
          textAlign: 'center',
          fontFamily: FONT.mono,
          fontSize: 42,
          fontWeight: 700,
          letterSpacing: '.16em',
          color: COLORS.gold,
          opacity: answer,
          textShadow: `0 0 48px ${COLORS.gold}44`,
        }}
      >
        看 实 机。
      </div>
      <div
        style={{
          position: 'absolute',
          left: '-10%',
          bottom: 0,
          width: `${130 * slash}%`,
          height: 4,
          transform: 'skewX(-35deg)',
          background: `linear-gradient(90deg, transparent, ${COLORS.gold}, ${COLORS.greenLine}, transparent)`,
          boxShadow: `0 0 60px ${COLORS.gold}`,
        }}
      />
    </AbsoluteFill>
  );
};

export const GameplayScene = ({props}: {props: TrailerProps}) => {
  const frame = useCurrentFrame();
  const {durationInFrames} = useVideoConfig();
  const opacity = fadeInOut(frame, durationInFrames, 22);
  if (props.gameplaySrc) {
    return (
      <AbsoluteFill style={{background: '#000', opacity}}>
        <OffthreadVideo
          src={staticFile(props.gameplaySrc)}
          style={{width: '100%', height: '100%', objectFit: 'cover'}}
          volume={1}
        />
        <div style={{position: 'absolute', top: 34, left: 40}}>
          <TechTag>REAL GAMEPLAY</TechTag>
        </div>
        <div
          style={{
            position: 'absolute',
            inset: 24,
            pointerEvents: 'none',
            border: '1px solid rgba(255,215,0,.2)',
            boxShadow: 'inset 0 0 100px rgba(0,0,0,.25)',
          }}
        />
      </AbsoluteFill>
    );
  }
  return (
    <TechBackground quiet accent="mixed">
      <div style={{position: 'absolute', left: SAFE_X, top: SAFE_Y}}>
        <TechTag tone="green">GAMEPLAY SLOT · {Math.round(props.gameplayDurationSeconds)}s</TechTag>
      </div>
      <div style={{position: 'absolute', left: 100, top: 190, width: 760}}>
        <div style={{fontFamily: FONT.mono, color: COLORS.muted, fontSize: 13, letterSpacing: '.1em', marginBottom: 18}}>
          SCORELEAP · PLAYBACK TIMELINE
        </div>
        <PianoRoll notes={props.notes} width={760} height={540} timeOffset={8} highlightHigh />
      </div>
      <div
        style={{
          position: 'absolute',
          right: 100,
          top: 190,
          width: 860,
          height: 540,
          border: `1px solid ${COLORS.gold}55`,
          background: 'linear-gradient(145deg, rgba(32,31,31,.82), rgba(5,5,5,.95))',
          display: 'grid',
          placeItems: 'center',
          boxShadow: `0 0 80px ${COLORS.gold}0a`,
        }}
      >
        <div style={{textAlign: 'center'}}>
          <KeyboardMusic size={80} color={COLORS.gold} strokeWidth={1} />
          <div style={{fontFamily: FONT.mono, fontSize: 26, color: COLORS.gold, letterSpacing: '.14em', marginTop: 28}}>
            REPLACE WITH REAL GAMEPLAY
          </div>
          <div style={{fontFamily: FONT.body, color: COLORS.muted, fontSize: 19, marginTop: 18}}>
            后续放入《第五人格》实机录像，这里自动接管时间线。
          </div>
        </div>
      </div>
      <div
        style={{
          position: 'absolute',
          left: 100,
          right: 100,
          bottom: 95,
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          gap: 22,
          fontFamily: FONT.mono,
          color: COLORS.green,
          fontSize: 15,
          letterSpacing: '.1em',
        }}
      >
        <Music2 size={18} /> COMPILED TIMELINE <FlowArrow width={220} /> GAME INPUT
      </div>
    </TechBackground>
  );
};

export const EndingScene = () => {
  const frame = useCurrentFrame();
  const {fps} = useVideoConfig();
  const p = spring({frame: Math.max(0, frame - 12), fps, config: {damping: 15, stiffness: 80}});
  return (
    <TechBackground quiet accent="mixed">
      <div
        style={{
          position: 'absolute',
          left: 0,
          right: 0,
          top: 210,
          display: 'flex',
          justifyContent: 'center',
          opacity: p,
          transform: `scale(${0.9 + p * 0.1})`,
        }}
      >
        <LogoMark size={130} />
      </div>
      <div
        style={{
          position: 'absolute',
          left: 0,
          right: 0,
          top: 450,
          textAlign: 'center',
          fontFamily: FONT.display,
          fontSize: 64,
          fontWeight: 700,
        }}
      >
        Audio <span style={{color: COLORS.muted}}>→</span> Notes <span style={{color: COLORS.muted}}>→</span>{' '}
        <span style={{color: COLORS.gold}}>Game</span>
      </div>
      <div style={{position: 'absolute', left: 0, right: 0, top: 570, display: 'flex', justifyContent: 'center', gap: 16}}>
        <TechTag>LOCAL AI</TechTag>
        <TechTag tone="green">OPEN SOURCE</TechTag>
        <TechTag tone="neutral">WINDOWS</TechTag>
      </div>
      <div
        style={{
          position: 'absolute',
          left: 0,
          right: 0,
          bottom: 160,
          display: 'flex',
          justifyContent: 'center',
          alignItems: 'center',
          gap: 16,
          fontFamily: FONT.mono,
          fontSize: 20,
          color: COLORS.green,
        }}
      >
        <Sparkles size={24} /> github.com/superdaobo/scoreleap
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
        让乐谱跨过屏幕，真正落到琴键上。
      </div>
    </TechBackground>
  );
};

export const AlgorithmRangePreview = RangeScene;
export const MelodyPreview = MelodyScene;
