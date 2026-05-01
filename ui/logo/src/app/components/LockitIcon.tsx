import React from 'react';

interface LockitIconProps {
  size?: number;
  variant?: 'light' | 'dark' | 'gradient';
  className?: string;
}

export default function LockitIcon({ size = 128, variant = 'dark', className }: LockitIconProps) {
  const getColors = () => {
    switch (variant) {
      case 'light':
        return {
          base: '#000000',
          accent1: '#8B0000', // Dark Red
          accent2: '#A84400', // Dark Orange
        };
      case 'dark':
      case 'gradient':
      default:
        return {
          base: '#FFFFFF',
          accent1: '#991B1B', // Dark Red
          accent2: '#B84A00', // Dark Orange
        };
    }
  };

  const colors = getColors();

  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 100 100"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      className={className}
    >
      {/* Left Bracket Top */}
      <path
        d="M 22 10 L 10 10 L 10 44"
        stroke={colors.base}
        strokeWidth="8"
        fill="none"
        strokeLinecap="square"
        strokeLinejoin="miter"
      />
      {/* Left Bracket Bottom */}
      <path
        d="M 10 56 L 10 90 L 22 90"
        stroke={colors.base}
        strokeWidth="8"
        fill="none"
        strokeLinecap="square"
        strokeLinejoin="miter"
      />
      {/* Left Connector Node */}
      <rect x="6" y="48" width="10" height="4" fill={colors.accent1} />

      {/* Right Bracket Top */}
      <path
        d="M 78 10 L 90 10 L 90 44"
        stroke={colors.base}
        strokeWidth="8"
        fill="none"
        strokeLinecap="square"
        strokeLinejoin="miter"
      />
      {/* Right Bracket Bottom */}
      <path
        d="M 90 56 L 90 90 L 78 90"
        stroke={colors.base}
        strokeWidth="8"
        fill="none"
        strokeLinecap="square"
        strokeLinejoin="miter"
      />
      {/* Right Connector Node */}
      <rect x="84" y="48" width="10" height="4" fill={colors.accent2} />

      {/* Top Left Quadrant - Dark Red */}
      <rect x="24" y="24" width="24" height="24" fill={colors.accent1} />

      {/* Top Right Quadrant - Outlined Base */}
      <rect
        x="55"
        y="27"
        width="18"
        height="18"
        fill="none"
        stroke={colors.base}
        strokeWidth="6"
        strokeLinecap="square"
      />

      {/* Bottom Left Quadrant - Solid Base with Terminal Cursor Hole */}
      <path
        fillRule="evenodd"
        clipRule="evenodd"
        d="M 24 52 H 48 V 76 H 24 V 52 Z M 30 66 H 42 V 70 H 30 V 66 Z"
        fill={colors.base}
      />

      {/* Bottom Right Quadrant - Dark Orange */}
      <rect x="52" y="52" width="24" height="24" fill={colors.accent2} />

      {/* Core/Grid Locking Pins */}
      <rect x="48" y="48" width="4" height="4" fill={colors.base} />
      <rect x="24" y="48" width="4" height="4" fill={colors.base} />
      <rect x="72" y="48" width="4" height="4" fill={colors.base} />
      <rect x="48" y="24" width="4" height="4" fill={colors.base} />
      <rect x="48" y="72" width="4" height="4" fill={colors.base} />
    </svg>
  );
}
