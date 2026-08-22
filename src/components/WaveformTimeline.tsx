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
  onCommit: () => void;
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

      const gradient = context.createLinearGradient(0, 0, width, 0);
      gradient.addColorStop(0, "#d946ef");
      gradient.addColorStop(0.38, "#a855f7");
      gradient.addColorStop(0.68, "#2787ff");
      gradient.addColorStop(1, "#22d3ee");
      const count = waveform.peaks.length;
      const stride = width / count;
      const barWidth = Math.max(1, Math.min(2.2, stride * 0.62));
      const progress = duration > 0 ? Math.min(Math.max(position / duration, 0), 1) : 0;
      context.fillStyle = gradient;
      waveform.peaks.forEach((peak, index) => {
        const x = index * stride + (stride - barWidth) / 2;
        const barHeight = Math.max(1.5, peak * (height - 6));
        context.globalAlpha = x / width <= progress ? 0.98 : 0.68;
        context.fillRect(x, (height - barHeight) / 2, barWidth, barHeight);
      });
      context.globalAlpha = 1;
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
        onPointerUp={onCommit}
        onKeyUp={onCommit}
      />
    </div>
  );
}
