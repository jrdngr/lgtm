import type { ArchDto, ComponentDto, EdgeDto } from "../bindings";

type Comp = Partial<ComponentDto> & Pick<ComponentDto, "id">;

export const comp = (c: Comp): ComponentDto => ({
  name: c.id,
  container: "src",
  files: [{ path: `src/${c.id}/mod.rs`, file: null }],
  status: "unchanged",
  layer: null,
  severity: null,
  ...c,
});

export const edge = (from: string, to: string, e: Partial<EdgeDto> = {}): EdgeDto => ({
  from,
  to,
  status: "same",
  impacted: false,
  imports: [],
  ...e,
});

export function archOf(
  components: ComponentDto[],
  edges: EdgeDto[] = [],
  extra: Partial<ArchDto> = {},
): ArchDto {
  const containers = [...new Set(components.map((c) => c.container))].map((id) => ({
    id,
    name: id || "(root)",
  }));
  return {
    containers,
    components,
    edges,
    layers: [],
    look: [],
    assessment: null,
    counts: { changed: 0, upstream: 0, impacted: 0 },
    ...extra,
  };
}
