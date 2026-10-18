import type { ReactNode } from "react";
import { Markdown } from "../bots/Markdown";
import { hue } from "../model/people";
import { ago, fullDate } from "../ui/time";
import "./github.css";

/** A person's initial on a disc in their color. */
export function Avatar({ name, small }: { name: string; small?: boolean }) {
  return (
    <span
      className={`avatar${small ? " small" : ""}`}
      style={{ background: `hsl(${hue(name)} 55% 68%)` }}
      aria-hidden="true"
    >
      {name.slice(0, 1).toUpperCase()}
    </span>
  );
}

interface Props {
  /** The login, which picks the avatar's color. */
  author: string;
  /** What shows as the author, like "You". Defaults to `author`. */
  name?: string;
  /** When it was written, ISO or epoch ms. */
  at?: string | number | null;
  /** Pills after the time. */
  pills?: ReactNode;
  body: string;
}

/** One post in a conversation: avatar, author, how long ago, and the body as markdown. */
export function Post({ author, name = author, at, pills, body }: Props) {
  return (
    <div className="post">
      <Avatar name={author} />
      <div className="post-meta">
        <b>{name}</b>
        {at ? (
          <span className="post-time" title={fullDate(at)}>
            {ago(at)}
          </span>
        ) : null}
        {pills}
      </div>
      {body.trim() && (
        <div className="post-body">
          <Markdown>{body}</Markdown>
        </div>
      )}
    </div>
  );
}
