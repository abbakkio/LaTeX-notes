import React from "react";
import type { FileLoadProgress } from "../types";

interface CircularProgressSpinnerProps {
  progress: FileLoadProgress | null;
  fallbackLabel?: string;
}

/** Apple-style pure-black circular progress spinner showing remaining percentage. */
export const CircularProgressSpinner: React.FC<CircularProgressSpinnerProps> = ({
  progress,
  fallbackLabel = "Loading note…",
}) => {
  const size = 84;
  const strokeWidth = 3.5;
  const radius = (size - strokeWidth * 2) / 2;
  const circumference = 2 * Math.PI * radius;

  const percentComplete = progress
    ? Math.max(0, Math.min(100, progress.percent_complete))
    : 0;
  const percentLeft = progress
    ? Math.max(0, Math.min(100, Math.round(progress.percent_left)))
    : 100;

  const dashOffset = circumference * (1 - percentComplete / 100);

  let detailLine = fallbackLabel;
  if (progress) {
    if (progress.total_pages > 0) {
      const activePage = Math.min(progress.total_pages, progress.current_page + 1);
      detailLine =
        progress.current_page >= progress.total_pages
          ? `Finalizing ${progress.total_pages} ${
              progress.total_pages === 1 ? "page" : "pages"
            }`
          : `Page ${activePage} of ${progress.total_pages}`;
    } else {
      detailLine = "Reading PDF structure…";
    }
  }

  return (
    <div className="upload-spinner-card" role="status" aria-live="polite">
      <div className="ring-spinner-wrap" style={{ width: size, height: size }}>
        <svg
          className="ring-spinner-svg"
          width={size}
          height={size}
          viewBox={`0 0 ${size} ${size}`}
        >
          {/* Hairline base track */}
          <circle
            className="ring-track"
            cx={size / 2}
            cy={size / 2}
            r={radius}
            strokeWidth={strokeWidth}
          />
          {/* Subtle rotating Apple spinner sweep so 0% still feels alive */}
          <circle
            className="ring-orbit"
            cx={size / 2}
            cy={size / 2}
            r={radius}
            strokeWidth={strokeWidth}
            strokeDasharray={`${circumference * 0.22} ${circumference * 0.78}`}
          />
          {/* Determinate completed arc */}
          {progress && (
            <circle
              className="ring-progress"
              cx={size / 2}
              cy={size / 2}
              r={radius}
              strokeWidth={strokeWidth}
              strokeDasharray={circumference}
              strokeDashoffset={dashOffset}
            />
          )}
        </svg>

        <div className="ring-center-readout">
          {progress ? (
            <>
              <span className="ring-percent">{percentLeft}%</span>
              <span className="ring-sub">left</span>
            </>
          ) : (
            <span className="ring-sub">…</span>
          )}
        </div>
      </div>

      <div className="upload-spinner-meta">
        {progress?.file_name && (
          <div className="upload-spinner-file">{progress.file_name}</div>
        )}
        <div className="upload-spinner-detail">{detailLine}</div>
      </div>
    </div>
  );
};
