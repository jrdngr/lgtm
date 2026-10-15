/** Links to findings by id, which jump to their cards. */
export function Refs({
  ids,
  onGo,
  gone,
  label = (id) => (id.includes("#") ? id : `#${id}`),
}: {
  ids: string[];
  onGo(id: string): void;
  /** Ids whose finding no longer exists, shown but not linked. */
  gone?(id: string): boolean;
  label?(id: string): string;
}) {
  return (
    <>
      {ids.map((id) => (
        <button
          type="button"
          className="finding-ref"
          key={id}
          disabled={gone?.(id)}
          onClick={() => onGo(id)}
        >
          {label(id)}
        </button>
      ))}
    </>
  );
}
