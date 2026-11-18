// Which components React rendered for one event, by name: the probe behind Leo's question (2026-11-18),
// "could you just draw the diff? I mean, isn't that what React is supposed to do?".
//
// `addRenderCountProbe` installs a stand-in `__REACT_DEVTOOLS_GLOBAL_HOOK__` before the page's first script, the
// way React DevTools does, so react-dom (production or development) registers with it and calls
// `onCommitFiberRoot` after every commit. On each commit the probe walks only the part of the fiber tree React
// touched, as DevTools does: a subtree whose parent kept its child list (a bailout) is skipped whole, so a walk
// costs what the commit changed, not the tree. It counts:
//   renders    component fibers whose function ran this commit (React's PerformedWork flag, or a new mount);
//   mounts     the subset of renders that were new mounts;
//   host       host elements (div, span, ...) whose props object changed, i.e. React diffed them at commit.
// Names come from the component's function name, so the page must be an unminified build
// (`serveBenchPage(..., { unminified: true })`, or `minify: false` in a bench's own vite build); a minified
// build prints mangled names and the probe says so.
//
// One event's numbers: `await renderCountsReset(page)`, run the event, wait for it to settle, then
// `await renderCountsTake(page)`. `formatRenderCounts` prints the top components and the totals in one line.
/// <reference types="bun" />
import type { BrowserContext, Page } from "playwright";

/// Renders, mounts and host-element updates since the last reset.
export interface RenderCounts {
  /// Commits React made in the window.
  commits: number;
  /// Component renders in all, and by component name.
  renders: number;
  byComponent: Record<string, number>;
  /// New mounts in all, and by name (a subset of renders).
  mounts: number;
  mountsByComponent: Record<string, number>;
  /// Host elements React re-diffed (their props object changed).
  host: number;
  /// Whether the names look minified (most are one or two letters): then the by-name rows mean nothing.
  minified: boolean;
}

/// The init script. Self-contained: it runs in the page, before any of the page's own code.
function renderCountProbe() {
  type Fiber = {
    tag: number;
    type: unknown;
    elementType: unknown;
    flags: number;
    child: Fiber | null;
    sibling: Fiber | null;
    alternate: Fiber | null;
    memoizedProps: unknown;
  };
  const w = window as unknown as Record<string, unknown>;
  if (w.__cmuxRenderCounts) return;
  // React's fiber tags (ReactWorkTags.js) and flags (ReactFiberFlags.js), stable since React 17.
  const FunctionComponent = 0;
  const ClassComponent = 1;
  const HostComponent = 5;
  const ForwardRef = 11;
  const MemoComponent = 14;
  const SimpleMemoComponent = 15;
  const PerformedWork = 1;
  const components = new Set([FunctionComponent, ClassComponent, ForwardRef, MemoComponent, SimpleMemoComponent]);
  let state = {
    commits: 0,
    renders: 0,
    mounts: 0,
    host: 0,
    byComponent: new Map<string, number>(),
    mountsByComponent: new Map<string, number>(),
  };
  const nameOf = (fiber: Fiber): string => {
    let type = fiber.type as { displayName?: string; name?: string; render?: unknown; type?: unknown } | null;
    if (fiber.tag === MemoComponent || fiber.tag === SimpleMemoComponent)
      type = (fiber.elementType as { type?: typeof type })?.type ?? type;
    if (type && fiber.tag === ForwardRef && type.render) type = type.render as typeof type;
    if (type && typeof type === "object" && type.type) type = type.type as typeof type;
    return (type && (type.displayName || type.name)) || "(anonymous)";
  };
  const bump = (map: Map<string, number>, key: string) => map.set(key, (map.get(key) ?? 0) + 1);
  const rendered = (fiber: Fiber, mount: boolean) => {
    if (!components.has(fiber.tag)) {
      if (fiber.tag === HostComponent && (mount || fiber.memoizedProps !== fiber.alternate?.memoizedProps))
        state.host += 1;
      return;
    }
    // A memo wrapper (tag 14) renders nothing itself: its inner component is its child and is counted there.
    if (fiber.tag === MemoComponent) return;
    const name = nameOf(fiber);
    state.renders += 1;
    bump(state.byComponent, name);
    if (mount) {
      state.mounts += 1;
      bump(state.mountsByComponent, name);
    }
  };
  const mountTree = (fiber: Fiber | null) => {
    for (let at = fiber; at; at = at.sibling) {
      rendered(at, true);
      mountTree(at.child);
    }
  };
  // `next` is this commit's fiber, `prev` the one it replaced. A parent that bailed out keeps its old child
  // list (`next.child === prev.child`): nothing under it rendered, so the walk stops there.
  const update = (next: Fiber, prev: Fiber) => {
    if (components.has(next.tag) ? (next.flags & PerformedWork) !== 0 : true) rendered(next, false);
    if (next.child === prev.child) return;
    for (let child = next.child; child; child = child.sibling) {
      if (child.alternate) update(child, child.alternate);
      else mountTree({ ...child, sibling: null } as Fiber);
    }
  };
  const hook = {
    renderers: new Map<number, unknown>(),
    supportsFiber: true,
    isDisabled: false,
    inject(renderer: unknown) {
      const id = hook.renderers.size + 1;
      hook.renderers.set(id, renderer);
      return id;
    },
    checkDCE() {},
    onScheduleFiberRoot() {},
    onCommitFiberUnmount() {},
    onPostCommitFiberRoot() {},
    onCommitFiberRoot(_id: number, root: { current: Fiber }) {
      state.commits += 1;
      const current = root.current;
      const prev = current.alternate;
      if (!prev) mountTree(current.child);
      else update(current, prev);
    },
  };
  w.__REACT_DEVTOOLS_GLOBAL_HOOK__ = hook;
  w.__cmuxRenderCounts = {
    reset() {
      state = {
        commits: 0,
        renders: 0,
        mounts: 0,
        host: 0,
        byComponent: new Map(),
        mountsByComponent: new Map(),
      };
    },
    take() {
      const names = [...state.byComponent.keys()];
      const short = names.filter((name) => name.length <= 2 && name !== "(anonymous)").length;
      return {
        commits: state.commits,
        renders: state.renders,
        byComponent: Object.fromEntries(state.byComponent),
        mounts: state.mounts,
        mountsByComponent: Object.fromEntries(state.mountsByComponent),
        host: state.host,
        minified: names.length > 4 && short / names.length > 0.5,
      };
    },
  };
}

/// Installs the probe on every page of `target` (a context, or one page before its `goto`).
export async function addRenderCountProbe(target: BrowserContext | Page): Promise<void> {
  await target.addInitScript(renderCountProbe);
}

/// Whether the page's React registered with the probe (false: the probe was installed after react-dom loaded).
export async function renderCountProbeAttached(page: Page): Promise<boolean> {
  return page.evaluate(() => {
    const hook = (window as unknown as { __REACT_DEVTOOLS_GLOBAL_HOOK__?: { renderers?: Map<number, unknown> } })
      .__REACT_DEVTOOLS_GLOBAL_HOOK__;
    return (hook?.renderers?.size ?? 0) > 0;
  });
}

export async function renderCountsReset(page: Page): Promise<void> {
  await page.evaluate(() => (window as unknown as { __cmuxRenderCounts: { reset(): void } }).__cmuxRenderCounts.reset());
}

export async function renderCountsTake(page: Page): Promise<RenderCounts> {
  return page.evaluate(
    () => (window as unknown as { __cmuxRenderCounts: { take(): RenderCounts } }).__cmuxRenderCounts.take(),
  );
}

/// The counts divided by `per` (keys typed, tokens streamed): per-event numbers, to two places.
export function perEvent(counts: RenderCounts, per: number): RenderCounts {
  const div = (value: number) => Math.round((value / per) * 100) / 100;
  const each = (record: Record<string, number>) =>
    Object.fromEntries(Object.entries(record).map(([key, value]) => [key, div(value)]));
  return {
    ...counts,
    commits: div(counts.commits),
    renders: div(counts.renders),
    byComponent: each(counts.byComponent),
    mounts: div(counts.mounts),
    mountsByComponent: each(counts.mountsByComponent),
    host: div(counts.host),
  };
}

/// One line: `RENDERS <label> commits=.. renders=.. mounts=.. host=.. top: A=n B=n ...`.
export function formatRenderCounts(label: string, counts: RenderCounts, top = 12): string {
  const ranked = Object.entries(counts.byComponent)
    .sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))
    .slice(0, top)
    .map(([name, count]) => `${name}=${count}`)
    .join(" ");
  return (
    `RENDERS ${label} commits=${counts.commits} renders=${counts.renders} mounts=${counts.mounts} host=${counts.host}` +
    `${counts.minified ? " (minified names: build unminified)" : ""} top: ${ranked}`
  );
}
