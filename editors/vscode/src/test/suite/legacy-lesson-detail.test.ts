import * as assert from "assert";

import { controlLessonDetail, type ControlLessonSummary } from "../../examples/controlLesson";

suite("Legacy lesson detail", () => {
  test("labels summary-only lessons as unavailable instead of promising a runnable response", () => {
    const summary: ControlLessonSummary = {
      sequence: 2,
      level: "Foundation",
      equipment: "simulation",
      principalTechnique: "Retired technique",
      prerequisites: [],
    };
    const detail = controlLessonDetail(summary, "Legacy catalog entry");
    assert.strictEqual(
      detail.expectedObservation,
      "This legacy lesson is unavailable as a runnable project in the current gallery."
    );
    assert.doesNotMatch(detail.expectedObservation, /complete the runnable example|inspect its measured response/i);
  });
});
