/** What an edit changed on one page (specification 43, AT-09). The authoring command computes it; this is the
 * shape the proposal review reads. It holds no rule. */
export interface PageChanges {
  added: string[];
  removed: string[];
  modified: Array<{ id: string; properties: string[] }>;
  pageProperties: string[];
  orderChanged: boolean;
}
