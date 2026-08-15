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
