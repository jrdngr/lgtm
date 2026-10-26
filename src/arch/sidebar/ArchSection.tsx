import { useMemo, useState } from "react";
import type { LookDto } from "../../bindings";
import { useWorkspace, type WorkspaceName } from "../../core/workspace";
import { DesignsSection } from "../../design/DesignsSection";
import { type CurrentReview, useCurrentReview } from "../../review/current";
import { useSendText } from "../../review/useSendText";
import { GenerateButton, GeneratePop } from "../../system/GenerateMenu";
import { generatePrompt, genItems } from "../../system/generate";
import { lensOf } from "../../system/lens";
import { SystemTree } from "../../system/SystemTree";
import { showInSystem } from "../../system/showInSystem";
import { useSystem } from "../../system/useSystem";
import { keepSubmit } from "../../terminal/registry";
import { Busy } from "../../ui/Busy";
import { Empty, SectionHeader } from "../../ui/SectionHeader";
import { useArch } from "../state";
import { History } from "./History";
import { Layers } from "./Layers";
import { Look } from "./Look";
import { Score } from "./Score";
import "./sidebar.css";

/** The workspace's architecture, with Generate. With a review open: the assessment's score, what to look at, the
 * change stack, the map of what it changes and the commits that changed it. Without one: the
 * system's tours, people, external systems and containers. */
export function ArchSection() {
  const workspace = useWorkspace();
  const review = useCurrentReview(workspace);
  const [error, setError] = useState<string>();
  const [genOpen, setGenOpen] = useState(false);
  const [sending, setSending] = useState(false);
  const { sendText, picker } = useSendText(workspace, setError, () => {}, "background");
  const { map } = useSystem(workspace);
  const { arch } = useArch(review);
  const gen = genItems(map, review && { arch });
  const send = async (
    p: Promise<{ status: "ok"; data: string } | { status: "error"; error: string }>,
  ) => {
    setSending(true);
    const r = await keepSubmit(p).finally(() => setSending(false));
    if (r.status === "error") return setError(r.error);
    setError(undefined);
    sendText(r.data);
  };
  return (
    <div className="side-split arch-split">
      <SectionHeader title="Architecture">
        <GenerateButton
          items={gen}
          open={genOpen}
          busy={sending}
          onToggle={() => setGenOpen(!genOpen)}
        />
      </SectionHeader>
      {genOpen && (
        <GeneratePop
          className="side"
          items={gen}
          onGenerate={(keys) => {
            setGenOpen(false);
            send(generatePrompt(workspace, review?.id, keys));
          }}
          onClose={() => setGenOpen(false)}
        />
      )}
      {error && <div className="arch-error">{error}</div>}
      <DesignsSection onError={setError} />
      {review ? (
        <Body key={review.id} workspace={workspace} review={review} onError={setError} />
      ) : (
        <NoReview workspace={workspace} />
      )}
      {picker}
    </div>
  );
}

function NoReview({ workspace }: { workspace: WorkspaceName }) {
  const { map, error } = useSystem(workspace);
  if (!map) return <Empty>{error ?? "Loading…"}</Empty>;
  return (
    <div className="side-split-body sys-tree">
      <SystemTree map={map} />
    </div>
  );
}

function Body({
  workspace,
  review,
  onError,
}: {
  workspace: WorkspaceName;
  review: CurrentReview;
  onError(e: string | undefined): void;
}) {
  const { arch, error, view } = useArch(review);
  const { map } = useSystem(workspace);
  const lens = useMemo(() => arch && lensOf(arch, review.files), [arch, review.files]);
  const showLook = (l: LookDto) => {
    if (l.path) review.show(l.path, l.line);
    else if (l.edge) showInSystem(workspace, `k:${l.edge[0]}>k:${l.edge[1]}`);
    else if (l.component) showInSystem(workspace, `k:${l.component}`);
  };
  return (
    <div className="side-split-body arch-side sys-tree">
      <Score assessment={arch?.assessment} />
      {error && <div className="arch-error">{error}</div>}
      {!arch ? (
        !error && (
          <div className="placeholder arch-loading">
            <Busy>Loading…</Busy>
          </div>
        )
      ) : (
        <>
          <Look
            workspace={workspace}
            review={review}
            look={arch.look}
            onShow={showLook}
            onError={onError}
          />
          <Layers review={review} layers={arch.layers} assessed={!!arch.assessment} />
          {map && lens && <SystemTree map={map} lens={lens} />}
        </>
      )}
      <History review={review} view={view} />
    </div>
  );
}
