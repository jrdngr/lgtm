import { type KeyboardEvent, useEffect, useMemo, useRef, useState } from "react";
import { showLook } from "../arch/showLook";
import { setArchView, useArch } from "../arch/state";
import type { LookDto, SystemDto, TourDto } from "../bindings";
import { revealInFiles } from "../core/reveal";
import { useWorkspace } from "../core/workspace";
import { openFile } from "../editor/open";
import { usePr } from "../github/pr";
import { commands } from "../ipc";
import { useCurrentReview } from "../review/current";
import { useSendText } from "../review/useSendText";
import { PipeButton } from "../terminal/PipeButton";
import type { CtxEntry } from "../ui/contextMenu";
import { NEUTRAL } from "./colors";
import { EdgeDetail } from "./EdgeDetail";
import { FindPalette } from "./FindPalette";
import { GenerateButton, GeneratePop } from "./GenerateMenu";
import { type GenKey, generatePrompt, genItems } from "./generate";
import { groupsOf } from "./groups";
import { layout } from "./layout";
import { lensOf } from "./lens";
import { edgeKey } from "./mapWires";
import { nodeInfo } from "./names";
import { NAV, navPush, navStep, type Place, sameLevel } from "./nav";
import { ReviewStrip } from "./ReviewStrip";
import { SymbolsView } from "./SymbolsView";
import { type Crumb, SystemBar } from "./SystemBar";
import { SystemCanvas } from "./SystemCanvas";
import { SystemDetail } from "./SystemDetail";
import {
  componentId,
  containerColors,
  containerId,
  containerOf,
  type Level,
  largestContainer,
  levelOf,
  nodesOf,
  type SEdge,
  type SNode,
  SYSTEM,
  sceneOf,
} from "./scene";
import { onShowInSystem, setSelectedNode } from "./showInSystem";
import { TourCard, TourSteps } from "./Tour";
import { ToursMenu } from "./ToursMenu";
import { TracePanel } from "./TracePanel";
import { TraceModes, TraceView } from "./TraceView";
import { stopsOf, tourOf } from "./tours";
import { drawn } from "./trace";
import { traceTargets } from "./traceTargets";
import { useSystem } from "./useSystem";
import { useTrace } from "./useTrace";
import { askPrompt } from "./view";
import "./system.css";
import "./lens.css";

/** The whole system as C4 context, containers, components and one component's symbols, from
 * the workspace's working copy and the description a terminal agent wrote, with the current
 * review's change over it. */
export function ArchitecturePanel() {
  const workspace = useWorkspace();
  const { map, error, reload } = useSystem(workspace);
  if (error && !map) return <div className="placeholder error">{error}</div>;
  if (!map) return <div className="placeholder">Reading the system…</div>;
  if (!map.components.length) return <div className="placeholder">No source files</div>;
  return <Loaded map={map} error={error} reload={reload} />;
}

/** The node that stands for `level` when nothing is selected. */
function subject(map: SystemDto, level: Level): SNode | undefined {
  if (level.kind === "context" || level.kind === "containers") {
    const g = level.kind === "containers" && groupsOf(map)?.find((x) => x.id === level.group);
    if (g) return { id: g.id, kind: "group", name: g.name, description: g.description };
    return { id: SYSTEM, kind: "system", name: map.repo, description: map.description?.summary };
  }
  const c = level.kind === "symbols" ? componentId(level.component) : containerId(level.container);
  const at: Level =
    level.kind === "symbols"
      ? { kind: "components", container: containerOf(map, level.component) }
      : { kind: "containers" };
  return nodesOf(sceneOf(map, at)).find((n) => n.id === c);
}

/** The scene edge for `from>to`, or the one into the door that stands for `to`'s container. */
function sceneEdge(map: SystemDto, edges: SEdge[], key: string): SEdge | undefined {
  const [from, to] = key.split(">");
  const exact = edges.find((e) => e.from === from && e.to === to);
  if (exact || !to?.startsWith("k:")) return exact;
  const door = containerId(containerOf(map, to.slice(2)));
  return edges.find((e) => e.from === from && e.to === door);
}

function Loaded({ map, error, reload }: { map: SystemDto; error?: string; reload(): void }) {
  const workspace = useWorkspace();
  const root = useRef<HTMLDivElement>(null);
  const [level, setLevel] = useState<Level>({ kind: "context" });
  const [selected, setSelected] = useState<string>();
  const [edgeSel, setEdgeSel] = useState<string>();
  const [symSel, setSymSel] = useState<string>();
  const [nav, setNav] = useState(NAV);
  /** The user's lens toggle; until they use it, the lens is on only for a PR's review. */
  const [lensPick, setLensPick] = useState<boolean>();
  const [finding, setFinding] = useState(false);
  const [toursOpen, setToursOpen] = useState(false);
  const [genOpen, setGenOpen] = useState(false);
  const tracing = useTrace(map);
  const { trace } = tracing;
  const [touring, setTouring] = useState<{ tour: TourDto; at: number }>();
  const [sendError, setSendError] = useState<string>();
  const { sendText, picker } = useSendText(workspace, setSendError, () => root.current?.focus());
  const review = useCurrentReview(workspace);
  const pr = usePr(workspace);
  const lensOn = lensPick ?? (!!pr && review?.revset === pr.revset);
  const lensReview = lensOn ? review : undefined;
  const { arch, error: reviewError, view } = useArch(review);
  const archError = lensOn ? reviewError : undefined;
  const files = lensReview?.files;
  const lens = useMemo(() => (arch && files ? lensOf(arch, files) : undefined), [arch, files]);
  const colors = useMemo(() => containerColors(map), [map]);
  const targets = useMemo(() => traceTargets(map, colors), [map, colors]);

  // What a reload or another review can take away.
  const shown: Level =
    (level.kind === "components" && !map.containers.some((c) => c.id === level.container)) ||
    (level.kind === "symbols" && !map.components.some((k) => k.id === level.component))
      ? { kind: "containers" }
      : level;
  const scene = useMemo(() => sceneOf(map, shown, lens), [map, shown, lens]);
  const placed = useMemo(() => layout(scene), [scene]);
  const sel = placed.nodes.find((n) => n.id === selected);
  const detail = sel ?? subject(map, shown);
  const edge = edgeSel ? sceneEdge(map, scene.edges, edgeSel) : undefined;
  const [selFrom = "", selTo = ""] = edgeSel?.split(">") ?? [];
  const ends = edgeSel ? { from: selFrom, to: selTo } : undefined;

  useEffect(() => setSelectedNode(workspace, selected), [workspace, selected]);

  const here: Place = { level: shown, node: selected, edge: edgeSel };
  const place = (p: Place) => {
    setLevel(p.level);
    setSelected(p.node);
    setEdgeSel(p.edge);
    setSymSel(undefined);
  };
  /** Shows `next`, remembering where it left when it changes level. */
  const go = (next: Level, node?: string, e?: string) => {
    const to = { level: next, node, edge: e };
    if (!sameLevel(next, shown)) setNav(navPush(nav, here, to));
    place(to);
  };
  const step = (by: -1 | 1) => {
    const n = navStep(nav, here, by);
    if (n === nav) return;
    setNav(n);
    place(n.stack[n.at]);
  };
  // The mouse's back and forward buttons.
  const stepRef = useRef(step);
  stepRef.current = step;
  useEffect(() => {
    const el = root.current;
    const onUp = (e: MouseEvent) => {
      if (e.button === 3) stepRef.current(-1);
      else if (e.button === 4) stepRef.current(1);
    };
    el?.addEventListener("mouseup", onUp);
    return () => el?.removeEventListener("mouseup", onUp);
  }, []);
  const canBack = nav.at > 0;
  const canForward = nav.at >= 0 && nav.at < nav.stack.length - 1;
  /** Shows `id`, a node or `from>to`, at the level that holds it, staying put when this level
   * shows it. */
  const show = (id: string) => {
    if (id.startsWith("tour:")) {
      const tour = map.tours.find((t) => t.id === id.slice(5));
      return tour && startTour(tour);
    }
    const [a, b] = id.split(">");
    if (b !== undefined) {
      const at = levelOf(map, a);
      return at && go(at.kind === "context" ? (levelOf(map, b) ?? at) : at, undefined, id);
    }
    if (placed.nodes.some((n) => n.id === id && !n.door)) {
      setEdgeSel(undefined);
      return setSelected(id);
    }
    const at = levelOf(map, id);
    if (at) go(at, id);
  };
  const groups = useMemo(() => groupsOf(map), [map]);
  const groupOf = (container: string) => groups?.find((g) => g.containers.includes(container))?.id;
  const open = (id: string) => {
    if (id === SYSTEM) return go({ kind: "containers" });
    if (groups?.some((g) => g.id === id)) return go({ kind: "containers", group: id });
    const c = map.containers.find((c) => containerId(c.id) === id);
    if (c && map.components.some((k) => k.container === c.id)) {
      go({ kind: "components", container: c.id });
    }
    if (id.startsWith("k:")) go({ kind: "symbols", component: id.slice(2) });
  };
  const up =
    shown.kind === "symbols"
      ? () =>
          go(
            { kind: "components", container: containerOf(map, shown.component) },
            componentId(shown.component),
          )
      : shown.kind === "components"
        ? () =>
            go(
              { kind: "containers", group: groupOf(shown.container) },
              containerId(shown.container),
            )
        : shown.kind === "containers"
          ? () => go({ kind: "context" }, shown.group ?? SYSTEM)
          : undefined;
  const container =
    shown.kind === "components"
      ? shown.container
      : shown.kind === "symbols"
        ? containerOf(map, shown.component)
        : largestContainer(map);
  const symbolsOf =
    shown.kind === "symbols" ? shown.component : sel?.kind === "component" ? sel.path : undefined;
  const pickLevel = (kind: Level["kind"]) => {
    if (kind === "symbols") return symbolsOf !== undefined && go({ kind, component: symbolsOf });
    if (kind !== "components") return go({ kind });
    if (container !== undefined) go({ kind, container });
  };

  useEffect(() => onShowInSystem(workspace, show));

  // Each tour step shows its place.
  const tour = touring?.tour;
  const stops = useMemo(() => (tour ? stopsOf(map, colors, tour) : []), [map, colors, tour]);
  const stop = touring ? stops[touring.at] : undefined;
  // biome-ignore lint/correctness/useExhaustiveDependencies: moves only when the step does
  useEffect(() => {
    if (!stop) return;
    go(stop.level, stop.node, stop.edge && `${stop.edge.from}>${stop.edge.to}`);
  }, [stop?.key, tour]);
  const startTour = (tour: TourDto) => {
    tracing.end();
    setToursOpen(false);
    setTouring({ tour, at: 0 });
  };
  const endTour = () => {
    setTouring(undefined);
    root.current?.focus();
  };
  const tourAt = (at: number) => touring && setTouring({ ...touring, at });
  const startTrace = (ends: { from?: string; to?: string }) => {
    setTouring(undefined);
    // An edge can end where nothing traces from, like the system at the context level.
    const ok = (id?: string) => (id && targets.some((t) => t.id === id) ? id : undefined);
    tracing.start({ from: ok(ends.from), to: ok(ends.to) });
  };
  const endTrace = () => {
    tracing.end();
    root.current?.focus();
  };

  const { hops } = tracing;
  const path = hops && drawn(map, shown, hops);
  const tourNode = stop?.node;
  const lit = path
    ? path.nodes
    : tourNode
      ? new Set([tourNode])
      : edge
        ? new Set([edge.from, edge.to])
        : undefined;
  const hot = new Map<string, number | undefined>();
  if (path) for (const [k, n] of path.edges) hot.set(k, n);
  else if (edge) hot.set(edgeKey(edge), undefined);
  else if (tourNode) {
    for (const e of scene.edges)
      if (e.from === tourNode || e.to === tourNode) hot.set(edgeKey(e), undefined);
  }
  const hotColor = path
    ? "#e6e6e6"
    : tourNode || edge
      ? (nodeInfo(map, colors, tourNode ?? edge?.from ?? "")?.color ?? NEUTRAL)
      : NEUTRAL;
  const stale = useMemo(() => {
    if (!map.description) return new Set<string>();
    const ids = new Set(map.stale.map((id) => `k:${id}`));
    for (const k of map.components) if (map.stale.includes(k.id)) ids.add(containerId(k.container));
    return ids;
  }, [map]);

  const pickNode = (id: string | undefined) => {
    setToursOpen(false);
    const door = placed.nodes.find((n) => n.id === id)?.door;
    if (id && door && !trace) {
      return id.startsWith("g:") || map.components.some((k) => containerId(k.container) === id)
        ? open(id)
        : show(id);
    }
    setEdgeSel(undefined);
    if (!trace || !id) return setSelected(id);
    if (targets.some((t) => t.id === id)) tracing.pick(id);
  };
  const crumb = (id: string, onClick?: () => void): Crumb | undefined => {
    const info = nodeInfo(map, colors, id);
    return (
      info && {
        id,
        name: info.name,
        onClick,
      }
    );
  };
  const comp =
    shown.kind === "symbols"
      ? componentId(shown.component)
      : sel?.kind === "component"
        ? sel.id
        : undefined;
  const crumbGroup =
    shown.kind === "containers"
      ? shown.group
      : shown.kind === "components" || shown.kind === "symbols"
        ? groupOf(container ?? "")
        : undefined;
  const groupName = groups?.find((g) => g.id === crumbGroup)?.name;
  const crumbs: Crumb[] = [
    { id: SYSTEM, name: map.repo, onClick: () => go({ kind: "context" }, SYSTEM) },
    ...(crumbGroup && groupName
      ? [{ id: crumbGroup, name: groupName, onClick: () => open(crumbGroup) }]
      : []),
    ...(shown.kind === "components" || shown.kind === "symbols"
      ? [
          crumb(containerId(container ?? ""), () =>
            go({ kind: "components", container: container ?? "" }),
          ),
        ]
      : []),
    ...(comp && (shown.kind === "components" || shown.kind === "symbols") ? [crumb(comp)] : []),
  ].filter((c): c is Crumb => !!c);

  const onFile = (p: string, line?: number) => openFile(workspace, p, line);
  const send = async (
    text: Promise<{ status: "ok"; data: string } | { status: "error"; error: string }>,
  ) => {
    const r = await text;
    if (r.status === "error") return setSendError(r.error);
    setSendError(undefined);
    sendText(r.data);
  };
  const gen = genItems(map, review && { arch });
  const generate = (keys: GenKey[]) => {
    setGenOpen(false);
    send(generatePrompt(workspace, review?.id, keys));
  };
  const writeTour = (ref: string) => {
    setToursOpen(false);
    send(commands.tourPrompt(workspace, ref));
  };
  const ask = (id: string) => {
    const n = placed.nodes.find((x) => x.id === id) ?? (detail?.id === id ? detail : undefined);
    if (n) sendText(askPrompt(map, n));
  };
  const onLook = (l: LookDto) => {
    if (l.path && review) review.show(l.path, l.line);
    else if (l.edge) show(`k:${l.edge[0]}>k:${l.edge[1]}`);
    else if (l.component) show(componentId(l.component));
  };
  const nodeMenu = (id: string): CtxEntry[] => {
    if (trace) return [];
    const info = nodeInfo(map, colors, id);
    const tour = tourOf(map, id);
    const tourItem = tour
      ? { label: "Tour", onSelect: () => startTour(tour) }
      : { label: "Write Tour", onSelect: () => writeTour(id) };
    const askItem = { label: "Ask", keys: "⌘>", onSelect: () => ask(id) };
    const traceTo = { label: "Trace To…", onSelect: () => startTrace({ from: id }) };
    const traceFrom = { label: "Trace From…", onSelect: () => startTrace({ to: id }) };
    if (info?.kind === "component") {
      const path = id.slice(2);
      return [
        { label: "Symbols", onSelect: () => go({ kind: "symbols", component: path }) },
        tourItem,
        traceTo,
        traceFrom,
        "-",
        { label: "Show in Files", onSelect: () => revealInFiles(workspace, path) },
        askItem,
      ];
    }
    if (info?.kind === "group") {
      return [{ label: "Containers", onSelect: () => open(id) }, "-", askItem];
    }
    if (info?.kind === "container") {
      return [
        map.components.some((k) => containerId(k.container) === id) && {
          label: "Components",
          onSelect: () => open(id),
        },
        tourItem,
        traceTo,
        traceFrom,
        "-",
        askItem,
      ];
    }
    return [askItem];
  };

  const onKey = (e: KeyboardEvent) => {
    if (finding) return;
    const mod = e.metaKey || e.ctrlKey;
    if (mod && e.shiftKey && (e.key === ">" || e.key === ".")) {
      if (detail) ask(detail.id);
    } else if (e.ctrlKey && !e.metaKey && (e.key === "-" || e.key === "_")) {
      step(e.shiftKey ? 1 : -1);
    } else if (mod) return;
    else if (e.key === "/") setFinding(true);
    else if (stop && e.key === "ArrowRight")
      tourAt(Math.min(stops.length - 1, (touring?.at ?? 0) + 1));
    else if (stop && e.key === "ArrowLeft") tourAt(Math.max(0, (touring?.at ?? 0) - 1));
    else if (e.key === "Escape" && toursOpen) setToursOpen(false);
    else if (e.key === "Escape" && genOpen) setGenOpen(false);
    else if (e.key === "Escape" && trace) endTrace();
    else if (e.key === "Escape" && touring) endTour();
    else if (e.key === "Escape" && symSel !== undefined) setSymSel(undefined);
    else if (e.key === "Escape" && (selected || edgeSel)) go(shown);
    else if ((e.key === "Escape" || e.key === "Backspace") && up) up();
    else return;
    e.preventDefault();
  };

  const ofSystem = touring?.tour.id === "overview" || touring?.tour.id === "features";
  const tourInfo = touring && !ofSystem ? nodeInfo(map, colors, touring.tour.id) : undefined;
  const tourTitle = tourInfo?.name ?? touring?.tour.title ?? "";
  const tourColor = tourInfo?.color ?? NEUTRAL;

  const steps =
    touring && stop ? (
      <TourSteps
        title={`${tourTitle} tour`}
        color={tourColor}
        stops={stops}
        at={touring.at}
        onAt={tourAt}
      />
    ) : undefined;
  const card = touring && stop && (
    <TourCard
      stops={stops}
      at={touring.at}
      onAt={tourAt}
      onClose={endTour}
      onOpen={
        stop.symbol
          ? () => stop.symbol && onFile(stop.symbol.path, stop.symbol.line ?? undefined)
          : stop.file
            ? () => stop.file && onFile(stop.file)
            : undefined
      }
    />
  );
  const description = gen[0]?.state;
  const tours = (
    <>
      {toursOpen && (
        <ToursMenu
          map={map}
          colors={colors}
          container={container}
          onStart={startTour}
          onWrite={writeTour}
          onClose={() => setToursOpen(false)}
        />
      )}
      {genOpen && (
        <GeneratePop items={gen} onGenerate={generate} onClose={() => setGenOpen(false)} />
      )}
      {!touring && !trace && description !== "fresh" && (
        // biome-ignore lint/a11y/useKeyWithClickEvents: keeps clicks off the canvas; its button takes the keyboard
        <div
          className="sys-desc-state"
          onPointerDown={(e) => e.stopPropagation()}
          onClick={(e) => e.stopPropagation()}
        >
          {description === "missing" ? "Not described" : "Description stale"}
          <PipeButton className="sm" onClick={() => generate(["description"])}>
            Describe
          </PipeButton>
        </div>
      )}
    </>
  );
  const traceModes = trace && hops && (
    // biome-ignore lint/a11y/useKeyWithClickEvents: keeps clicks off the canvas only
    <div
      className="sys-tmap"
      onPointerDown={(e) => e.stopPropagation()}
      onClick={(e) => e.stopPropagation()}
    >
      <TraceModes show={trace.show} count={tracing.all.length} onMode={tracing.show} />
    </div>
  );
  const nodeDetail = detail && (
    <SystemDetail
      map={map}
      colors={colors}
      node={detail}
      edges={scene.edges}
      stale={stale.has(detail.id)}
      openable={!!sel || shown.kind === "context"}
      onShow={show}
      onOpen={open}
      onFile={onFile}
      onReveal={(p) => revealInFiles(workspace, p)}
      onAsk={() => sendText(askPrompt(map, detail))}
      tour={
        detail.id === SYSTEM ? map.tours.find((t) => t.id === "overview") : tourOf(map, detail.id)
      }
      onTour={startTour}
      onSymbols={(c) => go({ kind: "symbols", component: c })}
      lens={lens}
      onLook={onLook}
      onChangedFile={(p) => review?.show(p, null)}
    />
  );

  return (
    // biome-ignore lint/a11y/noNoninteractiveTabindex: the tab takes its own keys
    <div className="sys-panel" ref={root} tabIndex={0} onKeyDown={onKey}>
      <SystemBar
        crumbs={crumbs}
        level={shown.kind}
        onBack={canBack ? () => step(-1) : undefined}
        onForward={canForward ? () => step(1) : undefined}
        onLevel={pickLevel}
        symbols={symbolsOf !== undefined}
        trace={
          trace && {
            from: trace.from,
            to: trace.to,
            active: trace.active,
            targets,
            onEdit: tracing.edit,
            onPick: (end, id) => {
              if (!tracing.pick(id, end)?.active) root.current?.focus();
            },
            onSwap: tracing.swap,
            onClose: endTrace,
          }
        }
        tour={touring && { name: tourTitle, color: tourColor, onClose: endTour }}
        finding={finding}
        onFind={() => setFinding(true)}
        onTrace={() =>
          startTrace({ from: targets.some((t) => t.id === selected) ? selected : undefined })
        }
        toursOpen={toursOpen}
        onTours={() => {
          setGenOpen(false);
          setToursOpen(!toursOpen);
        }}
        lens={review ? { on: lensOn, onToggle: () => setLensPick(!lensOn) } : undefined}
        generate={
          <GenerateButton
            className="sys-btn"
            items={gen}
            open={genOpen}
            onToggle={() => {
              setToursOpen(false);
              setGenOpen(!genOpen);
            }}
          />
        }
      />
      {lens && arch && review && !touring && (
        <ReviewStrip
          arch={arch}
          range={
            view.range && `${view.range.from?.slice(0, 8) ?? "root"}..${view.range.to.slice(0, 8)}`
          }
          onClearRange={() => setArchView(review.id, { range: undefined })}
          transitive={view.transitive}
          onTransitive={(transitive) => setArchView(review.id, { transitive })}
          look={lens.open}
          onLook={() => showLook(workspace)}
        />
      )}
      {(error || sendError || archError) && (
        <div className="sys-error error">{sendError ?? error ?? archError}</div>
      )}
      <div className="sys-work">
        {trace?.from && trace.to && hops && trace.show !== "map" ? (
          <>
            <TraceView
              map={map}
              colors={colors}
              from={trace.from}
              to={trace.to}
              hops={hops}
              all={tracing.all}
              show={trace.show}
              onMode={tracing.show}
              onShow={show}
              onFile={onFile}
            />
            {nodeDetail}
          </>
        ) : shown.kind === "symbols" ? (
          <SymbolsView
            workspace={workspace}
            map={map}
            colors={colors}
            component={shown.component}
            files={lens ? files : undefined}
            changedOnly={!!lens}
            tour={stop && { file: stop.file, symbol: stop.symbol }}
            selected={symSel}
            onSelect={setSymSel}
            aside={steps}
            fallback={nodeDetail}
            onShow={show}
            onFile={onFile}
            onAsk={sendText}
          >
            {card}
            {tours}
            {traceModes}
          </SymbolsView>
        ) : (
          <>
            <SystemCanvas
              layout={placed}
              edges={scene.edges}
              lit={lit}
              hot={hot}
              hotColor={hotColor}
              selected={trace ? undefined : (stop?.node ?? sel?.id)}
              selectedEdge={edge && edgeKey(edge)}
              focus={trace || touring ? undefined : sel?.id}
              stale={stale}
              onSelect={pickNode}
              onOpen={open}
              onEdge={(e) => {
                setToursOpen(false);
                setEdgeSel(edgeKey(e));
              }}
              nodeMenu={nodeMenu}
              onReload={reload}
            >
              {card}
              {tours}
              {traceModes}
            </SystemCanvas>
            {trace && hops ? (
              <TracePanel
                map={map}
                colors={colors}
                hops={
                  hops?.length || !tracing.all.length || !trace.from || !trace.to
                    ? hops
                    : [{ from: trace.from, to: trace.to, label: "inside" }]
                }
                onShow={show}
                onFile={onFile}
              />
            ) : (
              (steps ??
              (edge && !stop?.node ? (
                <EdgeDetail
                  workspace={workspace}
                  map={map}
                  colors={colors}
                  edge={edge}
                  ends={ends}
                  lens={lens}
                  onShow={show}
                  onFile={onFile}
                  onTrace={() => startTrace(ends ?? { from: edge.from, to: edge.to })}
                  onAsk={sendText}
                  onLook={onLook}
                />
              ) : (
                nodeDetail
              )))
            )}
          </>
        )}
      </div>
      {finding && (
        <FindPalette
          map={map}
          colors={colors}
          onShow={show}
          onFile={onFile}
          onClose={() => {
            setFinding(false);
            root.current?.focus();
          }}
        />
      )}
      {picker}
    </div>
  );
}
