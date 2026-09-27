/** Presentation only: dismissing a review never applies or discards its candidate. */
export function initializeSharedApplicationReviewControls(): void {
  const review = document.querySelector<HTMLElement>("[data-shared-application-review]");
  const close = document.querySelector<HTMLButtonElement>("[data-shared-application-close]");
  if (!review || !close) return;
  let origin: HTMLElement | undefined;
  const dismiss = () => {
    review.hidden = true;
    origin?.focus();
  };
  for (const button of document.querySelectorAll<HTMLElement>("[data-auto-tune-apply], [data-candidate-review]")) {
    button.addEventListener("click", () => {
      origin = button.matches("[data-auto-tune-apply]")
        ? document.querySelector<HTMLElement>("[data-control-studio-automatic-tune]") ?? button : button;
      review.hidden = false;
      requestAnimationFrame(() => {
        if (review.hidden) return;
        review.scrollIntoView({ block: "start" });
        close.focus({ preventScroll: true });
      });
    });
  }
  close.addEventListener("click", dismiss);
  review.addEventListener("keydown", event => {
    if (event.key === "Escape") { event.preventDefault(); dismiss(); }
  });
}
