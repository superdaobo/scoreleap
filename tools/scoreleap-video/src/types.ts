export type VideoNote = {
  start: number;
  end: number;
  pitch: number;
  velocity: number;
};

export type TrailerProps = {
  audioSrc: string | null;
  waveformSrc: string | null;
  gameplaySrc: string | null;
  gameplayDurationSeconds: number;
  notes: VideoNote[];
  sampleLabel: string;
  dataMode: 'synthetic' | 'local-real';
};
