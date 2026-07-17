import katex from 'katex';
import 'katex/dist/katex.min.css';

type Props = {
  tex: string;
  display?: boolean;
  className?: string;
  label?: string;
};

export default function MathFormula({ tex, display = false, className = '', label }: Props) {
  const html = katex.renderToString(tex, {
    displayMode: display,
    throwOnError: false,
    strict: false,
    output: 'html',
  });
  const Tag = display ? 'div' : 'span';
  return (
    <Tag
      className={`math-formula ${display ? 'math-display' : 'math-inline'} ${className}`}
      aria-label={label ?? tex}
      dangerouslySetInnerHTML={{ __html: html }}
    />
  );
}
