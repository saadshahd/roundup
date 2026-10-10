// Y2: reads the layout at every animation frame while `act` runs, so a law that holds "in every frame" is checked in every frame and a failure shows the first bad frame.
// It reads and writes nothing the user sees. A sampler that read no frame, or a browser without `layout-shift`, throws: it never passes empty.

export type Frame = { at: number; scrollLeft: number };

export type Sampled = {
  frames: Frame[];
  /** Sum of `layout-shift` values whose moved nodes are inside `watched`. */
  shift: number;
  /** The first frame whose `scrollLeft` is above 0, or null. */
  firstBad: Frame | null;
};

export type SampleOptions = {
  /** The element holding the Rail, terminal and Shelf. */
  columns: Element;
  /** The Rail, the terminal pane and the Shelf. */
  watched: Element[];
  /** Opens or closes the Drawer and resolves when its slide has ended. */
  act: () => Promise<void>;
  /** Frames are still read this long after `act` resolves. */
  afterMs?: number;
};

type ShiftEntry = PerformanceEntry & { value: number; sources?: { node: Node | null }[] };

const nextFrame = () => new Promise<number>((resolve) => requestAnimationFrame(resolve));

const hasLayoutShift = () => PerformanceObserver.supportedEntryTypes.includes("layout-shift");

export const sampleFrames = async ({ columns, watched, act, afterMs = 300 }: SampleOptions): Promise<Sampled> => {
  if (!hasLayoutShift()) throw new Error("this browser has no layout-shift entries; the Drawer law cannot be measured");

  let shift = 0;

  const count = (entries: PerformanceEntryList) => {
    // SAFETY: the observer is registered for `layout-shift` only, whose entries are `LayoutShift` (`value`, `sources`).
    for (const entry of entries as ShiftEntry[]) {
      if (entry.sources?.some((source) => source.node && watched.some((element) => element.contains(source.node))) ?? false) shift += entry.value;
    }
  };

  const observer = new PerformanceObserver((list) => count(list.getEntries()));

  observer.observe({ type: "layout-shift", buffered: false });

  const frames: Frame[] = [];
  let running = true;

  const read = async () => {
    while (running) {
      const at = await nextFrame();

      if (running) frames.push({ at, scrollLeft: columns.scrollLeft });
    }
  };

  const reading = read();

  try {
    await act();

    const end = performance.now() + afterMs;

    while (performance.now() < end) await nextFrame();
  } finally {
    running = false;
    await reading;
    // `takeRecords` flushes entries the observer has not yet delivered.
    count(observer.takeRecords());
    observer.disconnect();
  }

  if (frames.length === 0) throw new Error("the sampler read no frames");

  return { frames, shift, firstBad: frames.find((frame) => frame.scrollLeft > 0) ?? null };
};
