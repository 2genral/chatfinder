interface MarkedTextProps {
  text: string;
}

export function MarkedText({ text }: MarkedTextProps) {
  const parts = text.split(/(\[\[\[|\]\]\])/g);
  let highlighted = false;

  return (
    <>
      {parts.map((part, index) => {
        if (part === "[[[") {
          highlighted = true;
          return null;
        }
        if (part === "]]]") {
          highlighted = false;
          return null;
        }
        return highlighted ? <mark key={`${index}-${part}`}>{part}</mark> : part;
      })}
    </>
  );
}
