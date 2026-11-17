import { useEffect, useRef } from "react";
import { seekBarColour, type SeekBarColour, type SoundMap } from "../soundMap";
import type { TrackWaveform } from "../waveform";

export function WaveformTimeline({
  waveform,
  soundMap = null,
  colour = "plain",
  position,
  duration,
  disabled,
  onChange,
  onCommit,
}: {
  waveform: TrackWaveform | null;
  /** Colours the waveform and marks sections, when the track has one. */
  soundMap?: SoundMap | null;
  colour?: SeekBarColour;
  position: number;
  duration: number;
  disabled: boolean;
  onChange: (position: number) => void;
  onCommit: (position: number) => void;
}) {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    const container = canvas?.parentElement;
    if (!canvas || !container) return;
    const canvasElement = canvas;
    const containerElement = container;

    function draw() {
      const width = Math.max(containerElement.clientWidth, 1);
      const height = Math.max(containerElement.clientHeight, 1);
      const dpr = Math.min(window.devicePixelRatio || 1, 2);
      canvasElement.width = Math.round(width * dpr);
      canvasElement.height = Math.round(height * dpr);
      canvasElement.style.width = `${width}px`;
      canvasElement.style.height = `${height}px`;
      const context = canvasElement.getContext("2d");
      if (!context) return;
      context.setTransform(dpr, 0, 0, dpr, 0, 0);
      context.clearRect(0, 0, width, height);

      if (!waveform) {
        context.fillStyle = "rgba(154, 164, 181, 0.2)";
        context.fillRect(0, height / 2, width, 1);
        return;
      }
      const drawingContext = context;
      const peaks = waveform.peaks;

      const gradient = drawingContext.createLinearGradient(0, 0, width, 0);
      const lane = soundMap && colour !== "plain" ? soundMap[colour] : null;
      if (lane?.length) {
        const last = Math.max(lane.length - 1, 1);
        lane.forEach((value, index) => gradient.addColorStop(index / last, seekBarColour(colour as Exclude<SeekBarColour, "plain">, value)));
      } else {
        gradient.addColorStop(0, "#d946ef");
        gradient.addColorStop(0.38, "#a855f7");
        gradient.addColorStop(0.68, "#2787ff");
        gradient.addColorStop(1, "#22d3ee");
      }
      const progress = duration > 0 ? Math.min(Math.max(position / duration, 0), 1) : 0;

      function traceWaveform() {
        const middle = height / 2;
        const amplitude = Math.max((height - 4) / 2, 1);
        const denominator = Math.max(peaks.length - 1, 1);
        drawingContext.beginPath();
        drawingContext.moveTo(0, middle);
        peaks.forEach((peak, index) => {
          drawingContext.lineTo(index / denominator * width, middle - peak * amplitude);
        });
        for (let index = peaks.length - 1; index >= 0; index -= 1) {
          drawingContext.lineTo(index / denominator * width, middle + peaks[index] * amplitude);
        }
        drawingContext.closePath();
      }

      context.fillStyle = gradient;
      context.globalAlpha = 0.38;
      traceWaveform();
      context.fill();

      if (progress > 0) {
        context.save();
        context.beginPath();
        context.rect(0, 0, progress * width, height);
        context.clip();
        context.globalAlpha = 0.98;
        traceWaveform();
        context.fill();
        context.restore();
      }

      context.globalAlpha = 1;
      context.fillStyle = "rgba(154, 164, 181, 0.15)";
      context.fillRect(0, Math.floor(height / 2), width, 1);
      if (soundMap && duration > 0) {
        // Faint ticks where the sound changes, and a marker where the main part starts.
        context.fillStyle = "rgba(233, 231, 255, 0.3)";
        for (const second of soundMap.sections) {
          const x = Math.round(Math.min(second / duration, 1) * width);
          context.fillRect(x, 0, 1, 4);
          context.fillRect(x, height - 4, 1, 4);
        }
        if (soundMap.mainStart !== null) {
          const x = Math.min(soundMap.mainStart / duration, 1) * width;
          context.fillStyle = "rgba(143, 221, 232, 0.95)";
          context.beginPath();
          context.moveTo(x - 4, 0);
          context.lineTo(x + 4, 0);
          context.lineTo(x, 5);
          context.closePath();
          context.fill();
        }
      }
      if (progress > 0) {
        const x = Math.min(width - 1, progress * width);
        context.fillStyle = "rgba(233, 231, 255, 0.82)";
        context.fillRect(x, 1, 1, height - 2);
      }
    }

    draw();
    const observer = new ResizeObserver(draw);
    observer.observe(containerElement);
    return () => observer.disconnect();
  }, [colour, duration, position, soundMap, waveform]);

  return (
    <div className="waveform-timeline" data-loading={!waveform || undefined} data-colour={soundMap ? colour : undefined}>
      <canvas ref={canvasRef} aria-hidden="true" />
      <input
        type="range"
        aria-label="Playback position"
        min={0}
        max={Math.max(duration, 1)}
        step={1}
        value={position}
        disabled={disabled}
        onChange={(event) => onChange(Number(event.target.value))}
        onPointerUp={(event) => onCommit(Number(event.currentTarget.value))}
        onKeyUp={(event) => onCommit(Number(event.currentTarget.value))}
      />
    </div>
  );
}
