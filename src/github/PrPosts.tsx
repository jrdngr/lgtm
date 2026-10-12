import type { PostDto } from "../bindings";
import "../review/comments.css";
import "./github.css";

const VERDICTS: Record<string, string> = {
  APPROVED: "approved",
  CHANGES_REQUESTED: "requested changes",
  COMMENTED: "reviewed",
  DISMISSED: "review dismissed",
};

const when = (iso: string) => (iso ? new Date(iso).toLocaleString() : "");

/** A PR's conversation comments and review summaries, oldest first. */
export function PrPosts({ posts }: { posts: PostDto[] }) {
  if (posts.length === 0) return <div className="placeholder">No conversation yet</div>;
  return (
    <>
      {posts.map((p) => (
        <div key={p.url} className="pr-post">
          <div className="msg-who" title={when(p.createdAt)}>
            {p.author}
            {p.verdict && ` ${VERDICTS[p.verdict] ?? p.verdict.toLowerCase()}`}
          </div>
          {p.body && <div className="msg-text">{p.body}</div>}
        </div>
      ))}
    </>
  );
}
