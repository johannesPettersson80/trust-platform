import { EquipmentConnections } from "./EquipmentConnections";
import React from "react";

import { AddDevicePanel } from "./AddDevicePanel";
import { AddHostPanel } from "./AddHostPanel";
import { AddPane } from "./AddPane";
import { AddRuntimePanel } from "./AddRuntimePanel";
import { BrowseTagsPanel } from "./BrowseTagsPanel";
import { DiscoverPane } from "./DiscoverPane";
import { FilterPanel } from "./FilterPanel";
import { NodeInspector } from "./NodeInspector";
import { PlantDraftPanel } from "./PlantDraftPanel";
import { SetUpRuntimePanel } from "./SetUpRuntimePanel";

export interface CanvasDrawerRoutes {
  equipment?: React.ComponentProps<typeof EquipmentConnections>;
  addDevice?: React.ComponentProps<typeof AddDevicePanel>;
  plantDraft?: React.ComponentProps<typeof PlantDraftPanel>;
  inspectNode?: React.ComponentProps<typeof NodeInspector>;
  browseTags?: React.ComponentProps<typeof BrowseTagsPanel>;
  discover?: React.ComponentProps<typeof DiscoverPane>;
  setupRuntime?: React.ComponentProps<typeof SetUpRuntimePanel>;
  addHost?: React.ComponentProps<typeof AddHostPanel>;
  addConnection?: React.ComponentProps<typeof AddPane>;
  addRuntime?: React.ComponentProps<typeof AddRuntimePanel>;
  filter?: React.ComponentProps<typeof FilterPanel>;
}

// The route order is intentional: an editing task wins over read-only
// selection, and a browse task wins over discovery. This preserves one active
// right-hand task surface and keeps drawer geometry deterministic.
export function activeDrawerWidth(routes: CanvasDrawerRoutes): number {
  if (routes.equipment) return 360;
  // Plant setup is a main-canvas editor surface, not a narrow inspector.
  if (routes.plantDraft) return 0;
  if (routes.addDevice) return 360;
  if (routes.inspectNode) return 340;
  if (routes.browseTags) return 340;
  if (routes.discover) return 290;
  if (routes.setupRuntime) return 252;
  if (routes.addHost) return 300;
  if (routes.addConnection) return 360;
  if (routes.addRuntime) return 232;
  if (routes.filter) return 184;
  return 0;
}

export function CanvasDrawerRouter({ routes }: { routes: CanvasDrawerRoutes }) {
  if (routes.equipment) return <EquipmentConnections {...routes.equipment}/>;
  return (
    <>
      {routes.addConnection && <AddPane {...routes.addConnection} />}
      {routes.setupRuntime && <SetUpRuntimePanel {...routes.setupRuntime} />}
      {routes.addRuntime && <AddRuntimePanel {...routes.addRuntime} />}
      {routes.addHost && <AddHostPanel {...routes.addHost} />}
      {routes.filter && <FilterPanel {...routes.filter} />}
      {routes.discover && <DiscoverPane {...routes.discover} />}
      {routes.addDevice && <AddDevicePanel {...routes.addDevice} />}
      {routes.plantDraft && <PlantDraftPanel {...routes.plantDraft} />}
      {routes.inspectNode && <NodeInspector {...routes.inspectNode} />}
      {routes.browseTags && <BrowseTagsPanel {...routes.browseTags} />}
    </>
  );
}
