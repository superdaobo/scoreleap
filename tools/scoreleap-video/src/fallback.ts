import type {TrailerProps, VideoNote} from './types';

const motif = [60, 64, 67, 72, 76, 79, 84, 88, 91, 96, 91, 88, 84, 79, 76, 72];

export const fallbackNotes: VideoNote[] = Array.from({length: 72}, (_, index) => {
  const pitch = motif[index % motif.length] + (index % 19 === 0 ? 7 : 0);
  const start = index * 0.28;
  return {
    start,
    end: start + (index % 4 === 0 ? 0.72 : 0.46),
    pitch: Math.min(108, pitch),
    velocity: 68 + ((index * 13) % 42),
  };
});

export const fallbackProps: TrailerProps = {
  audioSrc: null,
  waveformSrc: null,
  gameplaySrc: null,
  gameplayDurationSeconds: 24,
  notes: fallbackNotes,
  sampleLabel: 'DEMO PIANO AUDIO',
  dataMode: 'synthetic',
};
