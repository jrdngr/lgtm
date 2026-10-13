import type { GhLink } from "../bindings";
import { Post } from "./Post";
import { authorName } from "./people";

/** A GitHub review thread's posts, above the reply being written for it. */
export function GhThread({ link, viewer }: { link: GhLink; viewer?: string }) {
  return (
    <>
      {link.posts.map((p, i) => (
        <Post
          // biome-ignore lint/suspicious/noArrayIndexKey: posts are append-only
          key={i}
          author={p.author}
          name={p.pending ? "You" : authorName(p.author, viewer)}
          at={p.createdAt}
          pills={p.pending && <span className="pill amber">Pending</span>}
          body={p.body}
        />
      ))}
    </>
  );
}

/** A thread's last post, as "who: what", for a one-line summary. */
export function lastPost(link: GhLink, viewer?: string) {
  const last = link.posts.at(-1);
  return last && `${authorName(last.author, viewer)}: ${last.body}`;
}
