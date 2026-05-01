import { useEffect, useState } from 'react';

interface StreamingMessageProps {
  content: string;
  isStreaming: boolean;
}

export function StreamingMessage({ content, isStreaming }: StreamingMessageProps) {
  const [displayedContent, setDisplayedContent] = useState(content);

  useEffect(() => {
    if (!isStreaming) {
      setDisplayedContent(content);
      return;
    }
    // For streaming, show content as it arrives with a cursor
    setDisplayedContent(content);
  }, [content, isStreaming]);

  return (
    <span>
      {displayedContent}
      {isStreaming && (
        <span className="inline-block w-[2px] h-[1em] bg-rc-accent-primary animate-pulse ml-0.5 align-middle" />
      )}
    </span>
  );
}
