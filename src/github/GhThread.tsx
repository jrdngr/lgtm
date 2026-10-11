import type { GhLink } from "../bindings";

/** A GitHub review thread's posts, above the reply being written for it. */
export function GhThread({ link }: { link: GhLink }) {
  return (
    <div className="gh-thread">
      {link.posts.map((p, i) => (
        // biome-ignore lint/suspicious/noArrayIndexKey: posts are append-only
        <div key={i} className="gh-post">
          <span className="msg-who">
            {p.author}
            {p.pending && <span className="gh-pending">pending</span>}
          </span>
          <div className="msg-text">{p.body}</div>
        </div>
      ))}
    </div>
  );
}

/** What a thread's block says about it in its header. */
export function threadState(link: GhLink) {
  return [link.resolved && "resolved", link.outdated && "outdated"].filter(Boolean).join(" · ");
}
