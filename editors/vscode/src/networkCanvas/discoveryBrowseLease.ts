export interface DiscoveryBrowseLease {
  readonly originId: string;
  readonly leaseId: string;
  readonly webviewSessionId: string;
  readonly browseSessionId?: string;
}

/** Compare-and-release lease for the one discovery-origin Browse flow a webview can own. */
export class DiscoveryBrowseLeaseStore {
  private active: DiscoveryBrowseLease | undefined;

  begin(originId: string, leaseId: string, webviewSessionId: string): void {
    this.active = { originId, leaseId, webviewSessionId };
  }

  bindAndValidate(
    originId: string,
    leaseId: string | undefined,
    webviewSessionId: string | undefined,
    browseSessionId: string | undefined
  ): boolean {
    const active = this.active;
    if (
      !active ||
      !leaseId ||
      !webviewSessionId ||
      !browseSessionId ||
      active.originId !== originId ||
      active.leaseId !== leaseId ||
      active.webviewSessionId !== webviewSessionId ||
      (active.browseSessionId !== undefined &&
        active.browseSessionId !== browseSessionId)
    ) {
      return false;
    }
    if (active.browseSessionId === undefined) {
      this.active = { ...active, browseSessionId };
    }
    return true;
  }

  release(
    originId: unknown,
    leaseId: unknown,
    browseSessionId?: unknown
  ): boolean {
    const active = this.active;
    if (
      !active ||
      originId !== active.originId ||
      leaseId !== active.leaseId ||
      (active.browseSessionId !== undefined &&
        browseSessionId !== active.browseSessionId)
    ) {
      return false;
    }
    this.active = undefined;
    return true;
  }

  clear(): void {
    this.active = undefined;
  }

  current(): DiscoveryBrowseLease | undefined {
    return this.active;
  }
}
