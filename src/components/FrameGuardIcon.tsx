import iconSrc from '../assets/frameguard-icon.png';

interface Props {
  size?: number;
  className?: string;
}

export default function FrameGuardIcon({ size = 24, className }: Props) {
  return (
    <img
      src={iconSrc}
      alt="FrameGuard"
      width={size}
      height={size}
      className={className}
      style={{ objectFit: 'contain' }}
    />
  );
}
