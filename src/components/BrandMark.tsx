import { useId } from "react";

export interface BrandMarkProps {
  /** Rendered pixel size (width & height). Defaults to 30 (the Figma master size). */
  size?: number;
  /**
   * Accessible label. Pass an empty string to render the mark as decorative
   * (`aria-hidden`, no accessible name) when an adjacent text label already
   * names the brand.
   */
  title?: string;
  /** Extra class(es) for layout/positioning. */
  className?: string;
}

/**
 * The Chaos Scheduler brand mark — the indigo "orbital-8" glyph. A faithful
 * inline-SVG reproduction of the brand SOT (`public/favicon.svg`, also the
 * source for the macOS app icon + menu-bar tray glyph), matching the Figma
 * `BrandMark` master (node 186:1241). Being the product logo, its violet→blue
 * brand gradients are a FIXED brand identity (a logo does not theme-switch) and
 * there are no semantic color tokens for them — so, unlike UI chrome, the mark
 * intentionally carries its brand-gradient stops rather than `var(--…)` tokens.
 * Gradient/filter ids are made unique per instance so multiple marks can coexist
 * on a page without id collisions. Purely presentational — not yet wired into
 * any screen.
 */
export default function BrandMark({
  size = 30,
  title = "Chaos Scheduler",
  className,
}: BrandMarkProps) {
  const uid = useId().replace(/:/g, "");
  const arcViolet = `${uid}-arc-violet`;
  const arcBlue = `${uid}-arc-blue`;
  const dotViolet = `${uid}-dot-violet`;
  const dotBlue = `${uid}-dot-blue`;
  const glow = `${uid}-glow`;
  const mark = `${uid}-mark`;
  const decorative = title === "";

  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      width={size}
      height={size}
      viewBox="0 0 512 512"
      fill="none"
      className={className}
      role="img"
      aria-hidden={decorative || undefined}
      aria-label={decorative ? undefined : title}
    >
      <defs>
        <linearGradient
          id={arcViolet}
          x1="196"
          y1="112"
          x2="404"
          y2="326"
          gradientUnits="userSpaceOnUse"
        >
          <stop offset="0" stopColor="#c24ffb" />
          <stop offset="1" stopColor="#7f36f6" />
        </linearGradient>
        <linearGradient
          id={arcBlue}
          x1="120"
          y1="248"
          x2="330"
          y2="430"
          gradientUnits="userSpaceOnUse"
        >
          <stop offset="0" stopColor="#5aa0ff" />
          <stop offset="1" stopColor="#2f43f7" />
        </linearGradient>
        <radialGradient id={dotViolet} cx="0.42" cy="0.4" r="0.72">
          <stop offset="0" stopColor="#c471ff" />
          <stop offset="1" stopColor="#9a34f2" />
        </radialGradient>
        <radialGradient id={dotBlue} cx="0.42" cy="0.4" r="0.72">
          <stop offset="0" stopColor="#49b0ff" />
          <stop offset="1" stopColor="#1477f2" />
        </radialGradient>
        <filter id={glow} x="-30%" y="-30%" width="160%" height="160%">
          <feGaussianBlur stdDeviation="7" />
        </filter>
        <g id={mark}>
          <path
            d="M191.54 191.51 A112 112 0 1 1 320.49 320.46"
            stroke={`url(#${arcViolet})`}
            strokeWidth="40"
            strokeLinecap="round"
          />
          <path
            d="M320.46 320.49 A112 112 0 1 1 191.51 191.54"
            stroke={`url(#${arcBlue})`}
            strokeWidth="40"
            strokeLinecap="round"
          />
          <circle cx="302" cy="210" r="34" fill={`url(#${dotViolet})`} />
          <circle cx="210" cy="302" r="34" fill={`url(#${dotBlue})`} />
        </g>
      </defs>
      <use href={`#${mark}`} filter={`url(#${glow})`} opacity="0.5" />
      <use href={`#${mark}`} />
    </svg>
  );
}
