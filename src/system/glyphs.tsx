/** The System tab's line icons. */

const svg = (size: number, children: React.ReactNode, width = 1.6) => (
  <svg
    width={size}
    height={size}
    viewBox="0 0 16 16"
    fill="none"
    stroke="currentColor"
    strokeWidth={width}
    strokeLinecap="round"
    strokeLinejoin="round"
    aria-hidden="true"
  >
    {children}
  </svg>
);

export const Up = () =>
  svg(
    16,
    <>
      <path d="M6 3.5L2.5 7 6 10.5" />
      <path d="M2.5 7h6.5a4 4 0 014 4v1.5" />
    </>,
  );

export const Chevron = ({ size = 12 }: { size?: number }) =>
  svg(size, <path d="M6 3.5L10.5 8 6 12.5" />, 2);

export const Arrow = () => svg(14, <path d="M3 8h10M9.5 4.5L13 8l-3.5 3.5" />);

export const Search = ({ size = 14 }: { size?: number }) =>
  svg(
    size,
    <>
      <circle cx="7" cy="7" r="4.5" />
      <path d="M10.5 10.5L14 14" />
    </>,
  );

export const Trace = () =>
  svg(
    14,
    <>
      <circle cx="3.5" cy="12.5" r="1.8" />
      <circle cx="12.5" cy="3.5" r="1.8" />
      <path d="M5 11.5c3-1 1-6 6-7" />
    </>,
  );

export const Play = ({ size = 14 }: { size?: number }) =>
  svg(size, <path d="M4 2.5l9 5.5-9 5.5z" />);

export const Close = () => svg(12, <path d="M4 4l8 8M12 4l-8 8" />, 1.8);

export const Person = () =>
  svg(
    22,
    <>
      <circle cx="8" cy="5.5" r="2.8" />
      <path d="M2.5 14c.6-3 2.8-4.5 5.5-4.5s4.9 1.5 5.5 4.5" />
    </>,
    1.4,
  );

export const Plus = () => svg(14, <path d="M8 3v10M3 8h10" />);

export const Minus = () => svg(14, <path d="M3 8h10" />);

export const Fit = () =>
  svg(14, <path d="M2.5 6V2.5H6M10 2.5h3.5V6M13.5 10v3.5H10M6 13.5H2.5V10" />);
