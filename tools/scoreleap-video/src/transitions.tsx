import type {CSSProperties, ReactNode} from 'react';
import {AbsoluteFill, interpolate, useCurrentFrame} from 'remotion';
import {COLORS} from './theme';

export const clamp = {extrapolateLeft: 'clamp', extrapolateRight: 'clamp'} as const;

export const easeOut = (t: number) => 1 - Math.pow(1 - t, 3);
export const easeInOut = (t: number) => (t < 0.5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2);

// ---------------------------------------------------------------------------
// 统一场景过渡：每个全屏场景用 enter/exit 覆盖，Sequence 之间 overlap ~18-30 帧。
// 使用有限的几种 motion，而不是每镜头随机一种，保证“同一套转场语言”。
// ---------------------------------------------------------------------------

export type SceneMotion = 'crossfade' | 'push-left' | 'slide-up' | 'zoom-in' | 'rise';

export const Scene = ({
  children,
  durationInFrames,
  enter = 20,
  exit = 22,
  motion = 'crossfade',
  background = '#050505',
}: {
  children: ReactNode;
  durationInFrames: number;
  enter?: number;
  exit?: number;
  motion?: SceneMotion;
  background?: string;
}) => {
  const frame = useCurrentFrame();
  const e = easeInOut(interpolate(frame, [0, enter], [0, 1], clamp));
  const x = easeInOut(interpolate(frame, [durationInFrames - exit, durationInFrames], [1, 0], clamp));
  const opacity = Math.min(e, x);

  let transform = 'none';
  const slide = 4.5; // % of viewport
  if (motion === 'push-left') {
    transform = `translateX(${interpolate(e, [0, 1], [slide, 0])}%) translateX(${interpolate(x, [0, 1], [0, -slide])}%)`;
  } else if (motion === 'slide-up') {
    transform = `translateY(${interpolate(e, [0, 1], [70, 0])}px)`;
  } else if (motion === 'zoom-in') {
    transform = `scale(${interpolate(e, [0, 1], [1.045, 1])})`;
  } else if (motion === 'rise') {
    transform = `translateY(${interpolate(x, [0, 1], [0, -40])}px) scale(${interpolate(x, [0, 1], [1, 1.04])})`;
  }

  return (
    <AbsoluteFill style={{overflow: 'hidden'}}>
      <AbsoluteFill style={{opacity, transform, background, willChange: 'transform, opacity'}}>
        {children}
      </AbsoluteFill>
    </AbsoluteFill>
  );
};

// ---------------------------------------------------------------------------
// 光标 + 点击反馈微交互
// ---------------------------------------------------------------------------

export type CursorKey = {
  f: number;
  x: number;
  y: number;
  /** 到达该关键点后立即做一次点击脉冲 */
  click?: boolean;
};

export const cursorAt = (frame: number, path: CursorKey[]): {x: number; y: number; press: number} => {
  let x = path[0].x;
  let y = path[0].y;
  let press = 0;
  for (let i = 0; i < path.length; i++) {
    const k = path[i];
    const next = path[i + 1];
    if (next && frame >= k.f && frame < next.f) {
      const t = easeOut(interpolate(frame, [k.f, next.f], [0, 1], clamp));
      x = interpolate(t, [0, 1], [k.x, next.x]);
      y = interpolate(t, [0, 1], [k.y, next.y]);
      if (k.click && frame >= k.f) {
        press = Math.sin(Math.PI * Math.min(1, (frame - k.f) / 16));
      }
      break;
    }
    if (i === path.length - 1) {
      x = k.x;
      y = k.y;
      if (k.click && frame >= k.f) {
        press = Math.sin(Math.PI * Math.min(1, (frame - k.f) / 16));
      }
    }
  }
  return {x, y, press};
};

export const Cursor = ({x, y, press}: {x: number; y: number; press: number}) => {
  const scale = 1 - press * 0.16;
  const ring = 10 + press * 34;
  return (
    <div style={{position: 'absolute', left: x, top: y, zIndex: 400, pointerEvents: 'none'}}>
      <svg
        width={28}
        height={28}
        viewBox="0 0 24 24"
        style={{
          transform: `translate(-8px, -5px) scale(${scale})`,
          transformOrigin: '0 0',
          filter: 'drop-shadow(0 3px 8px rgba(0,0,0,.85))',
        }}
      >
        <path d="M4.2 1.8 21 11.6l-7.7 1.7-3.3 6.9Z" fill="#f5f3ec" stroke="#111" strokeWidth="1.4" strokeLinejoin="round" />
      </svg>
      {press > 0 ? (
        <div
          style={{
            position: 'absolute',
            left: 0,
            top: 0,
            width: ring,
            height: ring,
            borderRadius: 999,
            border: `2px solid ${COLORS.gold}`,
            boxShadow: `0 0 18px ${COLORS.gold}66`,
            opacity: 1 - press,
            transform: 'translate(-50%,-50%)',
          }}
        />
      ) : null}
    </div>
  );
};

// ---------------------------------------------------------------------------
// 按钮按下反馈（Press 微交互）
// ---------------------------------------------------------------------------

export const pressedStyle = (press: number): CSSProperties => {
  const p = Math.max(0, Math.min(1, press));
  return {
    transform: `scale(${1 - p * 0.06})`,
    filter: press > 0.05 ? 'brightness(1.18)' : 'none',
  };
};

// ---------------------------------------------------------------------------
// 应用内页面滑动：enterAt 进入、exitAt 退出，前/后页 slide + fade
// ---------------------------------------------------------------------------

export const pageSlide = (frame: number, enterAt: number, exitAt: number, dir = 1, edge = 18) => {
  const e = easeInOut(interpolate(frame, [enterAt, enterAt + edge], [0, 1], clamp));
  const x = easeInOut(interpolate(frame, [exitAt - edge, exitAt], [1, 0], clamp));
  const opacity = Math.min(e, x);
  // x 表示“退出前可见度”：稳定阶段为 1，退出末尾降到 0。
  // 因此第二段位移必须在 x=1 时为 0、x=0 时才向左滑出；
  // 否则页面会在整个稳定阶段永久左移 70px，导致内容被侧栏裁切。
  const shift = interpolate(e, [0, 1], [70, 0]) + interpolate(x, [0, 1], [-70, 0]);
  return {opacity, transform: `translateX(${dir * shift}px)`};
};
