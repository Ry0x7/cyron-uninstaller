import type { ReactNode, SVGProps } from "react";

export interface IconProps extends Omit<SVGProps<SVGSVGElement>, "children"> {
  size?: number;
  strokeWidth?: number;
}

function Svg({ size = 18, strokeWidth = 1.7, children, ...rest }: IconProps & { children: ReactNode }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={strokeWidth}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
      {...rest}
    >
      {children}
    </svg>
  );
}

export const IconHome = (props: IconProps) => (
  <Svg {...props}>
    <path d="M3.5 10.2 12 3.5l8.5 6.7" />
    <path d="M5.5 9.2v10.3h13V9.2" />
    <path d="M9.8 19.5v-5.8h4.4v5.8" />
  </Svg>
);

export const IconOverview = IconHome;

export const IconStorage = (props: IconProps) => (
  <Svg {...props}>
    <rect x="3.2" y="4.5" width="17.6" height="15" rx="2.2" />
    <path d="M3.2 10.2h17.6" />
    <path d="M6.7 14.8h.01M10.1 14.8h.01" />
    <path d="M15.3 14.8h3.1" />
  </Svg>
);

export const IconHistory = (props: IconProps) => (
  <Svg {...props}>
    <path d="M3.6 12a8.4 8.4 0 1 0 2.5-5.9" />
    <path d="M3.4 5.3v4.3h4.3" />
    <path d="M12 7.8V12l3 1.8" />
  </Svg>
);

export const IconWindow = (props: IconProps) => (
  <Svg {...props}>
    <rect x="3.2" y="4.5" width="17.6" height="15" rx="2" />
    <path d="M3.2 8.8h17.6" />
    <path d="M6.4 6.7h.01M9 6.7h.01" />
  </Svg>
);

export const IconArrowLeft = (props: IconProps) => (
  <Svg {...props}>
    <path d="M19 12H5M10.5 6.5 5 12l5.5 5.5" />
  </Svg>
);

export const IconMore = (props: IconProps) => (
  <Svg {...props}>
    <circle cx="5" cy="12" r="1.1" fill="currentColor" stroke="none" />
    <circle cx="12" cy="12" r="1.1" fill="currentColor" stroke="none" />
    <circle cx="19" cy="12" r="1.1" fill="currentColor" stroke="none" />
  </Svg>
);

export const IconFolderOpen = (props: IconProps) => (
  <Svg {...props}>
    <path d="M3.5 8.2V6.4c0-1 .8-1.8 1.8-1.8h3.1l1.9 2.1h7.9c1 0 1.8.8 1.8 1.8v1.2" />
    <path d="M4.6 9.7h16.2l-2.2 8.1c-.2.8-.9 1.3-1.7 1.3H5.8c-1 0-1.8-.9-1.6-1.9l1.1-6.2c.1-.8.8-1.3 1.6-1.3Z" />
  </Svg>
);

export const IconSliders = (props: IconProps) => (
  <Svg {...props}>
    <path d="M4 7h6M14 7h6M4 17h10M18 17h2" />
    <circle cx="12" cy="7" r="2" />
    <circle cx="16" cy="17" r="2" />
  </Svg>
);

export const IconLayers = (props: IconProps) => (
  <Svg {...props}>
    <path d="m12 3.4 8.4 4.4-8.4 4.4L3.6 7.8 12 3.4Z" />
    <path d="m4.4 12 7.6 4 7.6-4" />
    <path d="m4.4 16.2 7.6 4 7.6-4" />
  </Svg>
);

export const IconArchive = (props: IconProps) => (
  <Svg {...props}>
    <rect x="3.5" y="4.5" width="17" height="4.5" rx="1.2" />
    <path d="M5.2 9v9.1c0 .8.7 1.4 1.4 1.4h10.8c.8 0 1.4-.7 1.4-1.4V9" />
    <path d="M9.7 13h4.6" />
  </Svg>
);

export const IconUndo = (props: IconProps) => (
  <Svg {...props}>
    <path d="M4 9.5h10.2a5.8 5.8 0 0 1 0 11.6H8.4" />
    <path d="M8.2 5.3 4 9.5l4.2 4.2" />
  </Svg>
);

export const IconChevronUp = (props: IconProps) => (
  <Svg {...props}>
    <path d="m5.5 14.5 6.5-6.5 6.5 6.5" />
  </Svg>
);

export const IconDownload = (props: IconProps) => (
  <Svg {...props}>
    <path d="M12 3.5v11M7.5 10.5 12 15l4.5-4.5" />
    <path d="M4.5 19.5h15" />
  </Svg>
);

export const IconApps = (props: IconProps) => (
  <Svg {...props}>
    <path d="M12 3.6 20 8v8l-8 4.4L4 16V8l8-4.4Z" />
    <path d="M4 8l8 4.4L20 8M12 12.4V20.4" />
  </Svg>
);

export const IconAnalyzer = (props: IconProps) => (
  <Svg {...props}>
    <rect x="3.5" y="3.5" width="17" height="17" rx="2.4" />
    <rect x="6.5" y="6.5" width="6" height="6" rx="0.8" />
    <path d="M15 6.5h2.5v3H15zM15 12.5h2.5v5H15zM6.5 15h6v2.5h-6z" />
  </Svg>
);

export const IconCleaner = (props: IconProps) => (
  <Svg {...props}>
    <path d="M4 15.5 13.2 6.3a2 2 0 0 1 2.8 0l2.2 2.2a2 2 0 0 1 0 2.8L9 20.5H5.2L4 19.3v-3.8Z" />
    <path d="M9.8 9.7l6.5 6.5M20 4v3M22 6.5h-3M5 4.5v2M6.5 5.5h-2" />
  </Svg>
);

export const IconActivity = (props: IconProps) => (
  <Svg {...props}>
    <path d="M3.6 12a8.4 8.4 0 1 0 2.6-6.1" />
    <path d="M3.5 5.5v4h4" />
    <path d="M12 8v4.4l3 1.8" />
  </Svg>
);

export const IconSettings = (props: IconProps) => (
  <Svg {...props}>
    <path d="M4 7h9M17 7h3M4 17h3M11 17h9" />
    <circle cx="15" cy="7" r="2.2" />
    <circle cx="9" cy="17" r="2.2" />
  </Svg>
);

export const IconChevronLeft = (props: IconProps) => (
  <Svg {...props}>
    <path d="m14.5 5.5-6.5 6.5 6.5 6.5" />
  </Svg>
);

export const IconChevronRight = (props: IconProps) => (
  <Svg {...props}>
    <path d="m9.5 5.5 6.5 6.5-6.5 6.5" />
  </Svg>
);

export const IconChevronDown = (props: IconProps) => (
  <Svg {...props}>
    <path d="m5.5 9.5 6.5 6.5 6.5-6.5" />
  </Svg>
);

export const IconSearch = (props: IconProps) => (
  <Svg {...props}>
    <circle cx="10.8" cy="10.8" r="6.3" />
    <path d="m15.5 15.5 4.5 4.5" />
  </Svg>
);

export const IconFolder = (props: IconProps) => (
  <Svg {...props}>
    <path d="M3.5 7.1c0-1 .8-1.8 1.8-1.8h3.2l2 2.2h8.2c1 0 1.8.8 1.8 1.8v8.4c0 1-.8 1.8-1.8 1.8H5.3c-1 0-1.8-.8-1.8-1.8V7.1Z" />
  </Svg>
);

export const IconFile = (props: IconProps) => (
  <Svg {...props}>
    <path d="M6.2 3.5h7.2l4.4 4.4v12.6H6.2V3.5Z" />
    <path d="M13.4 3.5v4.4h4.4" />
  </Svg>
);

export const IconDrive = (props: IconProps) => (
  <Svg {...props}>
    <rect x="3" y="5.5" width="18" height="10.5" rx="2" />
    <path d="M3 13h18" />
    <path d="M7 16.4h.01M11 16.4h.01M16.8 16.4h.01" />
  </Svg>
);

export const IconAlert = (props: IconProps) => (
  <Svg {...props}>
    <path d="M12 3.8 21 20H3L12 3.8Z" />
    <path d="M12 9.5V14M12 16.8h.01" />
  </Svg>
);

export const IconShield = (props: IconProps) => (
  <Svg {...props}>
    <path d="M12 3.4 19 6v5.6c0 4.2-2.8 7.3-7 9-4.2-1.7-7-4.8-7-9V6l7-2.6Z" />
    <path d="m8.8 12 2.2 2.2 4.2-4.4" />
  </Svg>
);

export const IconRefresh = (props: IconProps) => (
  <Svg {...props}>
    <path d="M20 11.4A8 8 0 0 0 5.9 6.6L4 8.5" />
    <path d="M4 4.4v4.1h4.1" />
    <path d="M4 12.6A8 8 0 0 0 18.1 17.4L20 15.5" />
    <path d="M20 19.6v-4.1h-4.1" />
  </Svg>
);

export const IconClose = (props: IconProps) => (
  <Svg {...props}>
    <path d="m6.5 6.5 11 11M17.5 6.5l-11 11" />
  </Svg>
);

export const IconPlay = (props: IconProps) => (
  <Svg {...props}>
    <path d="M8.5 5.5 18 12l-9.5 6.5v-13Z" />
  </Svg>
);

export const IconStop = (props: IconProps) => (
  <Svg {...props}>
    <rect x="6.5" y="6.5" width="11" height="11" rx="1.6" />
  </Svg>
);

export const IconLock = (props: IconProps) => (
  <Svg {...props}>
    <rect x="4.8" y="10.2" width="14.4" height="9.3" rx="2" />
    <path d="M8.4 10.2V7.8a3.6 3.6 0 0 1 7.2 0v2.4" />
  </Svg>
);

export const IconInfo = (props: IconProps) => (
  <Svg {...props}>
    <circle cx="12" cy="12" r="8.6" />
    <path d="M12 11v5M12 8.2h.01" />
  </Svg>
);

export const IconCheck = (props: IconProps) => (
  <Svg {...props}>
    <path d="m5 12.6 4.4 4.4L19 6.6" />
  </Svg>
);

export const IconTrash = (props: IconProps) => (
  <Svg {...props}>
    <path d="M4.5 6.8h15M9.5 6.8V4.5h5v2.3M6.8 6.8l.9 13h8.6l.9-13" />
    <path d="M10.2 10.5v5.8M13.8 10.5v5.8" />
  </Svg>
);

export const IconExternal = (props: IconProps) => (
  <Svg {...props}>
    <path d="M13.5 4.5H19.5V10.5" />
    <path d="m19.5 4.5-8 8" />
    <path d="M17.5 13.6v4.9c0 .8-.7 1.5-1.5 1.5H5.5c-.8 0-1.5-.7-1.5-1.5V8c0-.8.7-1.5 1.5-1.5h4.9" />
  </Svg>
);

export const IconClock = (props: IconProps) => (
  <Svg {...props}>
    <circle cx="12" cy="12" r="8.5" />
    <path d="M12 7.2V12l3.4 2" />
  </Svg>
);

export const IconPanel = (props: IconProps) => (
  <Svg {...props}>
    <rect x="3.5" y="4.5" width="17" height="15" rx="2" />
    <path d="M9.5 4.5v15" />
  </Svg>
);

export const IconSun = (props: IconProps) => (
  <Svg {...props}>
    <circle cx="12" cy="12" r="4" />
    <path d="M12 2.8v2M12 19.2v2M2.8 12h2M19.2 12h2M5.5 5.5l1.4 1.4M17.1 17.1l1.4 1.4M18.5 5.5l-1.4 1.4M6.9 17.1l-1.4 1.4" />
  </Svg>
);

export const IconMoon = (props: IconProps) => (
  <Svg {...props}>
    <path d="M20 14.4A8.4 8.4 0 0 1 9.6 4 8.6 8.6 0 1 0 20 14.4Z" />
  </Svg>
);

export const IconMonitor = (props: IconProps) => (
  <Svg {...props}>
    <rect x="3" y="4.5" width="18" height="12" rx="2" />
    <path d="M9 20h6M12 16.5V20" />
  </Svg>
);

export const IconArrowUpRight = (props: IconProps) => (
  <Svg {...props}>
    <path d="M7 17 17 7M9.5 7H17v7.5" />
  </Svg>
);

export const IconCopy = (props: IconProps) => (
  <Svg {...props}>
    <rect x="9" y="9" width="12" height="12" rx="2" />
    <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
  </Svg>
);

export const IconCrosshair = (props: IconProps) => (
  <Svg {...props}>
    <circle cx="12" cy="12" r="8" />
    <path d="M12 2v4M12 18v4M2 12h4M18 12h4M12 10a2 2 0 1 0 0 4 2 2 0 0 0 0-4Z" />
  </Svg>
);

export const IconGauge = (props: IconProps) => (
  <Svg {...props}>
    <path d="M4.2 18.2a8.6 8.6 0 1 1 15.6 0" />
    <path d="M12 13.4 16 9" />
    <circle cx="12" cy="14" r="1.5" />
    <path d="M3.2 18.2h17.6" />
  </Svg>
);

export const IconCpu = (props: IconProps) => (
  <Svg {...props}>
    <rect x="5" y="5" width="14" height="14" rx="2" />
    <rect x="9" y="9" width="6" height="6" />
    <path d="M9 1v4M15 1v4M9 19v4M15 19v4M1 9h4M1 15h4M19 9h4M19 15h4" />
  </Svg>
);
