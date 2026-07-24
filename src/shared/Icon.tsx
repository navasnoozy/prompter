import type { ReactNode } from "react";

const ICON_PATHS = {
  sparkle: (
    <path d="M12 2l1.35 4.15L17.5 7.5l-4.15 1.35L12 13l-1.35-4.15L6.5 7.5l4.15-1.35L12 2Zm6 10 .9 2.6 2.6.9-2.6.9L18 19l-.9-2.6-2.6-.9 2.6-.9L18 12ZM5 13l1.05 2.95L9 17l-2.95 1.05L5 21l-1.05-2.95L1 17l2.95-1.05L5 13Z" />
  ),
  wand: (
    <path d="m4 20 12.8-12.8m-10.6 9.6 1 1M14.8 8.2l1 1M15 2l.7 2.3L18 5l-2.3.7L15 8l-.7-2.3L12 5l2.3-.7L15 2Zm5 7 .5 1.5L22 11l-1.5.5L20 13l-.5-1.5L18 11l1.5-.5L20 9ZM5 3l.7 2.3L8 6l-2.3.7L5 9l-.7-2.3L2 6l2.3-.7L5 3Z" />
  ),
  clipboard: (
    <path d="M9 5h6m-6 0a2 2 0 0 0 2 2h2a2 2 0 0 0 2-2m-6 0a2 2 0 0 1 2-2h2a2 2 0 0 1 2 2m0 0h2a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2h2" />
  ),
  plus: <path d="M12 5v14M5 12h14" />,
  settings: (
    <path d="M12 15.5a3.5 3.5 0 1 0 0-7 3.5 3.5 0 0 0 0 7Zm0-12v2m0 13v2m8.5-8.5h-2m-13 0h-2m14.5-6-1.4 1.4M7.4 16.6 6 18m12 0-1.4-1.4M7.4 7.4 6 6" />
  ),
  back: <path d="m10 18-6-6 6-6m-6 6h16" />,
  forward: <path d="m14 6 6 6-6 6m6-6H4" />,
  reload: <path d="M20 11a8 8 0 1 0-2.34 5.66M20 4v7h-7" />,
  stop: <path d="m7 7 10 10M17 7 7 17" />,
  chevron: <path d="m9 18 6-6-6-6" />,
  edit: (
    <path d="m4 20 4.2-1 10.6-10.6a2.1 2.1 0 0 0-3-3L5.2 16 4 20Zm10.5-13.5 3 3" />
  ),
  trash: <path d="M4 7h16M9 7V4h6v3m3 0-1 14H7L6 7m4 4v6m4-6v6" />,
  close: <path d="m6 6 12 12M18 6 6 18" />,
  check: <path d="m5 12 4 4L19 6" />,
  moon: <path d="M20.5 14.2A8.5 8.5 0 0 1 9.8 3.5 8.5 8.5 0 1 0 20.5 14.2Z" />,
  sun: (
    <path d="M12 16a4 4 0 1 0 0-8 4 4 0 0 0 0 8Zm0-14v2m0 16v2m10-10h-2M4 12H2m17.07-7.07-1.41 1.41M6.34 17.66l-1.41 1.41m14.14 0-1.41-1.41M6.34 6.34 4.93 4.93" />
  ),
} satisfies Record<string, ReactNode>;

export type IconName = keyof typeof ICON_PATHS;

type IconProps = {
  name: IconName;
  size?: number;
};

export function Icon({ name, size = 18 }: IconProps) {
  const isFilled = name === "sparkle";

  return (
    <svg
      aria-hidden="true"
      className="icon"
      fill="none"
      height={size}
      viewBox="0 0 24 24"
      width={size}
    >
      <g
        fill={isFilled ? "currentColor" : "none"}
        stroke={isFilled ? "none" : "currentColor"}
        strokeLinecap="round"
        strokeLinejoin="round"
        strokeWidth="1.8"
      >
        {ICON_PATHS[name]}
      </g>
    </svg>
  );
}
