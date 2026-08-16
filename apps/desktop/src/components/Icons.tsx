import type { ReactNode } from "react";

interface IconProps {
  label?: string;
}

function Svg({
  label,
  children,
}: IconProps & { children: ReactNode }) {
  return (
    <svg
      aria-hidden={label ? undefined : true}
      aria-label={label}
      viewBox="0 0 16 16"
      width="16"
      height="16"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.4"
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      {children}
    </svg>
  );
}

export function IconEdit(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M11.5 2.5 13.5 4.5 6 12H4v-2z" />
    </Svg>
  );
}

export function IconPlus(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M8 3v10M3 8h10" />
    </Svg>
  );
}

export function IconExpand(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M3 6V3h3M10 3h3v3M13 10v3h-3M6 13H3v-3" />
    </Svg>
  );
}

export function IconBolt(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M9 2 4 9h4l-1 5 5-7H8z" />
    </Svg>
  );
}

export function IconClose(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M4 4l8 8M12 4l-8 8" />
    </Svg>
  );
}

export function IconMore(props: IconProps) {
  return (
    <Svg {...props}>
      <circle cx="8" cy="3.5" r="1" fill="currentColor" stroke="none" />
      <circle cx="8" cy="8" r="1" fill="currentColor" stroke="none" />
      <circle cx="8" cy="12.5" r="1" fill="currentColor" stroke="none" />
    </Svg>
  );
}

export function IconRefresh(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M13 8A5 5 0 1 1 11 4.2" />
      <path d="M13 3v3h-3" />
    </Svg>
  );
}

export function IconTrash(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M3 5h10M6 5V3.5h4V5M6.5 7v5M9.5 7v5M4.5 5l.6 8h5.8l.6-8" />
    </Svg>
  );
}

export function IconExport(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M8 10V3M5.5 5.5 8 3l2.5 2.5M4 12h8" />
    </Svg>
  );
}

function SizedSvg({
  label,
  size = 16,
  children,
}: IconProps & { children: ReactNode; size?: number }) {
  return (
    <svg
      aria-hidden={label ? undefined : true}
      aria-label={label}
      viewBox="0 0 16 16"
      width={size}
      height={size}
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      {children}
    </svg>
  );
}

export function IconSearch(props: IconProps) {
  return (
    <SizedSvg {...props}>
      <circle cx="7" cy="7" r="3.5" />
      <path d="M10 10.5 13 13.5" />
    </SizedSvg>
  );
}

export function IconSun(props: IconProps) {
  return (
    <SizedSvg {...props}>
      <circle cx="8" cy="8" r="2.4" />
      <path d="M8 2.2v1.4M8 12.4v1.4M2.2 8h1.4M12.4 8h1.4M3.8 3.8l1 1M11.2 11.2l1 1M12.2 3.8l-1 1M4.8 11.2l-1 1" />
    </SizedSvg>
  );
}

export function IconMoon(props: IconProps) {
  return (
    <SizedSvg {...props}>
      <path d="M10.2 2.8A5.2 5.2 0 1 0 13.2 11 4.2 4.2 0 0 1 10.2 2.8z" />
    </SizedSvg>
  );
}

export function IconGauge(props: IconProps) {
  return (
    <SizedSvg {...props}>
      <path d="M3.2 11.2a5.2 5.2 0 1 1 9.6 0" />
      <path d="M8 10.2 10.4 6.8" />
      <circle cx="8" cy="10.4" r="0.8" fill="currentColor" stroke="none" />
    </SizedSvg>
  );
}

export function IconServer(props: IconProps) {
  return (
    <SizedSvg {...props}>
      <rect x="2.5" y="2.5" width="11" height="4" rx="1" />
      <rect x="2.5" y="9.5" width="11" height="4" rx="1" />
      <path d="M4.5 4.5h.01M4.5 11.5h.01" />
    </SizedSvg>
  );
}

export function IconNodes(props: IconProps) {
  return (
    <SizedSvg {...props}>
      <circle cx="4.5" cy="4.5" r="1.6" />
      <circle cx="11.5" cy="4.5" r="1.6" />
      <circle cx="8" cy="11.5" r="1.6" />
      <path d="M5.8 5.3 7 10.2M10.2 5.3 9 10.2" />
    </SizedSvg>
  );
}

export function IconLink(props: IconProps) {
  return (
    <SizedSvg {...props}>
      <path d="M6.2 9.8 4.8 11.2a2 2 0 1 1-2.8-2.8l2-2a2 2 0 0 1 2.8 0" />
      <path d="M9.8 6.2 11.2 4.8a2 2 0 1 1 2.8 2.8l-2 2a2 2 0 0 1-2.8 0" />
    </SizedSvg>
  );
}

export function IconChart(props: IconProps) {
  return (
    <SizedSvg {...props}>
      <path d="M2.5 13h11" />
      <path d="M4 11V8M8 11V5M12 11V7" />
    </SizedSvg>
  );
}

export function IconTerminal(props: IconProps) {
  return (
    <SizedSvg {...props}>
      <rect x="2.5" y="3" width="11" height="10" rx="1.4" />
      <path d="M5 6.5 7 8.2 5 10M8.5 10.2H11" />
    </SizedSvg>
  );
}

export function IconRoute(props: IconProps) {
  return (
    <SizedSvg {...props}>
      <circle cx="4" cy="4" r="1.4" />
      <circle cx="12" cy="12" r="1.4" />
      <path d="M5.3 5.3 7 7.2c.6.6.6 1.6 0 2.2L5.5 11M10.5 5 8.8 6.8c-.6.6-.6 1.6 0 2.2L12 12" />
    </SizedSvg>
  );
}

export function IconGlobe(props: IconProps) {
  return (
    <SizedSvg {...props}>
      <circle cx="8" cy="8" r="5.2" />
      <path d="M2.8 8h10.4M8 2.8c1.6 1.6 2.4 3.5 2.4 5.2S9.6 11.6 8 13.2C6.4 11.6 5.6 9.7 5.6 8S6.4 4.4 8 2.8z" />
    </SizedSvg>
  );
}

export function IconGear(props: IconProps) {
  return (
    <SizedSvg {...props}>
      <circle cx="8" cy="8" r="2" />
      <path d="M8 2.6v1.4M8 12v1.4M2.6 8h1.4M12 8h1.4M4.1 4.1l1 1M10.9 10.9l1 1M11.9 4.1l-1 1M5.1 10.9l-1 1" />
    </SizedSvg>
  );
}

export function IconShield(props: IconProps) {
  return (
    <SizedSvg {...props} size={14}>
      <path d="M8 2.4 3.4 4.2v3.6c0 3 2 4.8 4.6 5.8 2.6-1 4.6-2.8 4.6-5.8V4.2z" />
      <path d="M5.8 8.1 7.3 9.6 10.4 6.4" />
    </SizedSvg>
  );
}

export function IconInfo(props: IconProps) {
  return (
    <SizedSvg {...props}>
      <circle cx="8" cy="8" r="5.2" />
      <path d="M8 7.2V11M8 5.2h.01" />
    </SizedSvg>
  );
}

export function IconWarn(props: IconProps) {
  return (
    <SizedSvg {...props}>
      <path d="M8 2.8 2.6 12.4h10.8z" />
      <path d="M8 6.4V9M8 10.6h.01" />
    </SizedSvg>
  );
}

export function IconList(props: IconProps) {
  return (
    <SizedSvg {...props}>
      <path d="M3 4.5h10M3 8h10M3 11.5h10" />
    </SizedSvg>
  );
}

export function IconGrid(props: IconProps) {
  return (
    <SizedSvg {...props}>
      <rect x="3" y="3" width="4" height="4" rx="0.6" />
      <rect x="9" y="3" width="4" height="4" rx="0.6" />
      <rect x="3" y="9" width="4" height="4" rx="0.6" />
      <rect x="9" y="9" width="4" height="4" rx="0.6" />
    </SizedSvg>
  );
}
