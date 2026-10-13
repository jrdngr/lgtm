import type { PostDto } from "../bindings";
import { Post } from "./Post";
import { authorName } from "./people";
import "../review/comments.css";
import "./github.css";

const VERDICTS: Record<string, [string, string]> = {
  APPROVED: ["green", "Approved"],
  CHANGES_REQUESTED: ["red", "Changes requested"],
  COMMENTED: ["", "Reviewed"],
  DISMISSED: ["", "Dismissed"],
};

function VerdictPill({ verdict }: { verdict: string }) {
  const [tone, label] = VERDICTS[verdict] ?? ["", verdict.toLowerCase()];
  return <span className={`pill${tone ? ` ${tone}` : ""}`}>{label}</span>;
}

/** A PR's conversation comments and review summaries, oldest first. */
export function PrPosts({ posts, viewer }: { posts: PostDto[]; viewer?: string }) {
  if (posts.length === 0) return <div className="pr-empty">No conversation yet</div>;
  return (
    <div className="pr-posts">
      {posts.map((p) => (
        <Post
          key={p.url}
          author={p.author}
          name={authorName(p.author, viewer)}
          at={p.createdAt}
          pills={p.verdict && <VerdictPill verdict={p.verdict} />}
          body={p.body}
        />
      ))}
    </div>
  );
}
