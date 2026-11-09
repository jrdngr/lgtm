/** What `space` goes on to in the review, shown while it waits for the next key. */

export interface SpaceItem {
  key: string;
  label: string;
}

export function SpaceMenu({ items }: { items: SpaceItem[] }) {
  return (
    <div className="mark-menu space-menu">
      {items.map((item) => (
        <div key={item.key} className="space-item">
          <span className="menu-key">{item.key}</span>
          {item.label}
        </div>
      ))}
    </div>
  );
}
