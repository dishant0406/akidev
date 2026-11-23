import type { CSSProperties, ReactNode } from "react";

/**
 * An iPhone around a screen. `width` is the width of the screen (any CSS length) and everything else follows it:
 * the bezel, the corners and the height. The screen's shape is the film's 616 x 1440 screen.
 */
export function Phone({ width, children, className }: { width: string; children: ReactNode; className?: string }) {
  return (
    <div
      aria-hidden
      className={className}
      style={
        {
          "--w": width,
          width: "calc(var(--w) * 1.072)",
          padding: "calc(var(--w) * 0.036)",
          borderRadius: "calc(var(--w) * 0.176)",
          background: "#050506",
          // A close shadow for the edge and a long one for the lift: no glow.
          boxShadow: "0 1px 1px rgb(0 0 0 / 0.06), 0 8px 16px -4px rgb(0 0 0 / 0.12), 0 32px 64px -16px rgb(0 0 0 / 0.28)",
        } as CSSProperties
      }
    >
      <div className="relative aspect-[616/1440] overflow-hidden bg-[#0D0D0E]" style={{ borderRadius: "calc(var(--w) * 0.14)" }}>
        {children}
      </div>
    </div>
  );
}
