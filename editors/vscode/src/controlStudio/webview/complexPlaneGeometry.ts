export interface PlanePoint { real: number; imaginary: number }
export interface PlaneArea { width: number; height: number }
export interface PlaneGeometry {
  grid: { left: number; top: number; width?: number; height?: number; right?: number; bottom?: number };
  x?: { min: number; max: number };
  y?: { min: number; max: number };
}

const MARGIN = { left: 64, right: 24, top: 36, bottom: 52 };

/** Axis ranges with equal units per pixel on both axes, so that circles stay round and distances read true. The plot
 * fills the area; the points and their mirror images in the real axis are centred with a margin. */
export function equalUnitGeometry(points: readonly PlanePoint[], area: PlaneArea): PlaneGeometry {
  const reals = points.map(point => point.real).filter(Number.isFinite);
  const left = Math.min(...reals), right = Math.max(...reals);
  const top = Math.max(...points.map(point => Math.abs(point.imaginary)).filter(Number.isFinite), 0.05 * (right - left));
  const width = Math.max(1, area.width - MARGIN.left - MARGIN.right), height = Math.max(1, area.height - MARGIN.top - MARGIN.bottom);
  const perPixel = 1.1 * Math.max((right - left) / width, 2 * top / height);
  const centre = (left + right) / 2;
  return {
    grid: { left: MARGIN.left, top: MARGIN.top, width, height },
    x: { min: centre - perPixel * width / 2, max: centre + perPixel * width / 2 },
    y: { min: -perPixel * height / 2, max: perPixel * height / 2 },
  };
}

/** The pole-zero map: equal units around the poles, the zeros and the stability boundary, the unit circle in the
 * z-plane or the imaginary axis at the origin in the s-plane. */
export function poleMapGeometry(poles: readonly PlanePoint[], zeros: readonly PlanePoint[], discrete: boolean, area: PlaneArea): PlaneGeometry {
  const boundary = discrete ? [{ real: 1, imaginary: 0 }, { real: -1, imaginary: 0 }, { real: 0, imaginary: 1 }] : [{ real: 0, imaginary: 0 }];
  return equalUnitGeometry([...poles, ...zeros, ...boundary], area);
}
