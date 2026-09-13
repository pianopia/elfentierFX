import { useCallback, useRef, useState } from "react";
import "./SplitPane.css";

interface SplitPaneProps {
  left: React.ReactNode;
  right: React.ReactNode;
  initialRatio?: number;
}

export default function SplitPane({ left, right, initialRatio = 0.52 }: SplitPaneProps) {
  const [ratio, setRatio] = useState(initialRatio);
  const dragging = useRef(false);
  const containerRef = useRef<HTMLDivElement>(null);

  const onPointerDown = useCallback(() => {
    dragging.current = true;
  }, []);

  const onPointerUp = useCallback(() => {
    dragging.current = false;
  }, []);

  const onPointerMove = useCallback((e: React.PointerEvent) => {
    if (!dragging.current || !containerRef.current) return;
    const rect = containerRef.current.getBoundingClientRect();
    const next = (e.clientX - rect.left) / rect.width;
    setRatio(Math.min(0.75, Math.max(0.25, next)));
  }, []);

  return (
    <div
      className="split-pane"
      ref={containerRef}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onPointerLeave={onPointerUp}
    >
      <div className="split-pane-left" style={{ flexBasis: `${ratio * 100}%` }}>
        {left}
      </div>
      <div
        className="split-pane-divider"
        role="separator"
        aria-orientation="vertical"
        onPointerDown={onPointerDown}
      />
      <div className="split-pane-right">{right}</div>
    </div>
  );
}
