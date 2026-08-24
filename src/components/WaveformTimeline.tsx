import { useEffect, useRef } from "react";
import type { TrackWaveform } from "../waveform";

export function WaveformTimeline({
  waveform,
  position,
  duration,
  disabled,
  onChange,
  onCommit,
}: {
  waveform: TrackWaveform | null;
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
      gradient.addColorStop(0, "#d946ef");
      gradient.addColorStop(0.38, "#a855f7");
      gradient.addColorStop(0.68, "#2787ff");
      gradient.addColorStop(1, "#22d3ee");
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
  }, [duration, position, waveform]);

  return (
    <div className="waveform-timeline" data-loading={!waveform || undefined}>
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
