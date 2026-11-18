// The composer's own growth, declared by the code that makes it (schemas/layout-motion/contract.json
// `composer-growth`): the field gaining or losing a line (fieldGrowth.ts), and the attachment row coming
// or going above it (ComposerBox.tsx). Leo, 2026-11-18: "the things that move we only expect at certain
// times". An edit that places an attachment grows the box in its own frame (composer-attach-no-jump.test.ts);
// a paste, a drop, a capture the host answers and a draft read back all arrive that way, by no key, so
// nothing else declares them.
//
// The declaration is as short as the move: until the field's motion lands, or for the frame the box
// changes in and the next (the judge reads what the frame drew after it painted). A box that grows later
// on its own (an image's picture decoding to another size) is outside it and still counts.
import { declareMove } from "../../../layout/motionContract";

/** Declares the composer's growth for `ms` (a running motion), or for this frame and the next. */
export function declareComposerGrowth(win: Window | null | undefined, by: string, ms?: number): void {
  if (ms !== undefined) {
    declareMove({ event: "composer-growth", scope: "all", by }, { ms });
    return;
  }
  const end = declareMove({ event: "composer-growth", scope: "all", by });
  if (!win?.requestAnimationFrame) {
    end();
    return;
  }
  win.requestAnimationFrame(() => win.requestAnimationFrame(end));
}
