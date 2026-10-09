import {AbsoluteFill, Audio, Sequence, staticFile, useVideoConfig} from 'remotion';
import {AppDemoFlowScene, EndingScene, GameplayRevealScene, HookScene, IdentityVScene} from './demo';
import type {TrailerProps} from './types';
import {Scene} from './transitions';

export const FPS = 60;
export const HOOK_SECONDS = 3;
export const APP_FLOW_SECONDS = 40;
export const IDENTITY_SECONDS = 6;
export const ENDING_SECONDS = 5;
export const OVERLAP = 22;

export const seconds = (value: number, fps: number) => Math.round(value * fps);

export const trailerStartFrames = (fps: number) => {
  const hook = seconds(HOOK_SECONDS, fps);
  const appStart = hook - OVERLAP;
  const appFrames = seconds(APP_FLOW_SECONDS, fps);
  const identityStart = appStart + appFrames - OVERLAP;
  const identityFrames = seconds(IDENTITY_SECONDS, fps);
  const gameplayStart = identityStart + identityFrames - OVERLAP;
  const endingFrames = seconds(ENDING_SECONDS, fps);
  return {hook, appStart, appFrames, identityStart, identityFrames, gameplayStart, endingFrames};
};

export const trailerLengthFrames = (gameplaySeconds: number, fps: number) => {
  const {gameplayStart, endingFrames} = trailerStartFrames(fps);
  const gameplayFrames = seconds(gameplaySeconds, fps);
  const endingStart = gameplayStart + gameplayFrames - OVERLAP;
  return endingStart + endingFrames;
};

export const ScoreLeapTrailer = (props: TrailerProps) => {
  const {fps, durationInFrames} = useVideoConfig();
  const gameplaySeconds = Math.max(12, Math.min(45, props.gameplayDurationSeconds));
  const {hook, appStart, appFrames, identityStart, identityFrames, gameplayStart, endingFrames} =
    trailerStartFrames(fps);
  const gameplayFrames = seconds(gameplaySeconds, fps);
  const endingStart = gameplayStart + gameplayFrames - OVERLAP;

  // 没有实机录像时，让开发样本继续作为整片音床；接入真实 gameplay 后，
  // 在 gameplay 起点做 24 帧音频交叉淡化，让同一首《花海》从源音频自然切到游戏钢琴实录。
  const audioCrossfadeFrames = 24;
  const audioBedFrames = props.gameplaySrc ? gameplayStart + audioCrossfadeFrames : durationInFrames;

  return (
    <AbsoluteFill style={{background: '#030303'}}>
      {props.audioSrc ? (
        <Sequence from={0} durationInFrames={audioBedFrames}>
          <Audio
            src={staticFile(props.audioSrc)}
            volume={(audioFrame) => {
              if (!props.gameplaySrc || audioFrame < gameplayStart) return 0.6;
              const p = Math.max(0, Math.min(1, (audioBedFrames - audioFrame) / audioCrossfadeFrames));
              return 0.6 * p;
            }}
          />
        </Sequence>
      ) : null}

      <Sequence from={0} durationInFrames={hook + OVERLAP}>
        <Scene durationInFrames={hook + OVERLAP} motion="crossfade" enter={16} exit={20}>
          <HookScene />
        </Scene>
      </Sequence>

      <Sequence from={appStart} durationInFrames={appFrames + OVERLAP}>
        <Scene durationInFrames={appFrames + OVERLAP} motion="zoom-in" enter={24} exit={20}>
          <AppDemoFlowScene props={props} />
        </Scene>
      </Sequence>

      <Sequence from={identityStart} durationInFrames={identityFrames + OVERLAP}>
        <Scene durationInFrames={identityFrames + OVERLAP} motion="rise" enter={20} exit={24}>
          <IdentityVScene props={props} />
        </Scene>
      </Sequence>

      <Sequence from={gameplayStart} durationInFrames={gameplayFrames}>
        <GameplayRevealScene props={props} />
      </Sequence>

      <Sequence from={endingStart} durationInFrames={endingFrames + OVERLAP}>
        <Scene durationInFrames={endingFrames + OVERLAP} motion="crossfade" enter={24} exit={22}>
          <EndingScene />
        </Scene>
      </Sequence>
    </AbsoluteFill>
  );
};
