export function BrandMark({ size = 22 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 64 64" aria-hidden="true">
      <rect width="64" height="64" rx="13" fill="#17150F" />
      <path
        d="M17 11h30v35.5l-4.4-3-4.5 3-4.5-3-4.5 3-4.5-3-4.6 3V11z"
        fill="#F5F0E4"
      />
      <path
        d="M23 22h18M23 28h18M23 34h9"
        stroke="#17150F"
        strokeWidth="2.4"
        strokeLinecap="square"
      />
      <circle cx="46" cy="45" r="12" fill="#1E6B4C" />
      <path
        d="M40 45l4 4 8-9"
        stroke="#F5F0E4"
        strokeWidth="3.4"
        fill="none"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}
