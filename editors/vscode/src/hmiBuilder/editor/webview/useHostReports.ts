// What the editor's host reports about the project and the page (specification 39, §3.2): the
// component catalogue, the pages, problems and warnings, click targets, panels and their
// placements, texts, docked panels, the view and the picker's values. The webview only shows them.

import { useEffect, useState } from "react";

import type { DrawFrame, HmiPageModel, PlacedFaceplate } from "../../drawing/model";
import { registerSymbols } from "../../drawing/symbols";
import type { Catalogue } from "../catalogue";
import type { FaceplateEntry, HostToWebview, LiveUpdate, PageEntry, ProjectTexts } from "../messages";
import type { QualityWarning } from "../quality";
import type { ClickTargets } from "./ClickSection";

export interface HostReports {
  catalogue?: Catalogue;
  formats?: DrawFrame["formats"];
  pages?: { pages: PageEntry[]; current: string };
  problems: Record<string, string[]>;
  warnings: Record<string, QualityWarning[]>;
  colourWarnings: QualityWarning[];
  targets: ClickTargets;
  faceplates: FaceplateEntry[];
  placed: Record<string, PlacedFaceplate | null>;
  projectTexts?: ProjectTexts;
  /** The page with the panels docked on it (§21.1 item 3). */
  composition?: HmiPageModel;
  pidView: boolean;
  pickerUpdate?: LiveUpdate;
}

export function useHostReports(): HostReports {
  const [catalogue, setCatalogue] = useState<Catalogue>();
  const [formats, setFormats] = useState<DrawFrame["formats"]>();
  const [pages, setPages] = useState<HostReports["pages"]>();
  const [problems, setProblems] = useState<Record<string, string[]>>({});
  const [warnings, setWarnings] = useState<Record<string, QualityWarning[]>>({});
  const [colourWarnings, setColourWarnings] = useState<QualityWarning[]>([]);
  const [targets, setTargets] = useState<ClickTargets>({ pages: [], popups: [], faceplates: [] });
  const [faceplates, setFaceplates] = useState<FaceplateEntry[]>([]);
  const [placed, setPlaced] = useState<Record<string, PlacedFaceplate | null>>({});
  const [projectTexts, setProjectTexts] = useState<ProjectTexts>();
  const [composition, setComposition] = useState<HmiPageModel>();
  const [pidView, setPidView] = useState(false);
  const [pickerUpdate, setPickerUpdate] = useState<LiveUpdate>();
  useEffect(() => {
    const onMessage = (event: MessageEvent<HostToWebview>): void => {
      const message = event.data;
      switch (message?.type) {
        case "catalogue":
          // The project's own symbols are drawn as the shipped ones are (§14.4).
          registerSymbols(message.symbols ?? []);
          setCatalogue(message.catalogue);
          setFormats(message.formats);
          return;
        case "pages":
          setPages({ pages: message.pages, current: message.current });
          return;
        case "problems":
          setProblems(message.problems);
          setWarnings(message.warnings ?? {});
          return;
        case "colourWarnings":
          setColourWarnings(message.warnings);
          return;
        case "targets":
          setTargets({ pages: message.pages, popups: message.popups, faceplates: message.faceplates ?? [] });
          return;
        case "faceplates":
          setFaceplates(message.faceplates);
          return;
        case "placed":
          setPlaced(message.placed);
          return;
        case "texts":
          setProjectTexts(message.texts);
          return;
        case "sharedPanels":
          setComposition(message.model);
          return;
        case "view":
          setPidView(message.view === "pid");
          return;
        case "pickerValues":
          setPickerUpdate(message.update);
          return;
      }
    };
    window.addEventListener("message", onMessage);
    return () => window.removeEventListener("message", onMessage);
  }, []);
  return {
    catalogue,
    formats,
    pages,
    problems,
    warnings,
    colourWarnings,
    targets,
    faceplates,
    placed,
    projectTexts,
    composition,
    pidView,
    pickerUpdate,
  };
}
