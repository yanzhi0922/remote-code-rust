interface BrandMarkProps {
  className?: string;
  imageClassName?: string;
}

export function BrandMark({
  className = 'h-8 w-8',
  imageClassName = 'h-full w-full',
}: BrandMarkProps) {
  return (
    <span className={`inline-flex shrink-0 items-center justify-center ${className}`}>
      <img src="/brand-mark.svg" alt="" className={imageClassName} draggable={false} />
    </span>
  );
}
