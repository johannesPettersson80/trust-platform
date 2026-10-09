// Bound ordinary value-card painting so large changing pages do not do all SVG text layout in
// one frame. State/alarm changes bypass this queue; a whole card keeps its value and unit together.
import type { SvgNode } from "../drawing/svg";
interface Paint {
  before: SvgNode;
  after: SvgNode;
}
export class ValuePaintQueue {
  private readonly pending = new Map<Element, Paint>();
  private scheduled = false;
  private disposed = false;
  constructor(
    private readonly schedule: (callback: () => void) => void,
    private readonly paint: (element: Element, before: SvgNode, after: SvgNode) => void,
    private readonly perFrame = 96,
  ) {}
  enqueue(element: Element, before: SvgNode, after: SvgNode): void {
    if (this.disposed) return;
    this.pending.set(element, { before: this.pending.get(element)?.before ?? before, after });
    this.request();
  }
  immediate(element: Element, before: SvgNode, after: SvgNode): void {
    const actual = this.pending.get(element)?.before ?? before;
    this.pending.delete(element);
    if (!this.disposed) this.paint(element, actual, after);
  }
  /** Paint in the current render turn, sharing its layout pass. Larger pages defer overflow. */
  flush(): void {
    if (this.disposed) return;
    const batch = [...this.pending].slice(0, this.perFrame);
    for (const [element, update] of batch) {
      this.pending.delete(element);
      if (element.isConnected) this.paint(element, update.before, update.after);
    }
    this.request();
  }
  clear(): void {
    this.pending.clear();
  }
  dispose(): void {
    this.disposed = true;
    this.clear();
  }
  private request(): void {
    if (this.scheduled || this.disposed || !this.pending.size) return;
    this.scheduled = true;
    this.schedule(() => {
      this.scheduled = false;
      this.flush();
    });
  }
}
