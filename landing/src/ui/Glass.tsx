import type { CSSProperties, ReactNode } from 'react';

type GlassProps = {
  children?: ReactNode;
  className?: string;
  style?: CSSProperties;
  /** stronger glass for key surfaces */
  intense?: boolean;
  as?: 'div' | 'section' | 'article';
};

/** Lightweight liquid-glass surface (CSS only — safe to use many times). */
export function Glass({
  children,
  className = '',
  style,
  intense = false,
  as: Tag = 'div',
}: GlassProps) {
  return (
    <Tag
      className={`glass ${intense ? 'glass-intense' : ''} ${className}`}
      style={style}
    >
      <div className="glass-shine" aria-hidden />
      {children != null ? <div className="glass-body h-full min-h-0">{children}</div> : null}
    </Tag>
  );
}

export function Pill({
  children,
  className = '',
}: {
  children: ReactNode;
  className?: string;
}) {
  return <span className={`glass-pill ${className}`}>{children}</span>;
}
