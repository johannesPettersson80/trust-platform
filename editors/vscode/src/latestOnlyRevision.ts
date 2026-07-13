/** Prevents an older asynchronous render from committing after a newer one. */
export class LatestOnlyRevision {
  private revision = 0;

  begin(): number {
    this.revision += 1;
    return this.revision;
  }

  isCurrent(revision: number): boolean {
    return revision === this.revision;
  }

  invalidate(): void {
    this.revision += 1;
  }
}
