// l10n-allow-file: gallery fixtures (sample prompts, replies and commands), not shipped UI.
// A chat followed at its bottom holds still while it ticks (Leo, 2026-11-18, with a recording of his
// NIGHTLY: "did we fix this scroll bounce at the bottom yet?"). His chat sat at the bottom with a
// running command ("cmux-ci submit … 8m 3s"), a background command in the composer's tray and
// "Waiting for the agent service (1:48)… Restart agent service" over the composer, every clock
// counting up each second while acpmux was slow, and the transcript jumped 30–70 px up and down every
// second or so while he did not scroll. The shift matrix measures each play step frame by frame
// (gallery/shift.ts): one step here waits 10 s on the ticking chat, the other re-sends the same
// chat every 1.2 s as a slow acpmux's waiting snapshots do. Each must score 0, and the play itself
// throws when a row in view moves more than half a pixel or Jump to latest shows.
import { agentPaneEntry, type AgentPaneVariant } from "../../gallery/format";
import type { PlayContext } from "../../gallery/play";
import type { AcpmuxSnapshot } from "./model";
import { GALLERY_NOW, minutesAgo } from "../../gallery/clock";
import { activity, assistant, chat, tool, user } from "../../gallery/fixtures/acpmux";
import { TRANSCRIPT_ROW } from "./transcriptSelectors";

const SUBMIT = "cmux-ci submit --kind cmux --label macos-26 --request-key kit-macos-build-43360-31";

function tickingChat() {
  const rows = [];
  for (let n = 0; n < 24; n++) {
    const at = 90 - n * 3;
    rows.push(user(`Question ${n + 1}: is the build on the merged head green yet?`, at));
    rows.push(
      assistant(
        `Answer ${n + 1}: the checks on the merged head are green. The macOS build job ran on the fleet and its receipt is attached to the pull request, with the test summary under it.`,
        at - 1,
      ),
    );
  }
  rows.push(user("Read why the macOS build job failed, then rerun it.", 12));
  rows.push(
    activity(
      [
        tool("Read why the macOS build job failed", "read", "completed", { id: "ticking-read" }),
        tool(SUBMIT, "execute", "in_progress", {
          id: "ticking-submit",
          command: SUBMIT,
          inputSummary: JSON.stringify({ command: SUBMIT }),
          startedAt: GALLERY_NOW - (8 * 60 + 3) * 1000,
        }),
        tool("wt new cmux leo/checkout --topic build", "execute", "completed", {
          id: "ticking-background",
          command: "wt new cmux leo/checkout --topic build",
          inputSummary: JSON.stringify({ command: "wt new cmux leo/checkout --topic build", run_in_background: true }),
          startedAt: minutesAgo(11),
          background: { state: "running" },
        }),
      ],
      11,
    ),
  );
  return chat(rows, {
    title: "Ticking bottom",
    isWorking: true,
    // "Waiting for the agent service (1:48)", counting up from there (HostWaitingLine).
    daemonWaitingSince: GALLERY_NOW - 108_000,
  });
}

/// The rows in view, by id, at their tops (viewport px).
function rowTops(doc: Document): Map<string, number> {
  const view = doc.querySelector<HTMLElement>(".acpmux-scroll")!.getBoundingClientRect();
  const tops = new Map<string, number>();
  for (const row of doc.querySelectorAll<HTMLElement>(TRANSCRIPT_ROW)) {
    const top = row.getBoundingClientRect().top;
    if (row.dataset.rowId && top > view.top && top < view.bottom) tops.set(row.dataset.rowId, top);
  }
  return tops;
}

/// Holds 10 s, re-sending the chat every `resendMs` when given, and throws on any move of a row in
/// view (the shift matrix scores the same frames).
async function holdStill(ctx: PlayContext, resendMs?: number) {
  const doc = ctx.document;
  await ctx.waitFor(() => doc.querySelector(".acpmux-host-line-group[data-host-waiting]"), { capMs: 10_000 });
  await ctx.waitFor(() => doc.querySelectorAll(TRANSCRIPT_ROW).length > 4, { capMs: 10_000 });
  const before = rowTops(doc);
  if (before.size < 3) throw new Error(`only ${before.size} rows in view`);
  const win = doc.defaultView!;
  const bridge = (win as unknown as { cmuxAcpmuxBridge?: { receive(snapshot: AcpmuxSnapshot): void } })
    .cmuxAcpmuxBridge;
  const snapshot = tickingChat();
  const resend = resendMs
    ? win.setInterval(() => bridge?.receive(structuredClone(snapshot)), resendMs)
    : undefined;
  let most = 0;
  let worst = "";
  let jump = false;
  const start = win.performance.now();
  try {
    await ctx.waitFor(
      () => {
        const now = rowTops(doc);
        for (const [id, top] of before) {
          const at = now.get(id);
          const moved = at === undefined ? Infinity : Math.abs(at - top);
          if (moved > most) {
            most = moved;
            worst = id;
          }
        }
        if (doc.querySelector(".acpmux-jump-latest[data-shown]")) jump = true;
        return win.performance.now() - start >= 10_000;
      },
      { capMs: 12_000 },
    );
  } finally {
    if (resend !== undefined) win.clearInterval(resend);
  }
  if (most > 0.5) throw new Error(`a row in view moved ${most.toFixed(1)} px (${worst}) while the chat ticked`);
  if (jump) throw new Error("Jump to latest showed while the chat ticked at its bottom");
}

const variant = (resendMs?: number): AgentPaneVariant => ({
  note: resendMs
    ? "Play: the same chat is re-sent every 1.2 s, as a slow acpmux's waiting snapshots arrive, for 10 s. No row in view moves."
    : "Play: 10 s at the bottom while the waiting line, the running command and the tray count up. No row in view moves.",
  snapshot: tickingChat(),
  play: (ctx) => holdStill(ctx, resendMs),
});

export default agentPaneEntry({
  id: "agent-pane.ticking-bottom-still",
  title: "Ticking chat holds still at the bottom",
  area: "Agent pane",
  height: 640,
  widths: { narrow: 420, normal: 760, wide: 900 },
  anchors: [{ selector: ".acpmux-composer-box" }],
  covers: ["agent-session/acpmux/VirtualTranscript.tsx"],
  variants: {
    ticking: variant(),
    "ticking-resent": variant(1200),
  },
});
