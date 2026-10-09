import { isSavedRunValidation, type SavedRunValidation } from "./runValidationPersistence";
export class RunValidationSession {
  private saved: SavedRunValidation | undefined;
  snapshot(): SavedRunValidation | undefined { return this.saved ? JSON.parse(JSON.stringify(this.saved)) : undefined; }
  restore(saved: SavedRunValidation | undefined): void {
    if (saved && !isSavedRunValidation(saved)) throw new Error("Saved run validation is invalid.");
    this.saved = saved ? JSON.parse(JSON.stringify(saved)) : undefined;
  }
  dispose(): void { this.saved = undefined; }
}
