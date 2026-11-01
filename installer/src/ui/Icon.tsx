const PATHS = {
  home: "M3 10.5 12 3l9 7.5V20a1 1 0 0 1-1 1h-5v-6H9v6H4a1 1 0 0 1-1-1z",
  screen: "M3 5h18v11H3z M8 20h8 M12 16v4",
  timer: "M12 21a8 8 0 1 0 0-16 8 8 0 0 0 0 16z M12 9v4l2.5 1.5 M9.5 2h5",
  tasks: "M9 11l3 3 8-8 M20 12v7a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2h9",
  calendar: "M5 5h14a1 1 0 0 1 1 1v13a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V6a1 1 0 0 1 1-1z M16 3v4 M8 3v4 M4 10h16",
  target: "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18z M12 16.5a4.5 4.5 0 1 0 0-9 4.5 4.5 0 0 0 0 9z M12 12.6a.6.6 0 1 0 0-1.2.6.6 0 0 0 0 1.2z",
  sliders: "M4 6h9 M17 6h3 M4 12h3 M11 12h9 M4 18h11 M19 18h1 M15 4v4 M7 10v4 M17 16v4",
  play: "M7 4.8v14.4a.5.5 0 0 0 .77.42l11.2-7.2a.5.5 0 0 0 0-.84L7.77 4.38A.5.5 0 0 0 7 4.8z",
  pause: "M8 5v14 M16 5v14",
  stop: "M6.5 6.5h11v11h-11z",
  skip: "M5 5.5v13l9-6.5z M18.5 5v14",
  plus: "M12 5v14 M5 12h14",
  minus: "M5 12h14",
  trash: "M4 7h16 M10 11v6 M14 11v6 M6 7l1 13h10l1-13 M9 7V4h6v3",
  edit: "M4 20h4L19 9l-4-4L4 16z M13.5 6.5l4 4",
  chevronLeft: "M15 5l-7 7 7 7",
  chevronRight: "M9 5l7 7-7 7",
  chevronDown: "M5 9l7 7 7-7",
  x: "M6 6l12 12 M18 6L6 18",
  square: "M6 6h12v12H6z",
  restore: "M9 9h9v9H9z M6 15V6h9",
  repeat: "M17 2l4 4-4 4 M3 11V9a3 3 0 0 1 3-3h15 M7 22l-4-4 4-4 M21 13v2a3 3 0 0 1-3 3H3",
  arrowRight: "M5 12h14 M13 6l6 6-6 6",
  check: "M5 12.5l4.5 4.5L19 7",
  power: "M12 3v9 M6.5 6.5a8 8 0 1 0 11 0",
  layout: "M4 5h16v14H4z M4 10h16 M10 10v9",
  flag: "M5 21V4 M5 4h11l-2 4 2 4H5",
  clock: "M12 21a9 9 0 1 0 0-18 9 9 9 0 0 0 0 18z M12 7v5l3 2",
  textCursor: "M4 4h16v16H4z M9 8.5h6 M12 8.5v7",
  link: "M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1 1 M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1-1",
  alert: "M12 4l9 16H3z M12 10v4 M12 17h.01",
  bolt: "M13 3L5 14h6l-1 7 8-11h-6z",
  eye: "M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12z M12 14.5a2.5 2.5 0 1 0 0-5 2.5 2.5 0 0 0 0 5z",
  history: "M3 12a9 9 0 1 0 3-6.7L3 8 M3 3v5h5 M12 7v5l3 2",
  dots: "M5 12h14"
} as const;

export type IconName = keyof typeof PATHS;

type IconProps = {
  name: IconName;
  size?: number;
  strokeWidth?: number;
  className?: string;
};

export function Icon({ name, size = 18, strokeWidth = 1.7, className }: IconProps) {
  if (name === "dots") {
    return (
      <svg className={className} width={size} height={size} viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
        <circle cx="5" cy="12" r="1.9" />
        <circle cx="12" cy="12" r="1.9" />
        <circle cx="19" cy="12" r="1.9" />
      </svg>
    );
  }

  return (
    <svg
      className={className}
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={strokeWidth}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <path d={PATHS[name]} />
    </svg>
  );
}