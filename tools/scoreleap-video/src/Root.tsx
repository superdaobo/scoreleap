import {Composition} from 'remotion';
import {fallbackProps} from './fallback';
import {AlgorithmRangePreview, MelodyPreview} from './scenes';
import {
  AppDemoFlowScene,
  EndingScene,
  GameplayRevealScene,
  HookScene,
  IdentityVScene,
} from './demo';
import {
  APP_FLOW_SECONDS,
  ENDING_SECONDS,
  FPS,
  HOOK_SECONDS,
  IDENTITY_SECONDS,
  OVERLAP,
  ScoreLeapTrailer,
  seconds,
  trailerLengthFrames,
} from './Trailer';
import type {TrailerProps} from './types';

const clampGameplay = (value: number) => Math.max(12, Math.min(45, value));

export const ScoreLeapVideoRoot = () => {
  return (
    <>
      <Composition
        id="ScoreLeapTrailer90"
        component={ScoreLeapTrailer}
        width={1920}
        height={1080}
        fps={FPS}
        durationInFrames={trailerLengthFrames(24, FPS)}
        defaultProps={fallbackProps}
        calculateMetadata={({props}) => {
          const typed = props as TrailerProps;
          const gameplay = clampGameplay(typed.gameplayDurationSeconds);
          return {
            durationInFrames: trailerLengthFrames(gameplay, FPS),
            props: {...typed, gameplayDurationSeconds: gameplay},
            defaultCodec: 'h264',
            defaultOutName: 'scoreleap-trailer.mp4',
          };
        }}
      />
      <Composition
        id="ScoreLeapAppFlowDemo"
        component={AppDemoFlowScene}
        width={1920}
        height={1080}
        fps={FPS}
        durationInFrames={seconds(APP_FLOW_SECONDS, FPS)}
        defaultProps={{props: fallbackProps}}
      />
      <Composition
        id="ScoreLeapHookDemo"
        component={HookScene}
        width={1920}
        height={1080}
        fps={FPS}
        durationInFrames={seconds(HOOK_SECONDS, FPS) + OVERLAP}
      />
      <Composition
        id="ScoreLeapIdentityVDemo"
        component={IdentityVScene}
        width={1920}
        height={1080}
        fps={FPS}
        durationInFrames={seconds(IDENTITY_SECONDS, FPS) + OVERLAP}
        defaultProps={{props: fallbackProps}}
      />
      <Composition
        id="ScoreLeapGameplaySlot"
        component={GameplayRevealScene}
        width={1920}
        height={1080}
        fps={FPS}
        durationInFrames={seconds(24, FPS)}
        defaultProps={{props: fallbackProps}}
      />
      <Composition
        id="ScoreLeapEndingDemo"
        component={EndingScene}
        width={1920}
        height={1080}
        fps={FPS}
        durationInFrames={seconds(ENDING_SECONDS, FPS) + OVERLAP}
      />
      <Composition
        id="ScoreLeapRangeDemo"
        component={AlgorithmRangePreview}
        width={1920}
        height={1080}
        fps={FPS}
        durationInFrames={6 * FPS}
      />
      <Composition
        id="ScoreLeapMelodyDemo"
        component={MelodyPreview}
        width={1920}
        height={1080}
        fps={FPS}
        durationInFrames={7 * FPS}
      />
    </>
  );
};
