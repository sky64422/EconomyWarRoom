import type { NotchLayout } from "./types";

interface SurfaceInput {
  expanded: boolean;
  height: number;
  target: number | null;
  providers: string[];
}
export interface SurfaceRequest {
  revision: number;
  expanded: boolean;
  height: number;
  target: number | null;
}

/** Serialize native geometry requests and paint only the latest requested surface. */
export function createNotchSurface(handlers: {
  send: (request: SurfaceRequest) => Promise<NotchLayout>;
  paint: (layout: NotchLayout, request: SurfaceRequest) => void;
  fail: (error: unknown) => void;
}) {
  let revision = Date.now();
  let lastKey = "";
  let chain: Promise<void> = Promise.resolve();
  let disposed = false;
  return {
    request({ expanded, height, target, providers }: SurfaceInput) {
      const key = JSON.stringify([expanded, height, target, providers]);
      if (disposed || key === lastKey) return;
      lastKey = key;
      const requestRevision = ++revision;
      chain = chain.then(async () => {
        if (disposed) return;
        const next = await handlers.send({ revision: requestRevision, expanded, height, target });
        if (!disposed && requestRevision === revision) handlers.paint(next, { revision: requestRevision, expanded, height, target });
      }).catch(error => {
        if (disposed) return;
        if (requestRevision === revision) lastKey = "";
        handlers.fail(error);
      });
    },
    settled: () => chain,
    destroy() { disposed = true; },
  };
}
