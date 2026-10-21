import { remembered } from "../core/remembered";

/** What the revisions log's rows show beyond ids, descriptions and bookmarks. */
export const showAuthor = remembered("lgtm:logAuthor", false, [true, false]);
export const showDate = remembered("lgtm:logDate", false, [true, false]);
