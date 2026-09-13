import { useCallback, useState } from "react";
import "./PromptBar.css";

interface PromptBarProps {
  onSubmit: (prompt: string) => void;
  summary?: string;
  explainText?: string;
  disabled?: boolean;
}

const HINTS = [
  "商店街",
  "階数を5に",
  "seed変え",
  "もっと窓",
  "グリッド配置",
];

export default function PromptBar({
  onSubmit,
  summary,
  explainText,
  disabled,
}: PromptBarProps) {
  const [value, setValue] = useState("");

  const handleSubmit = useCallback(
    (e: React.FormEvent) => {
      e.preventDefault();
      const trimmed = value.trim();
      if (!trimmed || disabled) return;
      onSubmit(trimmed);
      setValue("");
    },
    [value, disabled, onSubmit],
  );

  const applyHint = useCallback(
    (hint: string) => {
      if (disabled) return;
      onSubmit(hint);
    },
    [disabled, onSubmit],
  );

  return (
    <div className="prompt-bar">
      <form className="prompt-form" onSubmit={handleSubmit}>
        <input
          type="text"
          className="prompt-input"
          placeholder="Describe changes — 商店街, 階数を5に, more windows…"
          value={value}
          onChange={(e) => setValue(e.target.value)}
          disabled={disabled}
          aria-label="Prompt"
        />
        <button type="submit" className="prompt-submit" disabled={disabled || !value.trim()}>
          Apply
        </button>
      </form>
      <div className="prompt-hints">
        {HINTS.map((hint) => (
          <button
            key={hint}
            type="button"
            className="prompt-hint"
            onClick={() => applyHint(hint)}
            disabled={disabled}
          >
            {hint}
          </button>
        ))}
      </div>
      {summary && <p className="prompt-summary">{summary}</p>}
      {explainText && (
        <details className="prompt-explain">
          <summary>Explain graph</summary>
          <pre>{explainText}</pre>
        </details>
      )}
    </div>
  );
}
